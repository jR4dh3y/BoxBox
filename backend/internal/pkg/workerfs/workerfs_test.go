//go:build linux

package workerfs

import (
	"errors"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type worker struct {
	dials atomic.Int32
	conns chan *fdconn.Conn
}

// startWorker serves the real filesystem in-process, one server per dial, as a
// worker process would.
func startWorker(t *testing.T) (*Client, *worker) {
	t.Helper()
	w := &worker{conns: make(chan *fdconn.Conn, 4096)}
	client := NewClient(func() (*fdconn.Conn, error) {
		w.dials.Add(1)
		local, peer, err := fdconn.Pair()
		if err != nil {
			return nil, err
		}
		remote, err := fdconn.New(peer)
		if err != nil {
			return nil, err
		}
		w.conns <- remote
		go func() { _ = Serve(remote, filesystem.NewOsFS()) }()
		return local, nil
	}, 0)
	t.Cleanup(func() { _ = client.Close() })
	return client, w
}

func TestClientRoundTripsFilesystemCalls(t *testing.T) {
	client, _ := startWorker(t)
	root := t.TempDir()
	path := filepath.Join(root, "a.txt")

	if err := client.WriteFile(path, []byte("hello"), 0o640); err != nil {
		t.Fatal(err)
	}
	data, err := client.ReadFile(path)
	if err != nil || string(data) != "hello" {
		t.Fatalf("ReadFile = %q, %v", data, err)
	}
	info, err := client.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Name() != "a.txt" || info.Size() != 5 || info.Mode().Perm() != 0o640&^currentUmask() || info.IsDir() {
		t.Fatalf("Stat = %s %d %v", info.Name(), info.Size(), info.Mode())
	}
	if err := client.MkdirAllWithin(root, "x/y", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("a.txt", filepath.Join(root, "link")); err != nil {
		t.Fatal(err)
	}
	if target, err := client.Readlink(filepath.Join(root, "link")); err != nil || target != "a.txt" {
		t.Fatalf("Readlink = %q, %v", target, err)
	}
	if resolved, err := client.EvalSymlinks(filepath.Join(root, "link")); err != nil || resolved != path {
		t.Fatalf("EvalSymlinks = %q, %v", resolved, err)
	}
	if info, err := client.Lstat(filepath.Join(root, "link")); err != nil || info.Mode()&fs.ModeSymlink == 0 {
		t.Fatalf("Lstat = %v, %v", info, err)
	}

	entries, err := client.ReadDir(root)
	if err != nil {
		t.Fatal(err)
	}
	names := []string{}
	for _, entry := range entries {
		entryInfo, err := entry.Info()
		if err != nil {
			t.Fatal(err)
		}
		names = append(names, entry.Name()+":"+entryInfo.Mode().Type().String())
	}
	if strings.Join(names, ",") != "a.txt:----------,link:L---------,x:d---------" {
		t.Fatalf("ReadDir = %v", names)
	}
	if _, truncated, err := client.ReadDirLimit(root, 2); err != nil || !truncated {
		t.Fatalf("ReadDirLimit truncated = %v, %v", truncated, err)
	}
	if infos, truncated, err := client.ReadDirFileInfoLimit(root, 10); err != nil || truncated || len(infos) != 3 {
		t.Fatalf("ReadDirFileInfoLimit = %d, %v, %v", len(infos), truncated, err)
	}
	if ok, err := client.IsDir(filepath.Join(root, "x")); err != nil || !ok {
		t.Fatalf("IsDir = %v, %v", ok, err)
	}
	if ok, err := client.Exists(filepath.Join(root, "missing")); err != nil || ok {
		t.Fatalf("Exists = %v, %v", ok, err)
	}
	if err := client.Chmod(path, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := client.Rename(path, filepath.Join(root, "b.txt")); err != nil {
		t.Fatal(err)
	}
	if err := client.RemoveAll(filepath.Join(root, "x")); err != nil {
		t.Fatal(err)
	}
	if err := client.Remove(filepath.Join(root, "b.txt")); err != nil {
		t.Fatal(err)
	}
}

func TestClientReturnsWorkerOpenedFiles(t *testing.T) {
	client, _ := startWorker(t)
	root := t.TempDir()
	path := filepath.Join(root, "data.bin")

	created, err := client.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o644)
	if err != nil {
		t.Fatal(err)
	}
	if created.Name() != path {
		t.Fatalf("Name = %q", created.Name())
	}
	if _, err := io.WriteString(created, "0123456789"); err != nil {
		t.Fatal(err)
	}
	if err := created.Close(); err != nil {
		t.Fatal(err)
	}

	opened, err := client.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	buffer := make([]byte, 4)
	if _, err := opened.ReadAt(buffer, 3); err != nil || string(buffer) != "3456" {
		t.Fatalf("ReadAt = %q, %v", buffer, err)
	}
	if _, ok := opened.(*os.File); !ok {
		t.Fatalf("Open returned %T, want the descriptor itself", opened)
	}
}

func TestClientKeepsErrorIdentity(t *testing.T) {
	client, _ := startWorker(t)
	root := t.TempDir()
	missing := filepath.Join(root, "missing")

	_, err := client.Stat(missing)
	var pathErr *fs.PathError
	if !errors.Is(err, fs.ErrNotExist) || !errors.Is(err, syscall.ENOENT) || !errors.As(err, &pathErr) || pathErr.Path != missing {
		t.Fatalf("Stat error = %#v", err)
	}
	if err := client.Mkdir(root, 0o755); !errors.Is(err, fs.ErrExist) {
		t.Fatalf("Mkdir error = %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "a"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "b"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := client.RenameNoReplace(filepath.Join(root, "a"), filepath.Join(root, "b")); !errors.Is(err, fs.ErrExist) {
		t.Fatalf("RenameNoReplace error = %v", err)
	}
	if _, err := client.ReadDir(filepath.Join(root, "a")); !errors.Is(err, syscall.ENOTDIR) {
		t.Fatalf("ReadDir error = %v", err)
	}
	if _, err := client.Stat(root + "/../etc"); !errors.Is(err, filesystem.ErrInvalidPath) {
		t.Fatalf("traversal error = %v", err)
	}
}

func TestClientRedialsAfterWorkerExits(t *testing.T) {
	client, w := startWorker(t)
	root := t.TempDir()
	if _, err := client.Stat(root); err != nil {
		t.Fatal(err)
	}
	opened, err := client.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	// The worker exits: calls in flight fail and are not replayed.
	(<-w.conns).Close()
	deadline := time.Now().Add(2 * time.Second)
	for {
		_, err := client.Stat(root)
		if err == nil && w.dials.Load() == 2 {
			break
		}
		if err != nil && !errors.Is(err, filesystem.ErrWorkerUnavailable) {
			t.Fatalf("Stat after worker exit = %v", err)
		}
		if time.Now().After(deadline) {
			t.Fatalf("client did not redial; dials = %d", w.dials.Load())
		}
		time.Sleep(10 * time.Millisecond)
	}
	// A file opened by the old worker stays usable.
	if _, err := opened.Stat(); err != nil {
		t.Fatalf("file from exited worker: %v", err)
	}
}

func TestClientClosesIdleWorker(t *testing.T) {
	client, w := startWorker(t)
	client.idle = 20 * time.Millisecond
	root := t.TempDir()
	if _, err := client.Stat(root); err != nil {
		t.Fatal(err)
	}
	remote := <-w.conns
	if _, err := remote.Receive(); err == nil {
		t.Fatal("worker connection stayed open after the idle timeout")
	}
	if _, err := client.Stat(root); err != nil {
		t.Fatal(err)
	}
	if w.dials.Load() != 2 {
		t.Fatalf("dials = %d, want 2", w.dials.Load())
	}
}

func TestClientHandlesLargeDirectories(t *testing.T) {
	client, _ := startWorker(t)
	root := t.TempDir()
	for i := range 3000 {
		name := filepath.Join(root, strings.Repeat("n", 60)+string(rune('a'+i%26))+time.Duration(i).String())
		if err := os.WriteFile(name, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	entries, err := client.ReadDir(root)
	if err != nil || len(entries) != 3000 {
		t.Fatalf("ReadDir = %d entries, %v", len(entries), err)
	}
}

func currentUmask() fs.FileMode {
	mask := syscall.Umask(0)
	syscall.Umask(mask)
	return fs.FileMode(mask)
}

// Constant idle closes must never surface as failed calls. This guards the
// reconnect path; the narrow register-versus-close race it was written for
// does not reproduce reliably here.
func TestClientIdleChurnNeverFailsACall(t *testing.T) {
	client, _ := startWorker(t)
	client.idle = time.Microsecond
	root := t.TempDir()
	var wg sync.WaitGroup
	for range 8 {
		wg.Go(func() {
			for range 200 {
				if _, err := client.Stat(root); err != nil {
					t.Errorf("Stat during idle churn: %v", err)
					return
				}
			}
		})
	}
	wg.Wait()
}
