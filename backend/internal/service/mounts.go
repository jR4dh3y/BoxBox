package service

import (
	"errors"
	"fmt"

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
	return mount, fsPath, nil
}

// isMalformedPath reports whether err means the path text itself is invalid.
func isMalformedPath(err error) bool {
	return errors.Is(err, validator.ErrPathTraversal) ||
		errors.Is(err, validator.ErrEmptyPath) ||
		errors.Is(err, validator.ErrInvalidPath)
}
