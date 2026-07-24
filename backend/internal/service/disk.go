//go:build linux

package service

import (
	"bufio"
	"os"
	"strings"
	"syscall"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
)

// mountInfo stores information about a mount point from /proc/mounts
type mountInfo struct {
	Device     string
	MountPoint string
	FSType     string
}

// getMountInfo reads /proc/mounts to get mount information
// Note: This reads system info directly, not user files, so os.Open is appropriate here
func getMountInfo() ([]mountInfo, error) {
	file, err := os.Open(config.ProcMountsPath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var mounts []mountInfo
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) >= 3 {
			mounts = append(mounts, mountInfo{
				Device:     fields[0],
				MountPoint: fields[1],
				FSType:     fields[2],
			})
		}
	}
	return mounts, scanner.Err()
}

// findMountPoint finds the mount point for a given path
func findMountPoint(path string, mounts []mountInfo) *mountInfo {
	// Normalize the path
	path = strings.TrimSuffix(path, "/")
	if path == "" {
		path = "/"
	}

	var bestMatch *mountInfo
	bestLen := 0

	for i := range mounts {
		mount := &mounts[i]
		mp := strings.TrimSuffix(mount.MountPoint, "/")
		if mp == "" {
			mp = "/"
		}

		// Check if path starts with this mount point
		if path == mp || strings.HasPrefix(path, mp+"/") {
			if len(mp) > bestLen {
				bestLen = len(mp)
				bestMatch = mount
			}
		}
	}
	return bestMatch
}

// getDiskUsage returns disk usage statistics for the given path (Unix/Linux/macOS)
// It identifies the actual mount point for the path and returns its stats
func getDiskUsage(path string) (*diskUsage, error) {
	// Get mount information to identify the actual mount point
	mounts, err := getMountInfo()
	if err != nil {
		// Fallback to simple statfs if we can't read mounts
		return getDiskUsageSimple(path, nil)
	}

	// Find the mount point for this path
	mount := findMountPoint(path, mounts)
	if mount == nil {
		return getDiskUsageSimple(path, nil)
	}

	// Get stats for the actual mount point
	return getDiskUsageSimple(mount.MountPoint, mount)
}

// getDiskUsageSimple returns disk usage using statfs directly on the path
func getDiskUsageSimple(path string, mount *mountInfo) (*diskUsage, error) {
	var stat syscall.Statfs_t
	if err := syscall.Statfs(path, &stat); err != nil {
		return nil, err
	}

	total := stat.Blocks * uint64(stat.Bsize)
	free := stat.Bavail * uint64(stat.Bsize)
	used := total - free

	usedPct := float64(0)
	if total > 0 {
		usedPct = float64(used) / float64(total) * config.PercentMultiplier
	}

	du := &diskUsage{
		Total:   total,
		Free:    free,
		Used:    used,
		UsedPct: usedPct,
	}

	// Add mount info if available
	if mount != nil {
		du.Device = mount.Device
		du.FSType = mount.FSType
		du.MountPoint = mount.MountPoint
	}

	return du, nil
}
