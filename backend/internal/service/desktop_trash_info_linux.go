//go:build linux

package service

import (
	"net/url"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

const maxTrashInfoBytes = 1 << 20

func collisionName(original string, collision int) string {
	if collision == 0 {
		return original
	}
	return original + "." + strconv.Itoa(collision)
}

func trashInfoOriginalPath(location desktopTrashLocation, originalPath string) (string, error) {
	base := location.originBase
	if location.home {
		if pathWithinRoot(base, originalPath) {
			return filepath.Rel(base, originalPath)
		}
		return originalPath, nil
	}
	relative, err := filepath.Rel(base, originalPath)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) || filepath.IsAbs(relative) {
		return "", ErrPermissionDenied
	}
	return relative, nil
}

func parseTrashInfo(location desktopTrashLocation, contents []byte) (string, time.Time, bool) {
	if len(contents) > maxTrashInfoBytes {
		return "", time.Time{}, false
	}
	lines := strings.Split(strings.ReplaceAll(string(contents), "\r\n", "\n"), "\n")
	if len(lines) == 0 || lines[0] != "[Trash Info]" {
		return "", time.Time{}, false
	}
	var encodedPath string
	var deletedAt time.Time
	for _, line := range lines[1:] {
		if encodedPath == "" && strings.HasPrefix(line, "Path=") {
			encodedPath = strings.TrimPrefix(line, "Path=")
			continue
		}
		if deletedAt.IsZero() && strings.HasPrefix(line, "DeletionDate=") {
			value := strings.TrimPrefix(line, "DeletionDate=")
			for _, layout := range []string{"2006-01-02T15:04:05", "20060102T15:04:05"} {
				if parsed, err := time.ParseInLocation(layout, value, time.Local); err == nil {
					deletedAt = parsed
					break
				}
			}
		}
	}
	if encodedPath == "" {
		return "", deletedAt, false
	}
	path, err := url.PathUnescape(encodedPath)
	if err != nil || strings.ContainsRune(path, 0) {
		return "", deletedAt, false
	}
	if filepath.IsAbs(path) {
		path = filepath.Clean(path)
		if !location.home && !pathWithinRoot(location.volumeRoot, path) {
			return "", deletedAt, false
		}
		return path, deletedAt, true
	}
	path = filepath.Clean(path)
	if path == "." || path == ".." || strings.HasPrefix(path, ".."+string(filepath.Separator)) || filepath.IsAbs(path) {
		return "", deletedAt, false
	}
	resolved := filepath.Clean(filepath.Join(location.originBase, path))
	if !pathWithinRoot(location.originBase, resolved) || resolved == location.originBase {
		return "", deletedAt, false
	}
	return resolved, deletedAt, true
}
