//go:build linux

package service

import (
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/unix"
)

type desktopTrashDirs struct {
	rootFD  int
	trashFD int
	filesFD int
	infoFD  int
}

func (s *desktopTrashService) dataHome(create bool) (string, error) {
	if s.cfg.dataHome == "" || !filepath.IsAbs(s.cfg.dataHome) {
		return "", os.ErrNotExist
	}
	path := filepath.Clean(s.cfg.dataHome)
	if create {
		if err := os.MkdirAll(path, 0700); err != nil {
			return "", err
		}
	}
	resolved, err := filepath.EvalSymlinks(path)
	if err != nil {
		return "", err
	}
	fd, err := openAbsoluteDir(resolved, false)
	if err != nil {
		return "", err
	}
	defer unix.Close(fd)
	if err := verifyCurrentUserDirectory(fd); err != nil {
		return "", err
	}
	return resolved, nil
}

func (s *desktopTrashService) openTrashDirs(location desktopTrashLocation, create bool) (*desktopTrashDirs, error) {
	rootFD, err := openAbsoluteDir(location.root, false)
	if err != nil {
		return nil, err
	}
	dirs := &desktopTrashDirs{rootFD: rootFD, trashFD: -1, filesFD: -1, infoFD: -1}
	cleanup := func(err error) (*desktopTrashDirs, error) {
		dirs.close()
		return nil, err
	}
	if !location.home && strings.HasPrefix(location.relative, ".Trash"+string(filepath.Separator)) {
		var shared unix.Stat_t
		if err := unix.Fstatat(rootFD, ".Trash", &shared, unix.AT_SYMLINK_NOFOLLOW); err != nil ||
			shared.Mode&unix.S_IFMT != unix.S_IFDIR || shared.Mode&unix.S_ISVTX == 0 {
			return cleanup(ErrInvalidTrashItem)
		}
	}
	trashFD, err := openDirectoryRelative(rootFD, location.relative, create)
	if err != nil {
		return cleanup(err)
	}
	dirs.trashFD = trashFD
	if err := verifyCurrentUserDirectory(trashFD); err != nil {
		return cleanup(err)
	}
	if create {
		if err := unix.Fchmod(trashFD, 0700); err != nil {
			return cleanup(err)
		}
	}
	dirs.filesFD, err = openPrivateDirectory(trashFD, "files", create)
	if err != nil {
		return cleanup(err)
	}
	dirs.infoFD, err = openPrivateDirectory(trashFD, "info", create)
	if err != nil && !create && errors.Is(err, unix.ENOENT) {
		dirs.infoFD = -1
		err = nil
	}
	if err != nil {
		return cleanup(err)
	}
	return dirs, nil
}

func (d *desktopTrashDirs) close() {
	if d == nil {
		return
	}
	for _, fd := range []int{d.infoFD, d.filesFD, d.trashFD, d.rootFD} {
		if fd >= 0 {
			_ = unix.Close(fd)
		}
	}
	d.infoFD = -1
	d.filesFD = -1
	d.trashFD = -1
	d.rootFD = -1
}

func openAbsoluteDir(path string, create bool) (int, error) {
	if !filepath.IsAbs(path) {
		return -1, unix.EINVAL
	}
	clean := filepath.Clean(path)
	fd, err := unix.Open(string(filepath.Separator), unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return -1, err
	}
	for _, component := range strings.Split(strings.TrimPrefix(clean, string(filepath.Separator)), string(filepath.Separator)) {
		if component == "" || component == "." {
			continue
		}
		next, openErr := unix.Openat(fd, component, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if errors.Is(openErr, unix.ENOENT) && create {
			if mkdirErr := unix.Mkdirat(fd, component, 0700); mkdirErr != nil && !errors.Is(mkdirErr, unix.EEXIST) {
				_ = unix.Close(fd)
				return -1, mkdirErr
			}
			next, openErr = unix.Openat(fd, component, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		}
		if openErr != nil {
			_ = unix.Close(fd)
			return -1, openErr
		}
		_ = unix.Close(fd)
		fd = next
	}
	return fd, nil
}

func openDirectoryRelative(rootFD int, relative string, create bool) (int, error) {
	if filepath.IsAbs(relative) {
		return -1, unix.EINVAL
	}
	clean := filepath.Clean(relative)
	if clean == "." || clean == "" {
		return unix.Dup(rootFD)
	}
	if clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
		return -1, unix.EINVAL
	}
	fd, err := unix.Dup(rootFD)
	if err != nil {
		return -1, err
	}
	for _, component := range strings.Split(clean, string(filepath.Separator)) {
		if component == "" || component == "." || component == ".." {
			_ = unix.Close(fd)
			return -1, unix.EINVAL
		}
		next, openErr := unix.Openat(fd, component, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if errors.Is(openErr, unix.ENOENT) && create {
			if mkdirErr := unix.Mkdirat(fd, component, 0700); mkdirErr != nil && !errors.Is(mkdirErr, unix.EEXIST) {
				_ = unix.Close(fd)
				return -1, mkdirErr
			}
			next, openErr = unix.Openat(fd, component, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		}
		if openErr != nil {
			_ = unix.Close(fd)
			return -1, openErr
		}
		_ = unix.Close(fd)
		fd = next
	}
	return fd, nil
}

func ensurePrivateDirectory(parentFD int, name string) (int, error) {
	if name == "" || name == "." || name == ".." || strings.ContainsRune(name, filepath.Separator) {
		return -1, unix.EINVAL
	}
	if err := unix.Mkdirat(parentFD, name, 0700); err != nil && !errors.Is(err, unix.EEXIST) {
		return -1, err
	}
	return openPrivateDirectory(parentFD, name, false)
}

func openPrivateDirectory(parentFD int, name string, create bool) (int, error) {
	if create {
		if err := unix.Mkdirat(parentFD, name, 0700); err != nil && !errors.Is(err, unix.EEXIST) {
			return -1, err
		}
	}
	fd, err := unix.Openat(parentFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return -1, err
	}
	if err := verifyCurrentUserDirectory(fd); err != nil {
		_ = unix.Close(fd)
		return -1, err
	}
	if err := unix.Fchmod(fd, 0700); err != nil {
		_ = unix.Close(fd)
		return -1, err
	}
	return fd, nil
}

func verifyCurrentUserDirectory(fd int) error {
	var stat unix.Stat_t
	if err := unix.Fstat(fd, &stat); err != nil {
		return err
	}
	if stat.Mode&unix.S_IFMT != unix.S_IFDIR || int(stat.Uid) != os.Getuid() {
		return ErrInvalidTrashItem
	}
	return nil
}

func writeAndClose(fd int, contents []byte) error {
	for len(contents) > 0 {
		written, err := unix.Write(fd, contents)
		if err != nil {
			_ = unix.Close(fd)
			return err
		}
		if written == 0 {
			_ = unix.Close(fd)
			return io.ErrShortWrite
		}
		contents = contents[written:]
	}
	if err := unix.Fsync(fd); err != nil {
		_ = unix.Close(fd)
		return err
	}
	return unix.Close(fd)
}

func destinationExists(path string) bool {
	parentFD, err := openAbsoluteDir(filepath.Dir(path), false)
	if err != nil {
		return false
	}
	defer unix.Close(parentFD)
	var stat unix.Stat_t
	return unix.Fstatat(parentFD, filepath.Base(path), &stat, unix.AT_SYMLINK_NOFOLLOW) == nil
}

func readDirFD(fd int) ([]os.DirEntry, error) {
	duplicate, err := unix.Dup(fd)
	if err != nil {
		return nil, err
	}
	file := os.NewFile(uintptr(duplicate), "trash-directory")
	defer file.Close()
	return file.ReadDir(-1)
}

func readFileAt(parentFD int, name string) ([]byte, error) {
	fd, err := unix.Openat(parentFD, name, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, err
	}
	file := os.NewFile(uintptr(fd), "trash-info")
	defer file.Close()
	return io.ReadAll(io.LimitReader(file, maxTrashInfoBytes+1))
}

func removeTreeAt(parentFD int, name string, expected *unix.Stat_t) error {
	var stat unix.Stat_t
	if err := unix.Fstatat(parentFD, name, &stat, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		return err
	}
	if expected != nil && !sameTrashEntryIdentity(stat, *expected) {
		return ErrTrashItemNotFound
	}
	if stat.Mode&unix.S_IFMT != unix.S_IFDIR {
		return unix.Unlinkat(parentFD, name, 0)
	}
	childFD, err := unix.Openat(parentFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return err
	}
	children, err := readDirFD(childFD)
	if err != nil {
		_ = unix.Close(childFD)
		return err
	}
	for _, child := range children {
		if err := removeTreeAt(childFD, child.Name(), nil); err != nil && !errors.Is(err, unix.ENOENT) {
			_ = unix.Close(childFD)
			return err
		}
	}
	_ = unix.Close(childFD)
	return unix.Unlinkat(parentFD, name, unix.AT_REMOVEDIR)
}
