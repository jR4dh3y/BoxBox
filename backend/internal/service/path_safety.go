package service

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	boxfs "github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func resolveExistingPathWithinMount(fsys boxfs.FS, mount *model.MountPoint, path string) (string, error) {
	root, err := fsys.EvalSymlinks(mount.Path)
	if err != nil {
		return "", err
	}
	resolved, err := fsys.EvalSymlinks(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return "", ErrPathNotFound
		}
		return "", err
	}
	if !pathWithinRoot(root, resolved) {
		return "", ErrPermissionDenied
	}
	return resolved, nil
}

func resolveWritablePathWithinMount(fsys boxfs.FS, mount *model.MountPoint, path string) (string, error) {
	root, err := fsys.EvalSymlinks(mount.Path)
	if err != nil {
		return "", err
	}

	existing := path
	for {
		resolved, resolveErr := fsys.EvalSymlinks(existing)
		if resolveErr == nil {
			if !pathWithinRoot(root, resolved) {
				return "", ErrPermissionDenied
			}
			relative, relErr := filepath.Rel(existing, path)
			if relErr != nil {
				return "", relErr
			}
			return filepath.Join(resolved, relative), nil
		}
		if !errors.Is(resolveErr, fs.ErrNotExist) {
			return "", resolveErr
		}
		parent := filepath.Dir(existing)
		if parent == existing {
			return "", fmt.Errorf("no existing parent for %q", path)
		}
		existing = parent
	}
}

func pathWithinRoot(root, candidate string) bool {
	relative, err := filepath.Rel(filepath.Clean(root), filepath.Clean(candidate))
	return err == nil && relative != ".." &&
		!filepath.IsAbs(relative) &&
		!startsWithParent(relative)
}

func startsWithParent(path string) bool {
	return path == ".." || len(path) > 3 && path[:3] == ".."+string(filepath.Separator)
}

func verifyOpenFileWithinMount(fsys boxfs.FS, mount *model.MountPoint, opened any) error {
	if runtime.GOOS != "linux" {
		return nil
	}
	descriptor, ok := opened.(interface{ Fd() uintptr })
	if !ok {
		return nil
	}
	actual, err := os.Readlink("/proc/self/fd/" + strconv.FormatUint(uint64(descriptor.Fd()), 10))
	if err != nil {
		return nil
	}
	actual = strings.TrimSuffix(actual, " (deleted)")
	root, err := fsys.EvalSymlinks(mount.Path)
	if err != nil {
		return err
	}
	if !pathWithinRoot(root, actual) {
		return ErrPermissionDenied
	}
	return nil
}
