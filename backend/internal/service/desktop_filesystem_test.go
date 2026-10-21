package service

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestFileServiceHandlesLegalDotSequenceNamesOnDisk(t *testing.T) {
	root := t.TempDir()
	wantContents := map[string]string{
		"report..txt": "double-dot name",
		"...hidden":   "leading dots",
	}
	for name, content := range wantContents {
		if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
	}

	files := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "home",
		Path: root,
	}}})
	ctx := context.Background()
	listing, err := files.List(ctx, "home", model.ListOptions{
		Page:          1,
		PageSize:      len(wantContents),
		SortBy:        "name",
		SortDir:       "asc",
		IncludeHidden: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if listing.TotalCount != len(wantContents) || len(listing.Items) != len(wantContents) {
		t.Fatalf("listing = total %d, items %d; want %d", listing.TotalCount, len(listing.Items), len(wantContents))
	}
	listedNames := make(map[string]bool, len(listing.Items))
	for _, item := range listing.Items {
		listedNames[item.Name] = true
	}

	for _, name := range []string{"report..txt", "...hidden"} {
		if !listedNames[name] {
			t.Errorf("List omitted %q", name)
		}
		info, err := files.GetInfo(ctx, "home/"+name)
		if err != nil {
			t.Fatalf("GetInfo(%q): %v", name, err)
		}
		if info.Name != name {
			t.Errorf("GetInfo(%q) name = %q", name, info.Name)
		}

		opened, _, err := files.OpenFile(ctx, "home/"+name)
		if err != nil {
			t.Fatalf("OpenFile(%q): %v", name, err)
		}
		content, readErr := io.ReadAll(opened)
		closeErr := opened.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read/close %q: read error %v, close error %v", name, readErr, closeErr)
		}
		if string(content) != wantContents[name] {
			t.Errorf("OpenFile(%q) content = %q, want %q", name, content, wantContents[name])
		}
	}
}

func TestDesktopFileServiceListsDirectoriesBeyondServerLimit(t *testing.T) {
	const directoryEntryCount = 10_001
	root := t.TempDir()
	for i := 0; i < directoryEntryCount; i++ {
		name := fmt.Sprintf("entry-%05d.txt", i)
		file, err := os.Create(filepath.Join(root, name))
		if err != nil {
			t.Fatalf("create %q: %v", name, err)
		}
		if err := file.Close(); err != nil {
			t.Fatalf("close %q: %v", name, err)
		}
	}

	mounts := []model.MountPoint{{Name: "home", Path: root}}
	ctx := context.Background()
	desktopFiles := NewFileService(filesystem.NewOsFS(), FileServiceConfig{
		MountPoints:         mounts,
		DirectoryEntryLimit: -1,
	})
	page, err := desktopFiles.List(ctx, "home", model.ListOptions{
		Page:     2,
		PageSize: 25,
		SortBy:   "name",
		SortDir:  "asc",
	})
	if err != nil {
		t.Fatalf("desktop List: %v", err)
	}
	if page.TotalCount != directoryEntryCount || len(page.Items) != 25 {
		t.Fatalf("desktop page = total %d, items %d; want total %d, items 25", page.TotalCount, len(page.Items), directoryEntryCount)
	}
	if page.Items[0].Name != "entry-00025.txt" {
		t.Fatalf("second page starts at %q, want entry-00025.txt", page.Items[0].Name)
	}

	remoteFiles := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: mounts})
	if _, err := remoteFiles.List(ctx, "home", model.ListOptions{Page: 1, PageSize: 25}); !errors.Is(err, ErrDirectoryTooLarge) {
		t.Fatalf("default server List error = %v, want %v", err, ErrDirectoryTooLarge)
	}
}
