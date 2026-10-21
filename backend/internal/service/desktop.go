package service

import (
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// NewDesktopOpener confines virtual paths before handing them to the OS.
// Only the private desktop server installs this capability.
func NewDesktopOpener(fsys filesystem.FS, list []model.MountPoint, open func(string) error) func(string) error {
	mounts := newMounts(fsys, fixedMounts(list))
	return func(path string) error {
		_, resolved, err := mounts.resolve(path, readExisting)
		if err != nil {
			return err
		}
		return open(resolved)
	}
}
