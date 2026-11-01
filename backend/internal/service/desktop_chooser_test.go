package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/godbus/dbus/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/spf13/afero"
)

func chooserFixture(t *testing.T) (*DesktopChooser, string) {
	t.Helper()
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "Report.txt"), []byte("Report"), 0600); err != nil {
		t.Fatal(err)
	}
	return NewDesktopChooser(filesystem.NewOsFS(), []model.MountPoint{{Name: "fixture", Path: root}}), root
}
func TestChooserConfinedSelectionsAndReplacement(t *testing.T) {
	c, root := chooserFixture(t)
	ctx := context.Background()
	request, result := c.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Name: "Text", Rules: []ChooserRule{{Kind: 0, Value: "*.txt"}}}}})
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/../outside"}}); err == nil {
		t.Fatal("traversal accepted")
	}
	outside := t.TempDir()
	os.WriteFile(filepath.Join(outside, "secret.txt"), []byte("private"), 0600)
	os.Symlink(filepath.Join(outside, "secret.txt"), filepath.Join(root, "escape.txt"))
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/escape.txt"}}); err == nil {
		t.Fatal("escaping symlink accepted")
	}
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/Report.txt"}}); err != nil {
		t.Fatal(err)
	}
	if got := <-result; got.Code != 0 || len(got.URIs) != 1 || got.Filter.Name != "Text" {
		t.Fatalf("result %#v", got)
	}
	request, result = c.Begin(ChooserRequest{Kind: "save"})
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/Report.txt"}}); !errors.Is(err, ErrChooserReplace) {
		t.Fatalf("missing replacement confirmation: %v", err)
	}
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/Report.txt"}, Replace: true}); err != nil {
		t.Fatal(err)
	}
	<-result
	data, _ := os.ReadFile(filepath.Join(root, "Report.txt"))
	if string(data) != "Report" {
		t.Fatal("chooser wrote destination")
	}
}
func TestChooserCancelDirectoryAndSaveFiles(t *testing.T) {
	c, _ := chooserFixture(t)
	ctx := context.Background()
	request, result := c.Begin(ChooserRequest{Kind: "open", Directory: true})
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/Report.txt"}}); err == nil {
		t.Fatal("file accepted as directory")
	}
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture"}}); err != nil {
		t.Fatal(err)
	}
	<-result
	request, result = c.Begin(ChooserRequest{Kind: "save-files", Files: []string{"One.txt", "Two.txt"}})
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture"}}); err != nil {
		t.Fatal(err)
	}
	if got := <-result; len(got.URIs) != 2 {
		t.Fatal(got)
	}
	request, result = c.Begin(ChooserRequest{Kind: "open"})
	c.Cancel(request.ID)
	c.Cancel(request.ID)
	if got := <-result; got.Code != 1 {
		t.Fatal(got)
	}
	if _, err := c.Get(request.ID); !errors.Is(err, ErrChooserClosed) {
		t.Fatal(err)
	}
}
func TestPortalOptionDecoding(t *testing.T) {
	c, root := chooserFixture(t)
	p := chooserPortal{chooser: c}
	filter := ChooserFilter{Name: "Text", Rules: []ChooserRule{{Kind: 0, Value: "*.txt"}}}
	r, err := p.parse("open", "fixture.app", "Upload", map[string]dbus.Variant{
		"current_folder": dbus.MakeVariant(append([]byte(root), 0)), "filters": dbus.MakeVariant([]ChooserFilter{filter}), "current_filter": dbus.MakeVariant(filter), "multiple": dbus.MakeVariant(true),
	})
	if err != nil || r.Folder != "fixture" || !r.Multiple || len(r.Filters) != 1 {
		t.Fatalf("%#v %v", r, err)
	}
	for _, options := range []map[string]dbus.Variant{
		{"current_folder": dbus.MakeVariant([]byte("/not-terminated"))},
		{"multiple": dbus.MakeVariant("yes")},
		{"current_name": dbus.MakeVariant("../escape")},
	} {
		if _, err := p.parse("save", "", "", options); err == nil {
			t.Fatalf("accepted invalid options %#v", options)
		}
	}
}

func TestChooserMIMEFiltersUseConfiguredFilesystem(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/fixture", 0700); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/fixture/Report", []byte("A plain report\n"), 0600); err != nil {
		t.Fatal(err)
	}
	c := NewDesktopChooser(fsys, []model.MountPoint{{Name: "fixture", Path: "/fixture"}})
	for _, tc := range []struct {
		mime string
		want bool
	}{{"text/plain", true}, {"text/*", true}, {"image/*", false}} {
		t.Run(tc.mime, func(t *testing.T) {
			request, result := c.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Name: tc.mime, Rules: []ChooserRule{{Kind: 1, Value: tc.mime}}}}})
			defer c.Cancel(request.ID)
			listing, err := c.Directory(context.Background(), request.ID, "fixture")
			if err != nil || len(listing.Items) != 1 || listing.Items[0].MimeType != "text/plain; charset=utf-8" {
				t.Fatalf("extensionless file MIME listing = %#v, %v", listing, err)
			}
			err = c.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{"fixture/Report"}})
			if tc.want {
				if err != nil {
					t.Fatal(err)
				}
				if got := <-result; got.Code != 0 || len(got.URIs) != 1 {
					t.Fatalf("result %#v", got)
				}
			} else if !errors.Is(err, ErrInvalidOperation) {
				t.Fatalf("MIME mismatch error = %v", err)
			}
		})
	}
}

type chooserSwapFS struct {
	filesystem.FS
	path, outside string
	read          bool
}

func (f *chooserSwapFS) Open(path string) (afero.File, error) {
	if path == f.path {
		if err := os.Remove(path); err != nil {
			return nil, err
		}
		if err := os.Symlink(f.outside, path); err != nil {
			return nil, err
		}
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	return &chooserReadTracker{File: file, read: &f.read}, nil
}

type chooserReadTracker struct {
	*os.File
	read *bool
}

func (f *chooserReadTracker) Read(data []byte) (int, error) {
	*f.read = true
	return f.File.Read(data)
}

func TestChooserRejectsSymlinkSwapBeforeMIMESniff(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("opened descriptor confinement uses procfs on Linux")
	}
	root := t.TempDir()
	path := filepath.Join(root, "Report")
	outside := filepath.Join(t.TempDir(), "Secret")
	for _, file := range []string{path, outside} {
		if err := os.WriteFile(file, []byte("private"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	fsys := &chooserSwapFS{FS: filesystem.NewOsFS(), path: path, outside: outside}
	c := NewDesktopChooser(fsys, []model.MountPoint{{Name: "fixture", Path: root}})
	request, result := c.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Name: "Text", Rules: []ChooserRule{{Kind: 1, Value: "text/plain"}}}}})
	defer c.Cancel(request.ID)
	if err := c.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{"fixture/Report"}}); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("symlink swap error = %v", err)
	}
	if fsys.read {
		t.Fatal("read outside the mount before verifying the opened descriptor")
	}
	select {
	case got := <-result:
		t.Fatalf("rejected selection completed request: %#v", got)
	default:
	}
}

func TestChooserPortalGlobNegationSelection(t *testing.T) {
	c, root := chooserFixture(t)
	for _, name := range []string{"alpha", "beta"} {
		if err := os.WriteFile(filepath.Join(root, name), []byte("text"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	request, result := c.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Rules: []ChooserRule{{Kind: 0, Value: "[!a]*"}}}}})
	if err := c.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{"fixture/alpha"}}); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("negated class accepted alpha: %v", err)
	}
	if err := c.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{"fixture/beta"}}); err != nil {
		t.Fatal(err)
	}
	<-result
}

func TestChooserExtensionlessPNGVisibleAndSelectable(t *testing.T) {
	c, root := chooserFixture(t)
	if err := os.WriteFile(filepath.Join(root, "logo"), []byte("\x89PNG\r\n\x1a\n"), 0600); err != nil {
		t.Fatal(err)
	}
	request, result := c.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Rules: []ChooserRule{{Kind: 1, Value: "image/png"}}}}})
	listing, err := c.Directory(context.Background(), request.ID, "fixture")
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, item := range listing.Items {
		if item.Name == "logo" {
			found = item.MimeType == "image/png"
		}
	}
	if !found {
		t.Fatalf("missing extensionless PNG MIME: %#v", listing.Items)
	}
	if err := c.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{"fixture/logo"}}); err != nil {
		t.Fatal(err)
	}
	<-result
	if _, err := c.Directory(context.Background(), request.ID, "fixture"); !errors.Is(err, ErrChooserClosed) {
		t.Fatalf("closed picker accepted directory request: %v", err)
	}
}
