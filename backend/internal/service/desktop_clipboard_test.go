package service

import (
	"context"
	"errors"
	"net/url"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type fakeClipboardAdapter struct {
	formats map[string][]byte
	write   map[string][]byte
	writeOp DesktopClipboardOperation
	cleared bool
	err     error
}

func (a *fakeClipboardAdapter) Read(context.Context) (map[string][]byte, error) {
	return cloneClipboardFormats(a.formats), a.err
}

func (a *fakeClipboardAdapter) Write(_ context.Context, operation DesktopClipboardOperation, formats map[string][]byte) error {
	a.writeOp = operation
	a.write = cloneClipboardFormats(formats)
	return a.err
}

func (a *fakeClipboardAdapter) Clear(context.Context) error {
	a.cleared = true
	return a.err
}

func cloneClipboardFormats(formats map[string][]byte) map[string][]byte {
	if formats == nil {
		return nil
	}
	clone := make(map[string][]byte, len(formats))
	for mime, data := range formats {
		clone[mime] = append([]byte(nil), data...)
	}
	return clone
}

func TestDesktopClipboardCodec(t *testing.T) {
	uri := (&url.URL{Scheme: "file", Path: "/tmp/hello world #1.txt"}).String()
	for _, operation := range []DesktopClipboardOperation{DesktopClipboardCopy, DesktopClipboardCut} {
		formats, err := encodeClipboardFormats(operation, []clipboardFile{{uri: uri}})
		if err != nil {
			t.Fatal(err)
		}
		wantGnome := string(operation) + "\n" + uri + "\n"
		if got := string(formats[gnomeCopiedFilesMIME]); got != wantGnome {
			t.Fatalf("gnome format = %q, want %q", got, wantGnome)
		}
		if got := string(formats[uriListMIME]); got != uri+"\r\n" {
			t.Fatalf("URI list = %q", got)
		}
		wantKDE := "0"
		if operation == DesktopClipboardCut {
			wantKDE = "1"
		}
		if got := string(formats[kdeCutSelectionMIME]); got != wantKDE {
			t.Fatalf("KDE cut marker = %q, want %q", got, wantKDE)
		}
		gotOperation, gotURIs, err := decodeClipboardFormats(formats)
		if err != nil || gotOperation != operation || len(gotURIs) != 1 || gotURIs[0] != uri {
			t.Fatalf("decode = (%q, %q, %v)", gotOperation, gotURIs, err)
		}
	}
}

func TestDesktopClipboardDecodeURIListAndRejectRemoteHosts(t *testing.T) {
	localURI := "file:///home/example/Documents/a%20file.txt"
	operation, uris, err := decodeClipboardFormats(map[string][]byte{
		uriListMIME:         []byte("# copied from a file manager\r\n" + localURI + "\r\n"),
		kdeCutSelectionMIME: []byte("1"),
	})
	if err != nil || operation != DesktopClipboardCut || len(uris) != 1 || uris[0] != localURI {
		t.Fatalf("decode = (%q, %q, %v)", operation, uris, err)
	}
	if _, _, err := decodeClipboardFormats(map[string][]byte{
		uriListMIME: []byte("file://fileserver/share/report.txt\r\n"),
	}); err != nil {
		t.Fatalf("format decoding should preserve URI for confinement validation: %v", err)
	}
}

func TestDesktopClipboardSetResolvesPathsBeforePublishing(t *testing.T) {
	root := t.TempDir()
	file := filepath.Join(root, "report #1.txt")
	if err := os.WriteFile(file, []byte("report"), 0600); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.WriteFile(filepath.Join(outside, "secret.txt"), []byte("secret"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "escape")); err != nil {
		t.Fatal(err)
	}

	adapter := &fakeClipboardAdapter{}
	service := &desktopClipboardService{
		fs:      filesystem.NewOsFS(),
		mounts:  newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{{Name: "home", Path: root}})),
		adapter: adapter,
	}
	if err := service.Set(context.Background(), DesktopClipboardCopy, []string{"home/report #1.txt"}); err != nil {
		t.Fatal(err)
	}
	uri := (&url.URL{Scheme: "file", Path: file}).String()
	if got := string(adapter.write[gnomeCopiedFilesMIME]); got != "copy\n"+uri+"\n" {
		t.Fatalf("published gnome format = %q", got)
	}
	if err := service.Set(context.Background(), DesktopClipboardCut, []string{"home/escape/secret.txt"}); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("symlink escape error = %v", err)
	}
	if adapter.writeOp != DesktopClipboardCopy {
		t.Fatalf("rejected path reached clipboard adapter with operation %q", adapter.writeOp)
	}
}

func TestDesktopClipboardReadsOnlyLocalFilesInsideMostSpecificMount(t *testing.T) {
	root := t.TempDir()
	downloads := filepath.Join(root, "Downloads")
	if err := os.Mkdir(downloads, 0700); err != nil {
		t.Fatal(err)
	}
	file := filepath.Join(downloads, "notes.txt")
	if err := os.WriteFile(file, []byte("notes"), 0600); err != nil {
		t.Fatal(err)
	}
	uri := (&url.URL{Scheme: "file", Path: file}).String()
	adapter := &fakeClipboardAdapter{formats: map[string][]byte{
		uriListMIME: []byte(uri + "\r\n"),
	}}
	fsys := filesystem.NewOsFS()
	service := &desktopClipboardService{
		fs: fsys,
		mounts: newMounts(fsys, fixedMounts([]model.MountPoint{
			{Name: "home", Path: root},
			{Name: "downloads", Path: downloads},
		})),
		adapter: adapter,
	}
	contents, err := service.Get(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if contents == nil || contents.Operation != DesktopClipboardCopy || len(contents.Items) != 1 {
		t.Fatalf("clipboard = %#v", contents)
	}
	item := contents.Items[0]
	if item.Path != "downloads/notes.txt" || item.Name != "notes.txt" || item.IsDir {
		t.Fatalf("item = %#v", item)
	}

	remote := &desktopClipboardService{
		fs:     fsys,
		mounts: newMounts(fsys, fixedMounts([]model.MountPoint{{Name: "home", Path: root}})),
		adapter: &fakeClipboardAdapter{formats: map[string][]byte{
			uriListMIME: []byte("file://other-host" + file + "\r\n"),
		}},
	}
	contents, err = remote.Get(context.Background())
	if err != nil || contents != nil {
		t.Fatalf("remote clipboard = %#v, %v; want null", contents, err)
	}
}

func TestDesktopClipboardClearsOnlyTheCurrentCutSelection(t *testing.T) {
	root := t.TempDir()
	file := filepath.Join(root, "source.txt")
	if err := os.WriteFile(file, []byte("source"), 0600); err != nil {
		t.Fatal(err)
	}
	uri := (&url.URL{Scheme: "file", Path: file}).String()
	formats := map[string][]byte{gnomeCopiedFilesMIME: []byte("cut\n" + uri + "\n")}
	adapter := &fakeClipboardAdapter{formats: formats}
	fsys := filesystem.NewOsFS()
	service := &desktopClipboardService{
		fs: fsys, mounts: newMounts(fsys, fixedMounts([]model.MountPoint{{Name: "home", Path: root}})), adapter: adapter,
	}
	if err := service.ClearCut(context.Background(), []string{"home/source.txt"}); err != nil {
		t.Fatal(err)
	}
	if !adapter.cleared {
		t.Fatal("matching cut selection was not cleared")
	}

	adapter.cleared = false
	adapter.formats[gnomeCopiedFilesMIME] = []byte("copy\n" + uri + "\n")
	if err := service.ClearCut(context.Background(), []string{"home/source.txt"}); !errors.Is(err, ErrDesktopClipboardChanged) {
		t.Fatalf("copy selection clear error = %v", err)
	}
	if adapter.cleared {
		t.Fatal("copy selection was cleared as if it were a cut")
	}
}

func TestDesktopClipboardClearSupportsSourceAlreadyMoved(t *testing.T) {
	root := t.TempDir()
	file := filepath.Join(root, "source.txt")
	if err := os.WriteFile(file, []byte("source"), 0600); err != nil {
		t.Fatal(err)
	}
	uri := (&url.URL{Scheme: "file", Path: file}).String()
	adapter := &fakeClipboardAdapter{formats: map[string][]byte{gnomeCopiedFilesMIME: []byte("cut\n" + uri + "\n")}}
	fsys := filesystem.NewOsFS()
	service := &desktopClipboardService{
		fs: fsys, mounts: newMounts(fsys, fixedMounts([]model.MountPoint{{Name: "home", Path: root}})), adapter: adapter,
	}
	if err := os.Rename(file, filepath.Join(root, "moved.txt")); err != nil {
		t.Fatal(err)
	}
	if err := service.ClearCut(context.Background(), []string{"home/source.txt"}); err != nil {
		t.Fatalf("clear after successful move: %v", err)
	}
}

func TestDesktopClipboardRejectsURIThatResolvesToAnotherFile(t *testing.T) {
	root := t.TempDir()
	file := filepath.Join(root, "report%20.txt")
	for _, name := range []string{"report%20.txt", "report .txt"} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(name), 0600); err != nil {
			t.Fatal(err)
		}
	}
	uri := (&url.URL{Scheme: "file", Path: file}).String()
	adapter := &fakeClipboardAdapter{formats: map[string][]byte{gnomeCopiedFilesMIME: []byte("cut\n" + uri + "\n")}}
	fsys := filesystem.NewOsFS()
	service := &desktopClipboardService{
		fs: fsys, mounts: newMounts(fsys, fixedMounts([]model.MountPoint{{Name: "home", Path: root}})), adapter: adapter,
	}
	if err := service.Set(context.Background(), DesktopClipboardCut, []string{"home/report%20.txt"}); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("ambiguous clipboard publication error = %v", err)
	}
	if adapter.write != nil {
		t.Fatal("ambiguous path reached the clipboard adapter")
	}
	contents, err := service.Get(context.Background())
	if err != nil || contents != nil {
		t.Fatalf("ambiguous clipboard = %#v, %v; want no file", contents, err)
	}
	if err := os.Remove(file); err != nil {
		t.Fatal(err)
	}
	if err := service.ClearCut(context.Background(), []string{"home/report%20.txt"}); !errors.Is(err, ErrDesktopClipboardChanged) {
		t.Fatalf("ambiguous cut clear error = %v", err)
	}
	if adapter.cleared {
		t.Fatal("ambiguous cut selection was cleared")
	}
}
