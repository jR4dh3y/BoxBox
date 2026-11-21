//go:build !unix

package service

import "io/fs"

// fileID has no portable source here, so only size and modification time
// identify a shared file.
func fileID(fs.FileInfo) (device uint64, inode uint64) {
	return 0, 0
}
