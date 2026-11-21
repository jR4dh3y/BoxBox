//go:build linux

package filesystem

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"

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

// restoreEntry moves from back to to, two names in one folder below directory,
// without replacing an entry at to. renameat2 needs no hard links, so FAT and
// exFAT volumes restore too; older kernels fall back to a link.
func restoreEntry(directory *os.Root, from, to string) error {
	parent, err := directory.Open(filepath.Dir(to))
	if err != nil {
		return err
	}
	defer parent.Close()
	fd := int(parent.Fd())
	err = unix.Renameat2(fd, filepath.Base(from), fd, filepath.Base(to), unix.RENAME_NOREPLACE)
	if err == nil || errors.Is(err, fs.ErrExist) {
		return err
	}
	if !errors.Is(err, unix.ENOSYS) && !errors.Is(err, unix.EINVAL) && !errors.Is(err, unix.EOPNOTSUPP) {
		return err
	}
	return restoreByLink(directory, from, to)
}
