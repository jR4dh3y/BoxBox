package service

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"syscall"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
)

// pathAccess states how an operation will use a path. It decides which
// checks mounts.resolve applies.
type pathAccess int

const (
	// readExisting needs a path that exists. Any mount may be read.
	readExisting pathAccess = iota
	// writeExisting needs a path that exists on a writable mount.
	writeExisting
	// writeMaybeMissing accepts a path that does not exist yet, on a writable mount.
	writeMaybeMissing
)

// mounts is the one place that turns a virtual path such as "media/a.mkv"
// into a real path that stays inside its mount, symlinks included.
type mounts struct {
	fs   filesystem.FS
	list func() []model.MountPoint
}

func newMounts(fsys filesystem.FS, list func() []model.MountPoint) *mounts {
	return &mounts{fs: fsys, list: list}
}

func fixedMounts(list []model.MountPoint) func() []model.MountPoint {
	return func() []model.MountPoint { return list }
}

// resolve returns the mount and the confined real path for a virtual path.
// A path outside every mount returns an error that matches both
// ErrMountPointNotFound and the validator's cause.
func (m *mounts) resolve(path string, access pathAccess) (*model.MountPoint, string, error) {
	mount, fsPath, err := validator.ValidatePathAgainstMounts(path, m.list())
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) || errors.Is(err, validator.ErrMountPointNotFound) {
			return nil, "", fmt.Errorf("%w: %w", ErrMountPointNotFound, err)
		}
		return nil, "", err
	}
	if access != readExisting && mount.ReadOnly {
		return nil, "", ErrPermissionDenied
	}
	if access == writeMaybeMissing {
		fsPath, err = resolveWritablePathWithinMount(m.fs, mount, fsPath)
	} else {
		fsPath, err = resolveExistingPathWithinMount(m.fs, mount, fsPath)
	}
	if err != nil {
		return nil, "", err
	}
	if access != readExisting {
		if err := m.requireWritablePath(fsPath, false); err != nil {
			return nil, "", err
		}
	}
	return mount, fsPath, nil
}

// isMalformedPath reports whether err means the path text itself is invalid.
func isMalformedPath(err error) bool {
	return errors.Is(err, validator.ErrPathTraversal) ||
		errors.Is(err, validator.ErrEmptyPath) ||
		errors.Is(err, validator.ErrInvalidPath)
}

// The deepest real mount controls writes; read-only aliases win ties.
// Removing or moving a tree also affects read-only mounts below it.
func (m *mounts) requireWritablePath(path string, tree bool) error {
	path, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	deepest := ""
	readOnly := false
	for _, mount := range m.list() {
		root, err := m.resolveMountRoot(mount.Path, 0)
		if err != nil {
			if mount.ReadOnly {
				// ENOTDIR cannot hide a live root. Other failures may conceal a writable alias.
				if !errors.Is(err, syscall.ENOTDIR) || pathWithinRoot(root, path) || (tree && pathWithinRoot(path, root)) {
					return ErrPermissionDenied
				}
			}
			continue
		}
		if tree && mount.ReadOnly && pathWithinRoot(path, root) {
			return ErrPermissionDenied
		}
		if !pathWithinRoot(root, path) {
			continue
		}
		if len(root) > len(deepest) {
			deepest, readOnly = root, mount.ReadOnly
		} else if root == deepest && mount.ReadOnly {
			readOnly = true
		}
	}
	if readOnly {
		return ErrPermissionDenied
	}
	return nil
}

// Resolve missing roots and dangling aliases without losing their protected target.
func (m *mounts) resolveMountRoot(path string, links int) (string, error) {
	fallback := filepath.Clean(string(filepath.Separator) + path)
	if links >= 40 {
		return fallback, ErrPermissionDenied
	}
	existing := path
	for {
		resolved, err := m.fs.EvalSymlinks(existing)
		if err == nil {
			relative, err := filepath.Rel(existing, path)
			if err != nil {
				return fallback, err
			}
			return filepath.Abs(filepath.Join(resolved, relative))
		}
		if !errors.Is(err, fs.ErrNotExist) {
			return fallback, err
		}
		info, statErr := m.fs.Lstat(existing)
		if statErr != nil && !errors.Is(statErr, fs.ErrNotExist) {
			return fallback, statErr
		}
		if statErr == nil && info.Mode()&os.ModeSymlink != 0 {
			target, err := m.fs.Readlink(existing)
			if err != nil {
				return fallback, err
			}
			if !filepath.IsAbs(target) {
				target = filepath.Join(filepath.Dir(existing), target)
			}
			relative, err := filepath.Rel(existing, path)
			if err != nil {
				return fallback, err
			}
			return m.resolveMountRoot(filepath.Join(target, relative), links+1)
		}
		parent := filepath.Dir(existing)
		if parent == existing {
			return fallback, err
		}
		existing = parent
	}
}
