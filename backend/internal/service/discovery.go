//go:build linux

package service

import (
	"io/fs"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// DiscoverMountPoints expands mount points with auto_discover enabled
// by scanning for subdirectories that are actual mount points.
// Virtual filesystems (tmpfs, proc, sysfs, etc.) are filtered out.
func DiscoverMountPoints(fs filesystem.FS, mountPoints []model.MountPoint) []model.MountPoint {
	var result []model.MountPoint

	for _, mp := range mountPoints {
		if !mp.AutoDiscover {
			result = append(result, mp)
			continue
		}

		discovered := discoverSubMounts(fs, mp)
		if len(discovered) == 0 {
			// No subdirectories found, keep the original
			result = append(result, mp)
		} else {
			result = append(result, discovered...)
		}
	}

	return result
}

// discoverSubMounts scans a directory for subdirectories that are actual mount points
func discoverSubMounts(fs filesystem.FS, parent model.MountPoint) []model.MountPoint {
	mountSet := buildRealMountPointSet()
	if mountSet == nil {
		return nil
	}

	entries, err := fs.ReadDir(parent.Path)
	if err != nil {
		return nil
	}

	return filterMountedDirs(entries, parent, mountSet)
}

// buildRealMountPointSet reads system mounts and returns only real filesystems
func buildRealMountPointSet() map[string]mountInfo {
	mounts, err := getMountInfo()
	if err != nil {
		return nil
	}

	set := make(map[string]mountInfo, len(mounts))
	for _, m := range mounts {
		if isRealFilesystem(m.FSType) {
			set[normalizePath(m.MountPoint)] = m
		}
	}
	return set
}

// filterMountedDirs filters directory entries to only those that are mount points
func filterMountedDirs(entries []fs.DirEntry, parent model.MountPoint, mountSet map[string]mountInfo) []model.MountPoint {
	var discovered []model.MountPoint

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		subPath := filepath.Join(parent.Path, entry.Name())

		if !isMountPoint(subPath, mountSet) {
			continue
		}

		discovered = append(discovered, model.MountPoint{
			Name:         entry.Name(),
			Path:         subPath,
			ReadOnly:     parent.ReadOnly,
			AutoDiscover: false,
		})
	}

	return discovered
}

// isMountPoint checks if a path is an actual mount point
func isMountPoint(path string, mountSet map[string]mountInfo) bool {
	_, exists := mountSet[normalizePath(path)]
	return exists
}

// normalizePath normalizes a path for comparison
func normalizePath(path string) string {
	path = filepath.Clean(path)
	path = strings.TrimSuffix(path, string(filepath.Separator))
	if path == "" {
		return string(filepath.Separator)
	}
	return path
}

// isRealFilesystem returns true if the filesystem type is a real storage filesystem
func isRealFilesystem(fsType string) bool {
	return !config.VirtualFilesystems[fsType]
}
