package service

import (
	"bytes"
	"errors"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

var testPNG = []byte("\x89PNG\r\n\x1a\nwallpaper-bytes")

func newTestSettingsService() SettingsService {
	fs := filesystem.NewMemMapFS()
	_ = fs.MkdirAll("/data", 0755)
	return NewSettingsService(fs, SettingsServiceConfig{DataDir: "/data"})
}

func TestWallpaperLifecycle(t *testing.T) {
	svc := newTestSettingsService()

	if _, err := svc.GetWallpaper("owner"); !errors.Is(err, ErrWallpaperNotFound) {
		t.Fatalf("GetWallpaper before set = %v, want ErrWallpaperNotFound", err)
	}

	meta, err := svc.SetWallpaper("owner", testPNG, model.WallpaperDisplay{Mode: "cover", FrostedGlass: true}, "test-source")
	if err != nil {
		t.Fatalf("SetWallpaper: %v", err)
	}
	if meta.ContentType != "image/png" || meta.Mode != "cover" || !meta.FrostedGlass || meta.Source != "test-source" {
		t.Fatalf("unexpected meta %+v", meta)
	}

	image, _, err := svc.OpenWallpaper("owner")
	if err != nil || !bytes.Equal(image, testPNG) {
		t.Fatalf("OpenWallpaper = %q, %v", image, err)
	}

	if _, err := svc.GetWallpaper("someone-else"); !errors.Is(err, ErrWallpaperNotFound) {
		t.Fatalf("another user's wallpaper leaked: %v", err)
	}

	meta, err = svc.SetWallpaperDisplay("owner", model.WallpaperDisplay{Mode: "tile"})
	if err != nil || meta.Mode != "tile" || meta.FrostedGlass || meta.Source != "test-source" {
		t.Fatalf("SetWallpaperDisplay = %+v, %v", meta, err)
	}

	if err := svc.DeleteWallpaper("owner"); err != nil {
		t.Fatalf("DeleteWallpaper: %v", err)
	}
	if _, _, err := svc.OpenWallpaper("owner"); !errors.Is(err, ErrWallpaperNotFound) {
		t.Fatalf("OpenWallpaper after delete = %v", err)
	}
	if err := svc.DeleteWallpaper("owner"); err != nil {
		t.Fatalf("deleting a missing wallpaper should succeed: %v", err)
	}
}

func TestSetWallpaperRejectsInvalidInput(t *testing.T) {
	svc := newTestSettingsService()
	cases := map[string]struct {
		image []byte
		mode  string
	}{
		"not an image": {[]byte("<svg onload=alert(1)>"), "cover"},
		"unknown mode": {testPNG, "sideways"},
		"empty image":  {nil, "cover"},
		"html payload": {[]byte("<!DOCTYPE html><script></script>"), "cover"},
		"missing mode": {testPNG, ""},
	}
	for name, tc := range cases {
		if _, err := svc.SetWallpaper("owner", tc.image, model.WallpaperDisplay{Mode: tc.mode}, "test-source"); !errors.Is(err, ErrInvalidWallpaper) {
			t.Errorf("%s: err = %v, want ErrInvalidWallpaper", name, err)
		}
	}
	if _, err := svc.SetWallpaper("owner", testPNG, model.WallpaperDisplay{Mode: "cover"}, strings.Repeat("x", maxWallpaperSourceLength+1)); !errors.Is(err, ErrInvalidWallpaper) {
		t.Errorf("oversized source: err = %v, want ErrInvalidWallpaper", err)
	}
	if _, err := svc.SetWallpaperDisplay("owner", model.WallpaperDisplay{Mode: "cover"}); !errors.Is(err, ErrWallpaperNotFound) {
		t.Errorf("display update without a wallpaper = %v, want ErrWallpaperNotFound", err)
	}
}

func TestWallpaperUsernameCannotEscapeDataDir(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	_ = fs.MkdirAll("/data", 0755)
	svc := NewSettingsService(fs, SettingsServiceConfig{DataDir: "/data"})

	if _, err := svc.SetWallpaper("../../etc/passwd", testPNG, model.WallpaperDisplay{Mode: "cover"}, "test-source"); err != nil {
		t.Fatalf("SetWallpaper: %v", err)
	}
	if exists, _ := fs.Exists("/etc/passwd.img"); exists {
		t.Fatal("wallpaper written outside the data directory")
	}
}
