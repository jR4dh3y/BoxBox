package service

import (
	"context"
	"errors"
	"fmt"
	"slices"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func peekService(t *testing.T, files ...string) FileService {
	t.Helper()
	memory := filesystem.NewMemMapFS()
	if err := memory.MkdirAll("/data/media/folder", 0755); err != nil {
		t.Fatal(err)
	}
	for _, name := range files {
		if err := memory.WriteFile("/data/media/folder/"+name, []byte("x"), 0644); err != nil {
			t.Fatal(err)
		}
	}
	return NewFileService(memory, FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}}})
}

func names(items []model.FileInfo) []string {
	got := make([]string, 0, len(items))
	for _, item := range items {
		got = append(got, item.Name)
	}
	return got
}

func TestPeekShowsPicturesFirstThenNames(t *testing.T) {
	svc := peekService(t, "b.txt", "a.txt", "z.png", ".hidden.jpg", "c.JPG")
	items, err := svc.Peek(context.Background(), "media/folder", false)
	if err != nil {
		t.Fatal(err)
	}
	if want := []string{"c.JPG", "z.png", "a.txt"}; !slices.Equal(names(items), want) {
		t.Fatalf("peek = %v, want %v", names(items), want)
	}
	if items[0].Path != "media/folder/c.JPG" {
		t.Fatalf("path = %q", items[0].Path)
	}
	hidden, err := svc.Peek(context.Background(), "media/folder", true)
	if err != nil {
		t.Fatal(err)
	}
	if want := []string{".hidden.jpg", "c.JPG", "z.png"}; !slices.Equal(names(hidden), want) {
		t.Fatalf("peek with hidden = %v, want %v", names(hidden), want)
	}
}

func TestPeekReadsOnlyTheFirstEntriesOfAHugeFolder(t *testing.T) {
	files := make([]string, 0, 500)
	for i := range 500 {
		files = append(files, fmt.Sprintf("file%03d.txt", i))
	}
	svc := peekService(t, files...)
	items, err := svc.Peek(context.Background(), "media/folder", false)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 3 {
		t.Fatalf("peek has %d items, want 3", len(items))
	}
}

func TestPeekRefusesAFileAndAPathOutsideTheMounts(t *testing.T) {
	svc := peekService(t, "a.txt")
	if _, err := svc.Peek(context.Background(), "media/folder/a.txt", false); !errors.Is(err, ErrNotDirectory) {
		t.Fatalf("file: err = %v", err)
	}
	if _, err := svc.Peek(context.Background(), "media/../../etc", false); err == nil {
		t.Fatal("a path outside the mounts was read")
	}
}
