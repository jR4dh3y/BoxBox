//go:build unix

package service

import (
	"io/fs"
	"syscall"
)

// fileID returns the device and inode that name a file on this host.
func fileID(info fs.FileInfo) (device uint64, inode uint64) {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return 0, 0
	}
	return uint64(stat.Dev), uint64(stat.Ino)
}
