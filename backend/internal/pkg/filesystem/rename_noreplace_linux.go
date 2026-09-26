//go:build linux

package filesystem

import (
	"errors"
	"io/fs"
	"os"

	"golang.org/x/sys/unix"
)

func renameNoReplaceOS(oldpath, newpath string) error {
	err := unix.Renameat2(unix.AT_FDCWD, oldpath, unix.AT_FDCWD, newpath, unix.RENAME_NOREPLACE)
	if err == nil || errors.Is(err, fs.ErrExist) {
		return err
	}
	if !errors.Is(err, unix.ENOSYS) && !errors.Is(err, unix.EINVAL) && !errors.Is(err, unix.EOPNOTSUPP) {
		return err
	}
	return renameNoReplaceWithLink(oldpath, newpath, os.Link)
}
