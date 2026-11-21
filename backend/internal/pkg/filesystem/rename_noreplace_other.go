//go:build !linux

package filesystem

import "os"

func renameNoReplaceOS(oldpath, newpath string) error {
	return renameNoReplaceWithLink(oldpath, newpath, os.Link)
}

// restoreEntry moves from back to to without replacing an entry at to.
func restoreEntry(directory *os.Root, from, to string) error {
	return restoreByLink(directory, from, to)
}
