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
	// exits receives Serve's result each time a worker ends.
	exits chan error
}

// startWorker serves the real filesystem in-process, one server per dial, as a
// worker process would.
func startWorker(t *testing.T) (*Client, *worker) {
	t.Helper()
	w := &worker{conns: make(chan *fdconn.Conn, 4096), exits: make(chan error, 4096)}
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
		go func() { w.exits <- Serve(remote, filesystem.NewOsFS()) }()
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
	select {
	case err := <-w.exits:
		if err != nil {
			t.Fatalf("worker exited with %v, want a clean exit", err)
		}
	case <-time.After(5 * time.Second):
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

// Linux names are bytes; a copy walks back into the names ReadDir returned.
func TestClientKeepsNonUTF8NamesExact(t *testing.T) {
	client, _ := startWorker(t)
	root := t.TempDir()
	name := "caf\xe9 \xff.txt"
	if err := os.WriteFile(filepath.Join(root, name), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	entries, err := client.ReadDir(root)
	if err != nil || len(entries) != 1 || entries[0].Name() != name {
		t.Fatalf("ReadDir = %v, %v", entries, err)
	}
	info, err := client.Stat(filepath.Join(root, entries[0].Name()))
	if err != nil || info.Name() != name {
		t.Fatalf("Stat of listed name = %v, %v", info, err)
	}
	_, err = client.Stat(filepath.Join(root, "gone\xfe"))
	var pathErr *fs.PathError
	if !errors.As(err, &pathErr) || pathErr.Path != filepath.Join(root, "gone\xfe") {
		t.Fatalf("error path = %v", err)
	}
}

// A folder readable but not searchable lists names whose Info fails; a copy
// must see that failure, not an empty folder.
func TestClientReturnsEntryInfoErrors(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("root bypasses directory permissions")
	}
	client, _ := startWorker(t)
	root := t.TempDir()
	locked := filepath.Join(root, "locked")
	if err := os.Mkdir(locked, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(locked, "a.txt"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(locked, 0o444); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = os.Chmod(locked, 0o755) })

	entries, err := client.ReadDir(locked)
	if err != nil || len(entries) != 1 || entries[0].Name() != "a.txt" {
		t.Fatalf("ReadDir = %v, %v", entries, err)
	}
	if _, err := entries[0].Info(); !errors.Is(err, fs.ErrPermission) {
		t.Fatalf("Info = %v, want a permission error", err)
	}
}

func TestClientRejectsOversizedMessagesWithoutHanging(t *testing.T) {
	client, w := startWorker(t)
	root := t.TempDir()
	big := filepath.Join(root, "big")
	// Base64 in the reply grows this past the message limit.
	file, err := os.Create(big)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(fdconn.MaxMessageSize * 3 / 4); err != nil {
		t.Fatal(err)
	}
	_ = file.Close()

	done := make(chan error, 1)
	go func() {
		_, err := client.ReadFile(big)
		done <- err
	}()
	select {
	case err := <-done:
		if !errors.Is(err, fdconn.ErrMessageTooLarge) {
			t.Fatalf("ReadFile = %v, want %v", err, fdconn.ErrMessageTooLarge)
		}
	case <-time.After(30 * time.Second):
		t.Fatal("oversized reply never finished")
	}
	if err := client.WriteFile(filepath.Join(root, "out"), make([]byte, fdconn.MaxMessageSize+1), 0o644); !errors.Is(err, fdconn.ErrMessageTooLarge) {
		t.Fatalf("WriteFile = %v, want %v", err, fdconn.ErrMessageTooLarge)
	}
	if _, err := client.Stat(root); err != nil {
		t.Fatal(err)
	}
	if dials := w.dials.Load(); dials != 1 {
		t.Fatalf("worker dialed %d times; an oversized message must not end the session", dials)
	}
}

// A local ReadDir hands every Info error to its caller; a move must abort on
// an entry that vanished rather than delete it uncopied.
func TestEntryInfosKeepsEveryInfoError(t *testing.T) {
	wire := entryInfos([]fs.DirEntry{
		failingEntry{name: "gone", err: &fs.PathError{Op: "lstat", Path: "/x/gone", Err: syscall.ENOENT}},
		failingEntry{name: "locked", err: &fs.PathError{Op: "lstat", Path: "/x/locked", Err: syscall.EACCES}},
	})
	listed := entries(wire)
	if len(listed) != 2 {
		t.Fatalf("entries = %d, want 2", len(listed))
	}
	if _, err := listed[0].Info(); listed[0].Name() != "gone" || !errors.Is(err, fs.ErrNotExist) {
		t.Fatalf("vanished entry = %q, %v", listed[0].Name(), err)
	}
	if _, err := listed[1].Info(); !errors.Is(err, fs.ErrPermission) {
		t.Fatalf("locked entry Info = %v", err)
	}
}

type failingEntry struct {
	name string
	err  error
}

func (e failingEntry) Name() string               { return e.name }
func (e failingEntry) IsDir() bool                { return false }
func (e failingEntry) Type() fs.FileMode          { return 0 }
func (e failingEntry) Info() (fs.FileInfo, error) { return nil, e.err }

func TestClientKeepsModTimesOutsideTheNanosecondRange(t *testing.T) {
	client, _ := startWorker(t)
	path := filepath.Join(t.TempDir(), "future.txt")
	if err := os.WriteFile(path, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	future := time.Date(2300, 1, 1, 0, 0, 0, 123456789, time.UTC)
	if err := os.Chtimes(path, future, future); err != nil {
		t.Fatal(err)
	}
	local, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	remote, err := client.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !remote.ModTime().Equal(local.ModTime()) {
		t.Fatalf("ModTime = %v, want %v", remote.ModTime(), local.ModTime())
	}
}
