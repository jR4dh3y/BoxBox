package service

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
)

func TestMountsResolve(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	writable := filepath.Join(root, "rw")
	readOnly := filepath.Join(root, "ro")
	for _, dir := range []string{writable, readOnly} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(writable, "a.txt"), []byte("a"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(writable, "escape")); err != nil {
		t.Fatal(err)
	}

	list := []model.MountPoint{
		{Name: "rw", Path: writable},
		{Name: "ro", Path: readOnly, ReadOnly: true},
	}
	m := newMounts(filesystem.NewOsFS(), fixedMounts(list))

	tests := []struct {
		name    string
		path    string
		access  pathAccess
		wantErr error
	}{
		{"existing file", "rw/a.txt", readExisting, nil},
		{"missing file when it must exist", "rw/missing.txt", readExisting, ErrPathNotFound},
		{"missing file when it may be missing", "rw/new.txt", writeMaybeMissing, nil},
		{"existing file for modify", "rw/a.txt", writeExisting, nil},
		{"read-only mount can be read", "ro", readExisting, nil},
		{"read-only mount refuses modify", "ro", writeExisting, ErrPermissionDenied},
		{"read-only mount refuses create", "ro/new.txt", writeMaybeMissing, ErrPermissionDenied},
		{"symlink out of the mount for read", "rw/escape", readExisting, ErrPermissionDenied},
		{"symlink out of the mount for create", "rw/escape/new.txt", writeMaybeMissing, ErrPermissionDenied},
		{"unknown mount", "elsewhere/a.txt", readExisting, ErrMountPointNotFound},
		{"unknown mount keeps the validator cause", "elsewhere/a.txt", readExisting, validator.ErrOutsideMountPoint},
		{"traversal", "rw/../ro/x", readExisting, validator.ErrPathTraversal},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			mount, resolved, err := m.resolve(tt.path, tt.access)
			if !errors.Is(err, tt.wantErr) {
				t.Fatalf("resolve(%q) error = %v, want %v", tt.path, err, tt.wantErr)
			}
			if tt.wantErr == nil && (mount == nil || resolved == "") {
				t.Fatalf("resolve(%q) = %v, %q, want a mount and a path", tt.path, mount, resolved)
			}
		})
	}
}

func TestMountsResolveReadsTheLiveList(t *testing.T) {
	dir := t.TempDir()
	var list []model.MountPoint
	m := newMounts(filesystem.NewOsFS(), func() []model.MountPoint { return list })

	if _, _, err := m.resolve("media", readExisting); !errors.Is(err, ErrMountPointNotFound) {
		t.Fatalf("before the mount exists: error = %v, want %v", err, ErrMountPointNotFound)
	}
	list = []model.MountPoint{{Name: "media", Path: dir}}
	if _, _, err := m.resolve("media", readExisting); err != nil {
		t.Fatalf("after the mount exists: %v", err)
	}
}

func TestIsMalformedPath(t *testing.T) {
	if !isMalformedPath(validator.ErrPathTraversal) || !isMalformedPath(validator.ErrEmptyPath) || !isMalformedPath(validator.ErrInvalidPath) {
		t.Fatal("validator path errors should be malformed")
	}
	if isMalformedPath(ErrPathNotFound) || isMalformedPath(ErrMountPointNotFound) || isMalformedPath(nil) {
		t.Fatal("lookup errors are not malformed")
	}
}

func TestMountsResolveOverlappingReadOnlyPaths(t *testing.T) {
	root := t.TempDir()
	protected := filepath.Join(root, "backups")
	if err := os.Mkdir(protected, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(protected, "db.bak"), []byte("precious"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(protected, filepath.Join(root, "alias")); err != nil {
		t.Fatal(err)
	}
	list := []model.MountPoint{{Name: "home", Path: root}, {Name: "backups", Path: filepath.Join(root, "alias"), ReadOnly: true}, {Name: "child", Path: protected}}
	m := newMounts(filesystem.NewOsFS(), func() []model.MountPoint { return list })
	for _, test := range []struct {
		path   string
		access pathAccess
		want   error
	}{
		{"home/backups/db.bak", readExisting, nil},
		{"home/backups/db.bak", writeExisting, ErrPermissionDenied},
		{"home/alias/db.bak", writeExisting, ErrPermissionDenied},
		{"home/backups/new/sub/file", writeMaybeMissing, ErrPermissionDenied},
		{"child/db.bak", writeExisting, ErrPermissionDenied},
		{"home/backups-other/new", writeMaybeMissing, nil},
		{"home/allowed", writeMaybeMissing, nil},
	} {
		t.Run(test.path, func(t *testing.T) {
			_, _, err := m.resolve(test.path, test.access)
			if !errors.Is(err, test.want) {
				t.Fatalf("resolve: %v, want %v", err, test.want)
			}
		})
	}
	// Re-read the configuration on each write instead of caching permission decisions.
	list[1].ReadOnly = false
	if _, _, err := m.resolve("home/backups/db.bak", writeExisting); err != nil {
		t.Fatal(err)
	}
	list[1].ReadOnly = true
	if err := os.Remove(filepath.Join(protected, "db.bak")); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(protected); err != nil {
		t.Fatal(err)
	}
	// Use the configured real path while its directory is absent.
	list[1].Path = protected
	if _, _, err := m.resolve("home/backups/new", writeMaybeMissing); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("missing protected root: %v", err)
	}
}
