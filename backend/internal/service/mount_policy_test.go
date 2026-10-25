package service

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestMountWritePolicyUsesMostSpecificRoot(t *testing.T) {
	root := t.TempDir()
	home := filepath.Join(root, "home")
	protected := filepath.Join(home, "backups")
	for _, dir := range []string{home, protected} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
	}
	list := []model.MountPoint{{Name: "root", Path: root, ReadOnly: true}, {Name: "home", Path: home}, {Name: "backups", Path: protected, ReadOnly: true}}
	m := newMounts(filesystem.NewOsFS(), fixedMounts(list))
	for _, test := range []struct {
		path string
		want error
	}{
		{"home/new", nil},
		{"home/backups/new", ErrPermissionDenied},
		{"root/home/new", ErrPermissionDenied},
	} {
		_, _, err := m.resolve(test.path, writeMaybeMissing)
		if !errors.Is(err, test.want) {
			t.Errorf("%s: %v, want %v", test.path, err, test.want)
		}
	}
	if err := m.requireWritablePath(home, true); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("tree containing protected root: %v", err)
	}
}

func TestBrokenReadOnlyMountDoesNotBlockUnrelatedWrites(t *testing.T) {
	root := t.TempDir()
	broken := filepath.Join(root, "file")
	if err := os.WriteFile(broken, []byte("file"), 0644); err != nil {
		t.Fatal(err)
	}
	m := newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{{Name: "home", Path: root}, {Name: "broken", Path: filepath.Join(broken, "sub"), ReadOnly: true}}))
	if _, _, err := m.resolve("home/new", writeMaybeMissing); err != nil {
		t.Fatalf("unrelated write: %v", err)
	}
	if err := m.requireWritablePath(broken, true); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("affected tree: %v", err)
	}
}

func TestRelativeReadOnlyMountUsesFilesystemPathSemantics(t *testing.T) {
	root := t.TempDir()
	protected := filepath.Join(root, "backups")
	if err := os.Mkdir(protected, 0755); err != nil {
		t.Fatal(err)
	}
	// AferoFS roots relative paths at /, so this spelling names the same directory.
	relative := strings.TrimPrefix(protected, string(filepath.Separator))
	m := newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{{Name: "home", Path: root}, {Name: "backups", Path: relative, ReadOnly: true}}))
	if _, _, err := m.resolve("home/backups/new", writeMaybeMissing); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("relative protected root: %v", err)
	}
}

func TestDanglingReadOnlyAliasProtectsMissingTarget(t *testing.T) {
	root := t.TempDir()
	missing := filepath.Join(root, "missing")
	alias := filepath.Join(root, "alias")
	if err := os.Symlink("missing", alias); err != nil {
		t.Fatal(err)
	}
	m := newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{{Name: "home", Path: root}, {Name: "backups", Path: alias, ReadOnly: true}}))
	if _, _, err := m.resolve("home/missing/new", writeMaybeMissing); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("dangling alias to %s: %v", missing, err)
	}
}

type unavailableMountFS struct {
	filesystem.FS
	root string
	err  error
}

func (f unavailableMountFS) EvalSymlinks(path string) (string, error) {
	if pathWithinRoot(f.root, path) {
		return "", f.err
	}
	return f.FS.EvalSymlinks(path)
}

func TestUnresolvedReadOnlyMountFailsClosed(t *testing.T) {
	root := t.TempDir()
	unavailable := filepath.Join(root, "unavailable")
	for _, cause := range []error{os.ErrPermission, syscall.ELOOP} {
		fsys := unavailableMountFS{FS: filesystem.NewOsFS(), root: unavailable, err: cause}
		m := newMounts(fsys, fixedMounts([]model.MountPoint{{Name: "home", Path: root}, {Name: "unavailable", Path: unavailable, ReadOnly: true}}))
		if _, _, err := m.resolve("home/new", writeMaybeMissing); !errors.Is(err, ErrPermissionDenied) {
			t.Errorf("write with unresolved root %v: %v", cause, err)
		}
		if _, _, err := m.resolve("home", readExisting); err != nil {
			t.Errorf("read with unresolved root %v: %v", cause, err)
		}
		if err := m.requireWritablePath(filepath.Join(unavailable, "file"), false); !errors.Is(err, ErrPermissionDenied) {
			t.Errorf("affected write with %v: %v", cause, err)
		}
	}
}
