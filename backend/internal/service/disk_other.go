//go:build !linux

package service

import (
	"errors"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// getDiskUsage is a stub for non-Linux platforms
// The actual implementation uses Linux-specific syscalls
func getDiskUsage(path string) (*diskUsage, error) {
	return nil, errors.New("disk usage not supported on this platform")
}

// DiscoverMountPoints is a stub for non-Linux platforms.
// On non-Linux systems, auto-discovery is not supported, so we just
// return the mount points without the AutoDiscover ones expanded.
func DiscoverMountPoints(fs filesystem.FS, mountPoints []model.MountPoint) []model.MountPoint {
	// On non-Linux, just return the original mount points (auto-discover won't work)
	var result []model.MountPoint
	for _, mp := range mountPoints {
		if !mp.AutoDiscover {
			result = append(result, mp)
		}
		// Skip auto-discover mount points on non-Linux
	}
	return result
}

