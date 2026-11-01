//go:build linux

package service

import (
	"golang.org/x/sys/unix"
)

func readVolumeSpace(path string) (volumeSpace, error) {
	if path == "" {
		return volumeSpace{}, unix.EINVAL
	}
	var stats unix.Statfs_t
	if err := unix.Statfs(path, &stats); err != nil {
		return volumeSpace{}, err
	}
	blockSize := uint64(stats.Bsize)
	return volumeSpace{
		total:    stats.Blocks * blockSize,
		free:     stats.Bavail * blockSize,
		readOnly: stats.Flags&unix.ST_RDONLY != 0,
	}, nil
}
