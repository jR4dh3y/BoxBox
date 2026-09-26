//go:build !linux

package filesystem

import "os"

func renameNoReplaceOS(oldpath, newpath string) error {
	return renameNoReplaceWithLink(oldpath, newpath, os.Link)
}
