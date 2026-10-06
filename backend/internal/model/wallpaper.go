package model

import "time"

// WallpaperDisplay controls how a stored wallpaper is drawn.
type WallpaperDisplay struct {
	Mode         string `json:"mode"`
	FrostedGlass bool   `json:"frostedGlass"`
}

// WallpaperMeta describes a user's stored wallpaper image. Source is the
// client's reference to the wallpaper it uploaded, so clients can tell
// whether the stored copy matches what they show.
type WallpaperMeta struct {
	WallpaperDisplay
	Source      string    `json:"source"`
	SHA256      string    `json:"sha256"`
	ContentType string    `json:"contentType"`
	UpdatedAt   time.Time `json:"updatedAt"`
}

// ShareWallpaper tells a share recipient how to draw the owner's wallpaper.
// Version changes whenever the image or its display settings change.
type ShareWallpaper struct {
	WallpaperDisplay
	Version int64 `json:"version"`
}
