package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestOpenFileRejectsSymlinkEscapingMount(t *testing.T) {
	root := t.TempDir()
	mountRoot := filepath.Join(root, "mount")
	outsideRoot := filepath.Join(root, "outside")
	if err := os.MkdirAll(mountRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(outsideRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	secret := filepath.Join(outsideRoot, "secret.txt")
	if err := os.WriteFile(secret, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(secret, filepath.Join(mountRoot, "escape.txt")); err != nil {
		t.Fatal(err)
	}

	files := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "home", Path: mountRoot,
	}}})
	opened, _, err := files.OpenFile(context.Background(), "home/escape.txt")
	if opened != nil {
		_ = opened.Close()
	}
	if !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("symlink escape error = %v, want permission denied", err)
	}
}
