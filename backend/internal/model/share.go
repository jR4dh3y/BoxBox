package model

import "time"

// SharePermissions defines what a share-link recipient may do with a shared resource.
type SharePermissions struct {
	View     bool `json:"view"`
	Download bool `json:"download"`
	Write    bool `json:"write"`
}

// Share represents a file or folder share link. MountName and RelPath are
// internal bookkeeping for re-resolving the target at access time and are
// never exposed to recipients.
type Share struct {
	ID          string           `json:"id"`
	Token       string           `json:"token"`
	MountName   string           `json:"-"`
	RelPath     string           `json:"-"`
	IsFolder    bool             `json:"isFolder"`
	Permissions SharePermissions `json:"permissions"`
	FileName    string           `json:"fileName"`
	CreatedAt   time.Time        `json:"createdAt"`
	ExpiresAt   time.Time        `json:"expiresAt,omitempty"` // zero means never
	Revoked     bool             `json:"revoked"`
	CreatedBy   string           `json:"createdBy"`
}

// CreateShareRequest is the request body for creating a share link.
// A nil ExpiresInSeconds means the share never expires.
type CreateShareRequest struct {
	Path             string           `json:"path"`
	Permissions      SharePermissions `json:"permissions"`
	ExpiresInSeconds *int64           `json:"expiresInSeconds,omitempty"`
}

// ShareResponse is returned when a share link is created
type ShareResponse struct {
	ID          string           `json:"id"`
	Token       string           `json:"token"`
	URL         string           `json:"url"`
	FileName    string           `json:"fileName"`
	Permissions SharePermissions `json:"permissions"`
	IsFolder    bool             `json:"isFolder"`
	CreatedAt   time.Time        `json:"createdAt"`
	ExpiresAt   time.Time        `json:"expiresAt,omitempty"`
}

// ShareSummary is one active share in the owner's share list
type ShareSummary struct {
	ID          string           `json:"id"`
	Token       string           `json:"token"`
	URL         string           `json:"url"`
	FileName    string           `json:"fileName"`
	Path        string           `json:"path"`
	Permissions SharePermissions `json:"permissions"`
	IsFolder    bool             `json:"isFolder"`
	CreatedAt   time.Time        `json:"createdAt"`
	ExpiresAt   time.Time        `json:"expiresAt,omitempty"`
}

// ShareListResponse is the owner's list of active shares
type ShareListResponse struct {
	Shares []ShareSummary `json:"shares"`
}

// ShareInfoResponse is the recipient-facing share metadata. It deliberately
// omits mount names and internal paths.
type ShareInfoResponse struct {
	FileName    string           `json:"fileName"`
	Size        int64            `json:"size"`
	MimeType    string           `json:"mimeType"`
	Permissions SharePermissions `json:"permissions"`
	IsFolder    bool             `json:"isFolder"`
	ExpiresAt   time.Time        `json:"expiresAt,omitempty"`
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
