package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestPropertiesWalksFoldersAndTotalsTheSelection(t *testing.T) {
	memory := filesystem.NewMemMapFS()
	for path, content := range map[string]string{
		"/data/media/album/a.jpg":      "aaaa",
		"/data/media/album/deep/b.jpg": "bb",
		"/data/media/album/.hidden":    "h",
		"/data/media/notes.txt":        "notes",
	} {
		if err := memory.MkdirAll(filepath.Dir(path), 0755); err != nil {
			t.Fatal(err)
		}
		if err := memory.WriteFile(path, []byte(content), 0644); err != nil {
			t.Fatal(err)
		}
	}
	svc := NewFileService(memory, FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}}})
	props, err := svc.Properties(context.Background(), []string{"media/album", "media/notes.txt"})
	if err != nil {
		t.Fatal(err)
	}
	// album and album/deep; a.jpg, b.jpg, .hidden and notes.txt.
	if props.Folders != 2 || props.Files != 4 || props.Bytes != 12 || !props.Complete {
		t.Fatalf("totals = %+v", props)
	}
	if len(props.Items) != 2 || props.Items[0].Path != "media/album" {
		t.Fatalf("items = %+v", props.Items)
	}
}

func TestPropertiesRefusesNoPathsAndPathsOutsideTheMounts(t *testing.T) {
	svc := NewFileService(filesystem.NewMemMapFS(), FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}}})
	if _, err := svc.Properties(context.Background(), nil); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("no paths: %v", err)
	}
	if _, err := svc.Properties(context.Background(), []string{"media/../../etc"}); err == nil {
		t.Fatal("a path outside the mounts was described")
	}
}

func TestPropertiesNamesTheOwnerOnLinux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("owners come from a Linux stat")
	}
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "mine.txt"), []byte("x"), 0644); err != nil {
		t.Fatal(err)
	}
	svc := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: root}}})
	props, err := svc.Properties(context.Background(), []string{"media/mine.txt"})
	if err != nil {
		t.Fatal(err)
	}
	item := props.Items[0]
	if item.Owner == "" || item.Group == "" || item.Accessed == nil || item.Changed == nil {
		t.Fatalf("owner and dates missing: %+v", item)
	}
}

func TestPropertiesDescribesASelectedLinkNotItsTarget(t *testing.T) {
	_, _, mountList := symlinkFixture(t)
	svc := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: mountList})
	props, err := svc.Properties(context.Background(), []string{"home/shortcut"})
	if err != nil {
		t.Fatal(err)
	}
	// The link is one file; the folder it points at is not walked.
	if props.Files != 1 || props.Folders != 0 || !props.Complete {
		t.Fatalf("totals = %+v", props)
	}
}

func TestPropertiesAreIncompleteWhenAFolderCannotBeRead(t *testing.T) {
	if runtime.GOOS == "windows" || os.Geteuid() == 0 {
		t.Skip("needs a folder this user cannot read")
	}
	root := t.TempDir()
	locked := filepath.Join(root, "album", "locked")
	if err := os.MkdirAll(locked, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(locked, "a.jpg"), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(locked, 0); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = os.Chmod(locked, 0o755) })
	svc := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: root}}})
	props, err := svc.Properties(context.Background(), []string{"media/album"})
	if err != nil {
		t.Fatal(err)
	}
	if props.Folders != 2 || props.Files != 0 || props.Complete {
		t.Fatalf("totals = %+v", props)
	}
}
