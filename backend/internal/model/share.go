package model

import (
	"encoding/json"
	"time"
)

// SharePermissions defines what a share-link recipient may do with a shared resource.
type SharePermissions struct {
	View          bool `json:"view"`
	Download      bool `json:"download"`
	Upload        bool `json:"upload"`
	Delete        bool `json:"delete"`
	LegacyReplace bool `json:"-"`
}

// UnmarshalJSON keeps existing share records and older clients' write flag
// readable while exposing upload and delete as separate capabilities.
func (p *SharePermissions) UnmarshalJSON(data []byte) error {
	var value struct {
		View     bool  `json:"view"`
		Download bool  `json:"download"`
		Upload   *bool `json:"upload"`
		Delete   bool  `json:"delete"`
		Write    bool  `json:"write"`
	}
	if err := json.Unmarshal(data, &value); err != nil {
		return err
	}
	p.View = value.View
	p.Download = value.Download
	p.Upload = value.Write
	if value.Upload != nil {
		p.Upload = *value.Upload
	}
	p.Delete = value.Delete
	p.LegacyReplace = value.Upload == nil && value.Write
	return nil
}

// SharePermissionsResponse reports the effective permissions to share clients.
type SharePermissionsResponse struct {
	View       bool `json:"view"`
	Download   bool `json:"download"`
	Upload     bool `json:"upload"`
	Delete     bool `json:"delete"`
	CanReplace bool `json:"canReplace"`
}

func (p SharePermissions) ToResponse() SharePermissionsResponse {
	return SharePermissionsResponse{
		View:       p.View,
		Download:   p.Download,
		Upload:     p.Upload,
		Delete:     p.Delete,
		CanReplace: p.Delete || p.LegacyReplace,
	}
}

// Share represents a file or folder share link. MountName and RelPath are
// internal bookkeeping for re-resolving the target at access time and are
// never exposed to recipients.
type Share struct {
	ID             string           `json:"id"`
	Token          string           `json:"token"`
	MountName      string           `json:"-"`
	RelPath        string           `json:"-"`
	IsFolder       bool             `json:"isFolder"`
	Permissions    SharePermissions `json:"permissions"`
	MaxUploadBytes int64            `json:"maxUploadBytes"`
	FileName       string           `json:"fileName"`
	CreatedAt      time.Time        `json:"createdAt"`
	ExpiresAt      time.Time        `json:"expiresAt,omitempty"` // zero means never
	Revoked        bool             `json:"revoked"`
	CreatedBy      string           `json:"createdBy"`
}

// CreateShareRequest is the request body for creating a share link.
// A nil ExpiresInSeconds means the share never expires.
type CreateShareRequest struct {
	Path             string           `json:"path"`
	Permissions      SharePermissions `json:"permissions"`
	MaxUploadBytes   int64            `json:"maxUploadBytes,omitempty"`
	ExpiresInSeconds *int64           `json:"expiresInSeconds,omitempty"`
}

// UpdateShareRequest changes the access granted by an active folder share.
type UpdateShareRequest struct {
	Permissions    SharePermissions `json:"permissions"`
	MaxUploadBytes *int64           `json:"maxUploadBytes,omitempty"`
}

// ShareResponse is returned when a share link is created
type ShareResponse struct {
	ID             string                   `json:"id"`
	Token          string                   `json:"token"`
	URL            string                   `json:"url"`
	FileName       string                   `json:"fileName"`
	Permissions    SharePermissionsResponse `json:"permissions"`
	MaxUploadBytes int64                    `json:"maxUploadBytes"`
	IsFolder       bool                     `json:"isFolder"`
	CreatedAt      time.Time                `json:"createdAt"`
	ExpiresAt      time.Time                `json:"expiresAt,omitempty"`
}

// ShareSummary is one active share in the owner's share list
type ShareSummary struct {
	ID             string                   `json:"id"`
	Token          string                   `json:"token"`
	URL            string                   `json:"url"`
	FileName       string                   `json:"fileName"`
	Path           string                   `json:"path"`
	Permissions    SharePermissionsResponse `json:"permissions"`
	MaxUploadBytes int64                    `json:"maxUploadBytes"`
	IsFolder       bool                     `json:"isFolder"`
	CreatedAt      time.Time                `json:"createdAt"`
	ExpiresAt      time.Time                `json:"expiresAt,omitempty"`
}

// ShareListResponse is the owner's list of active shares
type ShareListResponse struct {
	Shares []ShareSummary `json:"shares"`
}

// ShareInfoResponse is the recipient-facing share metadata. It deliberately
// omits mount names and internal paths.
type ShareInfoResponse struct {
	FileName       string                   `json:"fileName"`
	Size           int64                    `json:"size"`
	MimeType       string                   `json:"mimeType"`
	Permissions    SharePermissionsResponse `json:"permissions"`
	MaxUploadBytes int64                    `json:"maxUploadBytes"`
	IsFolder       bool                     `json:"isFolder"`
	ExpiresAt      time.Time                `json:"expiresAt,omitempty"`
}

// ShareItem is a directory entry exposed below a shared folder. Path is
// relative to the shared folder and never contains the owner's mount path.
type ShareItem struct {
	Name     string    `json:"name"`
	Path     string    `json:"path"`
	Size     int64     `json:"size"`
	IsDir    bool      `json:"isDir"`
	ModTime  time.Time `json:"modTime"`
	MimeType string    `json:"mimeType,omitempty"`
}

// ShareDirectoryResponse contains the entries in a shared folder.
type ShareDirectoryResponse struct {
	Path  string      `json:"path"`
	Items []ShareItem `json:"items"`
}

// ShareUploadResponse is returned after a recipient adds or replaces a file.
type ShareUploadResponse struct {
	FileName string `json:"fileName"`
	Size     int64  `json:"size"`
}
