package service

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestDesktopPreferencesSurviveNewBackend(t *testing.T) {
	directory := t.TempDir()
	store := NewDesktopPreferences(filesystem.NewOsFS(), directory)
	empty, err := store.Get()
	if err != nil || string(empty) != "null" {
		t.Fatalf("empty preferences: %s, %v", empty, err)
	}
	value := json.RawMessage(`{"favoriteFolders":[{"name":"Photos","path":"home/Photos"}],"compactMode":true}`)
	if err := store.Set(value); err != nil {
		t.Fatal(err)
	}
	restarted := NewDesktopPreferences(filesystem.NewOsFS(), directory)
	got, err := restarted.Get()
	if err != nil || string(got) != string(value) {
		t.Fatalf("restarted preferences: %s, %v", got, err)
	}
	info, err := os.Stat(filepath.Join(directory, "desktop", "preferences.json"))
	if err != nil || info.Mode().Perm() != 0600 {
		t.Fatalf("private preferences permissions: %v, %v", info, err)
	}
	if err := store.Set(json.RawMessage(`[]`)); err == nil {
		t.Fatal("accepted non-object preferences")
	}
	got, err = restarted.Get()
	if err != nil || string(got) != string(value) {
		t.Fatal("invalid write replaced saved preferences")
	}
}
