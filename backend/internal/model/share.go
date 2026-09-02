package model

import "time"

// SharePermissions defines what a share-link recipient may do with the shared file
type SharePermissions struct {
	View     bool `json:"view"`
	Download bool `json:"download"`
	Write    bool `json:"write"`
}

// Share represents a single-file share link. MountName and RelPath are internal
// bookkeeping for re-resolving the target at access time and are never exposed
// to recipients.
type Share struct {
	ID          string           `json:"id"`
	Token       string           `json:"token"`
	MountName   string           `json:"-"`
	RelPath     string           `json:"-"`
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
	ExpiresAt   time.Time        `json:"expiresAt,omitempty"`
}

// ShareUploadResponse is returned after a recipient overwrites the shared file
type ShareUploadResponse struct {
	FileName string `json:"fileName"`
	Size     int64  `json:"size"`
}
