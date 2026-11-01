package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/godbus/dbus/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
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
