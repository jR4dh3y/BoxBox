package service

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestDesktopOpenerConfinesPaths(t *testing.T) {
	root := t.TempDir()
	file := filepath.Join(root, "hello.txt")
	if err := os.WriteFile(file, []byte("hello"), 0600); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "escape")); err != nil {
		t.Fatal(err)
	}
	var opened string
	open := NewDesktopOpener(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, func(path string) error {
		opened = path
		return nil
	})
	if err := open("home/hello.txt"); err != nil {
		t.Fatal(err)
	}
	if opened != file {
		t.Fatalf("opened = %q, want %q", opened, file)
	}
	for _, path := range []string{"home/../secret", "home/escape", "other/hello.txt", "home/missing", ""} {
		opened = ""
		if err := open(path); err == nil {
			t.Fatalf("accepted %q", path)
		}
		if opened != "" {
			t.Fatalf("launched opener for %q", path)
		}
	}
}
