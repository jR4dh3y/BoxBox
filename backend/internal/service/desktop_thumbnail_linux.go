package service

import (
	"bytes"
	"context"
	"errors"
	"image/jpeg"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

const maxNativeThumbnailBytes = 1 << 20

// The isolated process inherits only the read-only descriptor, never a source path.
func NewDesktopThumbnailRenderer(command []string) ThumbnailRenderer {
	command = append([]string(nil), command...)
	return func(ctx context.Context, file File, info *model.FileInfo, width, height int) ([]byte, error) {
		kind := thumbnailMediaKind(info)
		if kind == "" || len(command) == 0 {
			return nil, ErrThumbnailUnsupported
		}
		source, ok := file.(interface {
			Fd() uintptr
			Stat() (fs.FileInfo, error)
		})
		if !ok {
			return nil, ErrThumbnailUnsupported
		}
		stat, err := source.Stat()
		if err != nil {
			return nil, err
		}
		if !stat.Mode().IsRegular() {
			return nil, ErrThumbnailUnsupported
		}
		descriptor, err := syscall.Dup(int(source.Fd()))
		if err != nil {
			return nil, err
		}
		owned := os.NewFile(uintptr(descriptor), "thumbnail-source")
		defer owned.Close()
		renderCtx, cancel := context.WithTimeout(ctx, 8*time.Second)
		defer cancel()
		args := append(append([]string(nil), command[1:]...), kind, strconv.Itoa(width), strconv.Itoa(height))
		worker := exec.CommandContext(renderCtx, command[0], args...)
		worker.ExtraFiles = []*os.File{owned}
		worker.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
		worker.WaitDelay = time.Second
		worker.Cancel = func() error {
			if worker.Process == nil {
				return os.ErrProcessDone
			}
			err := syscall.Kill(-worker.Process.Pid, syscall.SIGKILL)
			if errors.Is(err, syscall.ESRCH) {
				return os.ErrProcessDone
			}
			return err
		}
		// Keep desktop credentials and native GUI preload libraries out of renderers.
		worker.Env = make([]string, 0, 6)
		for _, key := range []string{"PATH", "HOME", "LANG", "LC_ALL", "XDG_CACHE_HOME", "TMPDIR"} {
			if value, ok := os.LookupEnv(key); ok {
				worker.Env = append(worker.Env, key+"="+value)
			}
		}
		output := &thumbnailOutput{cancel: cancel}
		worker.Stdout = output
		if err := worker.Run(); err != nil {
			if ctx.Err() != nil {
				return nil, ctx.Err()
			}
			if output.exceeded {
				return nil, ErrThumbnailTooLarge
			}
			var exit *exec.ExitError
			if errors.As(err, &exit) {
				switch exit.ExitCode() {
				case 2:
					return nil, ErrThumbnailUnsupported
				case 3:
					return nil, ErrThumbnailTooLarge
				}
			}
			return nil, ErrThumbnailUnavailable
		}
		image, err := jpeg.DecodeConfig(bytes.NewReader(output.data))
		if err != nil || image.Width <= 0 || image.Height <= 0 || image.Width > width || image.Height > height {
			return nil, ErrThumbnailUnsupported
		}
		return output.data, nil
	}
}

type thumbnailOutput struct {
	data     []byte
	exceeded bool
	cancel   context.CancelFunc
}

func (b *thumbnailOutput) Write(value []byte) (int, error) {
	if len(value) > maxNativeThumbnailBytes-len(b.data) {
		b.exceeded = true
		b.cancel()
		return 0, ErrThumbnailTooLarge
	}
	b.data = append(b.data, value...)
	return len(value), nil
}

func thumbnailMediaKind(info *model.FileInfo) string {
	mime := strings.ToLower(info.MimeType)
	switch {
	case strings.HasPrefix(mime, "image/"):
		return "image"
	case strings.HasPrefix(mime, "video/"):
		return "video"
	case strings.HasPrefix(mime, "audio/"):
		return "audio"
	case mime == "application/pdf":
		return "pdf"
	}
	extension := strings.ToLower(filepath.Ext(info.Name))
	for _, group := range []struct {
		kind       string
		extensions string
	}{
		{"image", ".jpg .jpeg .png .gif .webp .bmp .tif .tiff .ico .qoi"},
		{"video", ".mp4 .m4v .mov .mkv .webm .avi .mpeg .mpg .ts .mts .m2ts .ogv .wmv .flv .3gp"},
		{"audio", ".mp3 .m4a .m4b .aac .wav .flac .ogg .opus .wma .aif .aiff .ape .wv .amr"},
		{"pdf", ".pdf"},
	} {
		for _, candidate := range strings.Fields(group.extensions) {
			if extension == candidate {
				return group.kind
			}
		}
	}
	return ""
}
