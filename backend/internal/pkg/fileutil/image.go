package fileutil

import (
	"path/filepath"
	"strings"
)

// RendersImageThumbnail reports whether a file name has an extension the pure-Go renderer decodes.
func RendersImageThumbnail(name string) bool {
	switch strings.ToLower(filepath.Ext(name)) {
	case ".jpg", ".jpeg", ".png", ".gif", ".webp":
		return true
	}
	return false
}
