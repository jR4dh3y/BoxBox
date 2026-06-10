package model

import "time"

// FileInfo represents metadata for a file or directory
type FileInfo struct {
	Name        string    `json:"name"`
	Path        string    `json:"path"`
	Size        int64     `json:"size"`
	IsDir       bool      `json:"isDir"`
	ModTime     time.Time `json:"modTime"`
	Permissions string    `json:"permissions"`
	MimeType    string    `json:"mimeType,omitempty"`
}

// FileList represents a paginated list of files in a directory
type FileList struct {
	Path       string     `json:"path"`
	Items      []FileInfo `json:"items"`
	TotalCount int        `json:"totalCount"`
	Page       int        `json:"page"`
	PageSize   int        `json:"pageSize"`
}

// ListOptions contains options for listing directory contents
type ListOptions struct {
	Page          int    `json:"page"`
	PageSize      int    `json:"pageSize"`
	SortBy        string `json:"sortBy"`
	SortDir       string `json:"sortDir"`
	Filter        string `json:"filter"`
	IncludeHidden bool   `json:"includeHidden"`
}

// DefaultListOptions returns sensible defaults for listing
func DefaultListOptions() ListOptions {
	return ListOptions{
		Page:          1,
		PageSize:      50,
		SortBy:        "name",
		SortDir:       "asc",
		Filter:        "",
		IncludeHidden: true,
	}
}

// DriveStats represents disk usage statistics for a mount point
type DriveStats struct {
	Name       string  `json:"name"`
	Path       string  `json:"path"`
	Device     string  `json:"device,omitempty"`     // The underlying device (e.g., /dev/sda1)
	FSType     string  `json:"fsType,omitempty"`     // Filesystem type (e.g., ext4, ntfs)
	MountPoint string  `json:"mountPoint,omitempty"` // Actual mount point in the system
	TotalBytes uint64  `json:"totalBytes"`
	FreeBytes  uint64  `json:"freeBytes"`
	UsedBytes  uint64  `json:"usedBytes"`
	UsedPct    float64 `json:"usedPct"`
	ReadOnly   bool    `json:"readOnly"`
}

// DriveStatsResponse represents the response for drive stats endpoint
type DriveStatsResponse struct {
	Drives []DriveStats `json:"drives"`
}

// SystemDrive represents a filesystem/drive discovered from the system (via df)
type SystemDrive struct {
	Device     string  `json:"device"`           // The device name (e.g., /dev/sda1)
	MountPoint string  `json:"mountPoint"`       // Where it's mounted (e.g., /home)
	FSType     string  `json:"fsType,omitempty"` // Filesystem type (e.g., ext4, ntfs)
	TotalBytes uint64  `json:"totalBytes"`       // Total size in bytes
	UsedBytes  uint64  `json:"usedBytes"`        // Used space in bytes
	FreeBytes  uint64  `json:"freeBytes"`        // Available space in bytes
	UsedPct    float64 `json:"usedPct"`          // Percentage used
}

// SystemDrivesResponse represents the response for system drives endpoint
type SystemDrivesResponse struct {
	Drives []SystemDrive `json:"drives"`
}
