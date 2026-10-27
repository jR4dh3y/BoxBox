// Package fileutil provides shared file utility functions.
package fileutil

import (
	"io/fs"
	"mime"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

var extensionMimeFallbacks = map[string]string{
	".flac": "audio/flac",
	".mp3":  "audio/mpeg",
	".m4a":  "audio/mp4",
	".wav":  "audio/wav",
	".aac":  "audio/aac",
	".oga":  "audio/ogg",
	".opus": "audio/opus",
	".mp4":  "video/mp4",
	".m4v":  "video/mp4",
	".mov":  "video/quicktime",
	".webm": "video/webm",
	".mkv":  "video/x-matroska",
	".avi":  "video/x-msvideo",
	".wmv":  "video/x-ms-wmv",
	".flv":  "video/x-flv",
	".ogv":  "video/ogg",
	".pdf":  "application/pdf",
}

func detectMimeTypeByExtension(ext string) string {
	if ext == "" {
		return ""
	}

	hasLeadingDot := strings.HasPrefix(ext, ".")
	if mimeType := mime.TypeByExtension(ext); mimeType != "" {
		return mimeType
	}
	if !hasLeadingDot {
		if mimeType := mime.TypeByExtension("." + ext); mimeType != "" {
			return mimeType
		}
	}
	for fallbackExtension, mimeType := range extensionMimeFallbacks {
		if strings.EqualFold(ext, fallbackExtension) {
			return mimeType
		}
		if !hasLeadingDot && strings.EqualFold(ext, fallbackExtension[1:]) {
			return mimeType
		}
	}

	return ""
}

// ToFileInfo converts fs.FileInfo to model.FileInfo.
func ToFileInfo(name, path string, info fs.FileInfo) model.FileInfo {
	return ToFileInfoWithPermissions(name, path, info, info.Mode().String())
}

// ToFileInfoWithPermissions converts fs.FileInfo using a reusable permission string.
func ToFileInfoWithPermissions(name, path string, info fs.FileInfo, permissions string) model.FileInfo {
	isDir := info.IsDir()
	fileInfo := model.FileInfo{
		Name:        name,
		Path:        path,
		Size:        info.Size(),
		IsDir:       isDir,
		ModTime:     info.ModTime(),
		Permissions: permissions,
	}

	if !isDir {
		ext := filepath.Ext(name)
		if mimeType := detectMimeTypeByExtension(ext); mimeType != "" {
			fileInfo.MimeType = mimeType
		}
	}

	return fileInfo
}

// DetectMimeType returns the MIME type for a file based on its extension
// Returns "application/octet-stream" if the type cannot be determined
func DetectMimeType(filename string) string {
	if mimeType := detectMimeTypeByExtension(filepath.Ext(filename)); mimeType != "" {
		return mimeType
	}
	return "application/octet-stream"
}
