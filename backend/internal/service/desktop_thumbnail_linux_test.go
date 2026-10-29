package service

import (
	"bytes"
	"context"
	"errors"
	"image/color"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"syscall"
	"testing"
	"time"

	"golang.org/x/sys/unix"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestDesktopThumbnailProcessHelper(t *testing.T) {
	marker := -1
	for i, arg := range os.Args {
		if arg == "--thumbnail-helper" {
			marker = i
			break
		}
	}
	if marker < 0 {
		return
	}
	args := os.Args[marker+1:]
	if len(args) < 4 || os.Getenv("BOXBOX_DESKTOP_TOKEN") != "" || os.Getenv("LD_PRELOAD") != "" {
		os.Exit(1)
	}
	mode := args[0]
	switch mode {
	case "copy":
		if args[1] != "image" {
			os.Exit(1)
		}
		source := os.NewFile(3, "source")
		if _, err := io.Copy(os.Stdout, source); err != nil {
			os.Exit(1)
		}
	case "oversized":
		_, _ = os.Stdout.Write(bytes.Repeat([]byte("x"), maxNativeThumbnailBytes+1))
	case "invalid":
		_, _ = os.Stdout.Write([]byte("not JPEG"))
	case "unsupported":
		os.Exit(2)
	case "limit":
		os.Exit(3)
	case "wait":
		child := exec.Command("sleep", "60")
		if err := child.Start(); err != nil {
			os.Exit(1)
		}
		if err := os.WriteFile(args[1], []byte(strconv.Itoa(child.Process.Pid)), 0600); err != nil {
			os.Exit(1)
		}
		_ = child.Wait()
	}
	os.Exit(0)
}

func thumbnailHelper(mode string, extra ...string) []string {
	return append([]string{os.Args[0], "-test.run=^TestDesktopThumbnailProcessHelper$", "--", "--thumbnail-helper", mode}, extra...)
}

func TestDesktopThumbnailUsesConfinedDescriptorAndCache(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "photo.jpg")
	original := generatedImage(t, "jpeg", 32, 16, color.NRGBA{R: 220, A: 255})
	if err := os.WriteFile(source, original, 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("BOXBOX_DESKTOP_TOKEN", "must-not-leak")
	files := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: root}}})
	renders := 0
	native := NewDesktopThumbnailRenderer(thumbnailHelper("copy"))
	renderer := func(ctx context.Context, file File, info *model.FileInfo, w, h int) ([]byte, error) {
		renders++
		if renders == 1 {
			if err := os.Rename(source, filepath.Join(root, "original.jpg")); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(source, generatedImage(t, "jpeg", 32, 16, color.NRGBA{B: 220, A: 255}), 0600); err != nil {
				t.Fatal(err)
			}
			// Keep the cache key distinct even on filesystems with coarse timestamps.
			replacementTime := info.ModTime.Add(2 * time.Second)
			if err := os.Chtimes(source, replacementTime, replacementTime); err != nil {
				t.Fatal(err)
			}
			replacement, err := os.Stat(source)
			if err != nil {
				t.Fatal(err)
			}
			if !replacement.ModTime().After(info.ModTime) {
				t.Fatal("replacement fixture did not change the thumbnail cache key")
			}
		}
		return native(ctx, file, info, w, h)
	}
	service := NewThumbnailServiceWithRenderer(files, renderer)
	result, err := service.Render(context.Background(), "media/photo.jpg", 64, 64, "")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(result.Content, original) {
		t.Fatal("renderer reopened the replacement path instead of the confined inode")
	}
	if _, err := service.Render(context.Background(), "outside/photo.jpg", 64, 64, ""); err == nil {
		t.Fatal("unmounted file reached renderer")
	}
	if renders != 1 {
		t.Fatalf("unmounted file invoked renderer (%d calls)", renders)
	}
	// A changed source invalidates the original inode's cached result.
	changed, err := service.Render(context.Background(), "media/photo.jpg", 64, 64, "")
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(changed.Content, original) {
		t.Fatal("changed source retained old thumbnail")
	}
	if _, err := service.Render(context.Background(), "media/photo.jpg", 64, 64, changed.ETag); !errors.Is(err, ErrThumbnailNotModified) {
		t.Fatal(err)
	}
	if renders != 2 {
		t.Fatalf("conditional request rerendered (%d calls)", renders)
	}
}

func TestDesktopThumbnailRejectsBadWorkerOutput(t *testing.T) {
	source, err := os.CreateTemp(t.TempDir(), "image-*.jpg")
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	for _, test := range []struct {
		mode string
		want error
	}{
		{"oversized", ErrThumbnailTooLarge}, {"invalid", ErrThumbnailUnsupported},
		{"unsupported", ErrThumbnailUnsupported}, {"limit", ErrThumbnailTooLarge},
	} {
		t.Run(test.mode, func(t *testing.T) {
			render := NewDesktopThumbnailRenderer(thumbnailHelper(test.mode))
			_, err := render(context.Background(), source, &model.FileInfo{Name: "image.jpg"}, 224, 144)
			if !errors.Is(err, test.want) {
				t.Fatalf("got %v, want %v", err, test.want)
			}
		})
	}
}

func TestDesktopThumbnailCancellationKillsMediaChildren(t *testing.T) {
	root := t.TempDir()
	source, err := os.Create(filepath.Join(root, "source.mp4"))
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	pidPath := filepath.Join(root, "child.pid")
	render := NewDesktopThumbnailRenderer(thumbnailHelper("wait", pidPath))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan error, 1)
	go func() { _, err := render(ctx, source, &model.FileInfo{Name: "source.mp4"}, 224, 144); done <- err }()
	var childPID int
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if value, err := os.ReadFile(pidPath); err == nil {
			if pid, err := strconv.Atoi(string(value)); err == nil && pid > 0 {
				childPID = pid
				break
			}
		}
		time.Sleep(10 * time.Millisecond)
	}
	if childPID == 0 {
		t.Fatal("media child never started")
	}
	pidfd, err := unix.PidfdOpen(childPID, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(pidfd)
	cancel()
	select {
	case err := <-done:
		if !errors.Is(err, context.Canceled) {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("renderer did not cancel")
	}
	// SIGKILL is asynchronous for descendants; observe exit instead of one /proc snapshot.
	events := []unix.PollFd{{Fd: int32(pidfd), Events: unix.POLLIN}}
	deadline = time.Now().Add(5 * time.Second)
	for {
		remaining := time.Until(deadline).Milliseconds()
		if remaining <= 0 {
			t.Fatal("cancelled renderer left its media child running")
		}
		n, err := unix.Poll(events, int(remaining))
		if errors.Is(err, unix.EINTR) {
			continue
		}
		if err != nil || n != 1 || events[0].Revents&unix.POLLIN == 0 {
			t.Fatalf("media child did not exit: events=%v, err=%v", events, err)
		}
		break
	}
}

func TestThumbnailDoesNotOpenNamedPipes(t *testing.T) {
	root := t.TempDir()
	if err := syscall.Mkfifo(filepath.Join(root, "clip.mp4"), 0600); err != nil {
		t.Fatal(err)
	}
	files := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: root}}})
	_, _, err := files.OpenFile(context.Background(), "media/clip.mp4")
	if !errors.Is(err, ErrNotFile) {
		t.Fatalf("got %v, want a regular-file rejection", err)
	}
}
