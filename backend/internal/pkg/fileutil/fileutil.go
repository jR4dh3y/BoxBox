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

	normalizedExt := strings.ToLower(ext)
	if !strings.HasPrefix(normalizedExt, ".") {
		normalizedExt = "." + normalizedExt
	}

	if mimeType := mime.TypeByExtension(normalizedExt); mimeType != "" {
		return mimeType
	}

	return extensionMimeFallbacks[normalizedExt]
}

// ToFileInfo converts fs.FileInfo to model.FileInfo
// This is a centralized utility function used by file service and search service
func ToFileInfo(name, path string, info fs.FileInfo) model.FileInfo {
	fileInfo := model.FileInfo{
		Name:        name,
		Path:        path,
		Size:        info.Size(),
		IsDir:       info.IsDir(),
		ModTime:     info.ModTime(),
		Permissions: info.Mode().String(),
	}

	// Set MIME type for files
	if !info.IsDir() {
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
