//go:build linux

package service

import (
	"io/fs"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type unreadableMountFS struct {
	filesystem.FS
	path string
}

func (f unreadableMountFS) ReadDirLimit(path string, limit int) ([]fs.DirEntry, bool, error) {
	if filepath.Clean(path) == f.path {
		return nil, false, fs.ErrPermission
	}
	return f.FS.ReadDirLimit(path, limit)
}

func TestFilterMountedDirsSkipsUnreadableMounts(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	for _, path := range []string{"/drives/readable", "/drives/unreadable"} {
		if err := fsys.MkdirAll(path, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	entries, err := fsys.ReadDir("/drives")
	if err != nil {
		t.Fatal(err)
	}
	mountSet := map[string]mountInfo{
		normalizePath("/drives/readable"):   {MountPoint: "/drives/readable", FSType: "ext4"},
		normalizePath("/drives/unreadable"): {MountPoint: "/drives/unreadable", FSType: "ext4"},
	}
	fsy := unreadableMountFS{FS: fsys, path: "/drives/unreadable"}

	got := filterMountedDirs(fsy, entries, model.MountPoint{Name: "drives", Path: "/drives"}, mountSet)
	if len(got) != 1 || got[0].Name != "readable" {
		t.Fatalf("filterMountedDirs() = %+v, want only readable mount", got)
	}
}

func TestBPFIsNotAStorageFilesystem(t *testing.T) {
	if isRealFilesystem("bpf") {
		t.Fatal("bpf filesystem should not be discovered as a storage drive")
	}
}
