package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestChooserRoutingRestoresTheOldRoute(t *testing.T) {
	original := "[preferred]\ndefault=gtk;wlr\norg.freedesktop.impl.portal.ScreenCast=wlr\norg.freedesktop.impl.portal.FileChooser=gtk\n# keep me\n"
	enabled, err := chooserRouting(original, true)
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"default=gtk;wlr", "ScreenCast=wlr", "FileChooser=boxbox", "# Previous: org.freedesktop.impl.portal.FileChooser=gtk"} {
		if !strings.Contains(enabled, want) {
			t.Errorf("enabled routing lacks %q:\n%s", want, enabled)
		}
	}
	if again, _ := chooserRouting(enabled, true); again != enabled {
		t.Errorf("enabling twice changed the file:\n%s", again)
	}
	if disabled, _ := chooserRouting(enabled, false); disabled != original {
		t.Errorf("disable = %q, want %q", disabled, original)
	}
}

func TestChooserRoutingAppendsToPreferredAndRefusesEdits(t *testing.T) {
	original := "[preferred]\ndefault=gtk\n[other]\nkey=value\n"
	enabled, err := chooserRouting(original, true)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Index(enabled, "FileChooser=boxbox") > strings.Index(enabled, "[other]") {
		t.Errorf("routing left the preferred group:\n%s", enabled)
	}
	if disabled, _ := chooserRouting(enabled, false); disabled != original {
		t.Errorf("disable = %q, want %q", disabled, original)
	}
	if _, err := chooserRouting(strings.Replace(enabled, "FileChooser=boxbox", "FileChooser=kde", 1), false); err == nil {
		t.Error("an edited managed line must be refused")
	}
}

func TestInstallChooserPortal(t *testing.T) {
	root := t.TempDir()
	metadata := filepath.Join(root, "metadata")
	if err := os.MkdirAll(metadata, 0o755); err != nil {
		t.Fatal(err)
	}
	env := map[string]string{
		"HOME": root, "XDG_CONFIG_HOME": filepath.Join(root, "config"), "XDG_DATA_HOME": filepath.Join(root, "data"),
		"XDG_CONFIG_DIRS": filepath.Join(root, "config-dirs"), "XDG_DATA_DIRS": filepath.Join(root, "data-dirs"),
		"XDG_CURRENT_DESKTOP": "BoxBoxTest", "XDG_DESKTOP_PORTAL_DIR": metadata,
	}
	getenv := func(key string) string { return env[key] }
	launcher := filepath.Join(root, "Box Box", "boxbox")
	if _, err := installChooserPortal(launcher, true, getenv); err == nil {
		t.Fatal("enabling without boxbox.portal metadata must fail")
	}
	if err := os.WriteFile(filepath.Join(metadata, "boxbox.portal"), []byte("[portal]\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	path, err := installChooserPortal(launcher, true, getenv)
	if err != nil {
		t.Fatal(err)
	}
	if want := filepath.Join(root, "config/xdg-desktop-portal/boxboxtest-portals.conf"); path != want {
		t.Errorf("routing written to %s, want %s", path, want)
	}
	service, err := os.ReadFile(filepath.Join(root, "data/dbus-1/services", portalBus+".service"))
	if err != nil || !strings.Contains(string(service), `Exec="`+launcher+`" --portal`) {
		t.Errorf("service = %q, %v", service, err)
	}
	if _, err := installChooserPortal(launcher, false, getenv); err != nil {
		t.Fatal(err)
	}
	routed, _ := os.ReadFile(path)
	if strings.Contains(string(routed), "boxbox") {
		t.Errorf("disable left the route:\n%s", routed)
	}
}
