// Package service provides business logic for the file manager.
// Focused service tests for directory and file operations.
package service

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"slices"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// setupTestFileService creates a file service with an in-memory filesystem for testing
func setupTestFileService() (FileService, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()

	// Create mount point directories
	fs.MkdirAll("/data/media", 0755)
	fs.MkdirAll("/data/documents", 0755)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "documents", Path: "/data/documents", ReadOnly: false},
	}

	svc := NewFileService(fs, FileServiceConfig{MountPoints: mounts})
	return svc, fs
}

type truncatedDirectoryListingFS struct {
	filesystem.FS
}

func (f truncatedDirectoryListingFS) ReadDirFileInfoLimit(_ string, limit int) ([]fs.FileInfo, bool, error) {
	if limit != 10_000 {
		return nil, false, fmt.Errorf("unexpected directory limit %d", limit)
	}
	return nil, true, nil
}

func TestDirectoryListingReturnsHiddenAndVisibleEntries(t *testing.T) {
	svc, fs := setupTestFileService()
	ctx := context.Background()

	testDir := "/data/media/visible-count"
	if err := fs.MkdirAll(testDir, 0755); err != nil {
		t.Fatal(err)
	}
	want := []string{".cache", ".env", "alpha.txt", "beta.txt"}
	for _, name := range want {
		if err := fs.WriteFile(testDir+"/"+name, []byte("content"), 0644); err != nil {
			t.Fatal(err)
		}
	}

	list, err := svc.List(ctx, "media/visible-count")
	if err != nil {
		t.Fatal(err)
	}
	got := make([]string, 0, len(list.Items))
	for _, item := range list.Items {
		got = append(got, item.Name)
	}
	slices.Sort(got)
	if !slices.Equal(got, want) {
		t.Fatalf("listing = %v, want %v", got, want)
	}
}

func TestListReadsTheDirectoryOnce(t *testing.T) {
	memory := filesystem.NewMemMapFS()
	if err := memory.MkdirAll("/data/media/big", 0755); err != nil {
		t.Fatal(err)
	}
	for i := range 500 {
		if err := memory.WriteFile(fmt.Sprintf("/data/media/big/file%03d.txt", i), nil, 0644); err != nil {
			t.Fatal(err)
		}
	}
	counting := &readCountingFS{FS: memory}
	svc := NewFileService(counting, FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}}})

	list, err := svc.List(context.Background(), "media/big")
	if err != nil {
		t.Fatal(err)
	}
	if len(list.Items) != 500 {
		t.Fatalf("listing has %d items, want 500", len(list.Items))
	}
	if counting.reads != 1 {
		t.Fatalf("opening a folder read it %d times, want 1", counting.reads)
	}
}

// readCountingFS counts directory reads so a test can pin how often a folder open touches the disk.
type readCountingFS struct {
	filesystem.FS
	reads int
}

func (c *readCountingFS) ReadDir(name string) ([]fs.DirEntry, error) {
	c.reads++
	return c.FS.ReadDir(name)
}

func (c *readCountingFS) ReadDirLimit(name string, limit int) ([]fs.DirEntry, bool, error) {
	c.reads++
	return c.FS.ReadDirLimit(name, limit)
}

func (c *readCountingFS) ReadDirFileInfoLimit(name string, limit int) ([]fs.FileInfo, bool, error) {
	c.reads++
	return c.FS.ReadDirFileInfoLimit(name, limit)
}

func TestDirectoryListingHonorsCanceledContext(t *testing.T) {
	svc, _ := setupTestFileService()
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	if _, err := svc.List(ctx, "media/canceled"); !errors.Is(err, context.Canceled) {
		t.Fatalf("List error = %v, want context.Canceled", err)
	}
}

func TestDirectoryListingEnforcesEntryLimit(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media/large", 0755); err != nil {
		t.Fatal(err)
	}
	svc := NewFileService(truncatedDirectoryListingFS{FS: fsys}, FileServiceConfig{
		MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}},
	})

	if _, err := svc.List(context.Background(), "media/large"); !errors.Is(err, ErrDirectoryTooLarge) {
		t.Fatalf("List error = %v, want ErrDirectoryTooLarge", err)
	}
}

func TestCreateFileCreatesEmptyFileWithoutOverwriting(t *testing.T) {
	svc, fs := setupTestFileService()
	ctx := context.Background()

	file, err := svc.CreateFile(ctx, "media/new-note.txt")
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}

	content, err := fs.ReadFile("/data/media/new-note.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "" {
		t.Fatalf("expected created file to be empty, got %q", string(content))
	}

	existing, err := svc.CreateFile(ctx, "media/new-note.txt")
	if existing != nil {
		_ = existing.Close()
	}
	if !errors.Is(err, ErrPathExists) {
		t.Fatalf("expected ErrPathExists, got %v", err)
	}
}

func TestWriteFileOverwritesExistingFileOnly(t *testing.T) {
	svc, fs := setupTestFileService()
	ctx := context.Background()

	if err := fs.WriteFile("/data/media/editable.txt", []byte("before"), 0644); err != nil {
		t.Fatal(err)
	}

	if err := svc.WriteFile(ctx, "media/editable.txt", []byte("after")); err != nil {
		t.Fatal(err)
	}

	content, err := fs.ReadFile("/data/media/editable.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "after" {
		t.Fatalf("expected overwritten content, got %q", string(content))
	}

	if err := svc.WriteFile(ctx, "media/missing.txt", []byte("new")); !errors.Is(err, ErrPathNotFound) {
		t.Fatalf("expected ErrPathNotFound, got %v", err)
	}
}
