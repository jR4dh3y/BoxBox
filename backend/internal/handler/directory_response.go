package handler

import (
	"path"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// Directory entries share their parent path instead of repeating it on the wire.
type directoryEntry struct {
	Name        string    `json:"name"`
	Size        int64     `json:"size"`
	IsDir       bool      `json:"isDir"`
	ModTime     time.Time `json:"modTime"`
	Permissions string    `json:"permissions"`
	MimeType    string    `json:"mimeType,omitempty"`
}

type directoryResponse struct {
	Path  string           `json:"path"`
	Items []directoryEntry `json:"items"`
}

func directoryListing(list *model.FileList) directoryResponse {
	items := make([]directoryEntry, len(list.Items))
	for i, item := range list.Items {
		items[i] = directoryEntry{Name: item.Name, Size: item.Size, IsDir: item.IsDir, ModTime: item.ModTime, Permissions: item.Permissions, MimeType: item.MimeType}
	}
	return directoryResponse{Path: path.Clean(list.Path), Items: items}
}
