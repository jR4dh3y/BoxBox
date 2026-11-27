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

// FileList is every entry of one directory, unsorted
type FileList struct {
	Path  string     `json:"path"`
	Items []FileInfo `json:"items"`
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

// ItemProperties is what Properties shows of one item beyond its listing entry. Owner, group
// and the access and change times are left out where the platform does not tell them.
type ItemProperties struct {
	Path     string     `json:"path"`
	Owner    string     `json:"owner,omitempty"`
	Group    string     `json:"group,omitempty"`
	Accessed *time.Time `json:"accessed,omitempty"`
	Changed  *time.Time `json:"changed,omitempty"`
}

// PropertiesResponse is the selection's items and everything they hold, folders walked.
type PropertiesResponse struct {
	Items   []ItemProperties `json:"items"`
	Files   int64            `json:"files"`
	Folders int64            `json:"folders"`
	Bytes   int64            `json:"bytes"`
	// False when the walk stopped at its limit, so the totals are a lower bound.
	Complete bool `json:"complete"`
}
