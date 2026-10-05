package service

import (
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/http"
	"path/filepath"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

var (
	ErrWallpaperNotFound = errors.New("wallpaper not found")
	ErrInvalidWallpaper  = errors.New("invalid wallpaper")
)

var wallpaperModes = map[string]bool{
	"cover":   true,
	"contain": true,
	"stretch": true,
	"center":  true,
	"tile":    true,
}

var wallpaperContentTypes = map[string]bool{
	"image/jpeg": true,
	"image/png":  true,
	"image/gif":  true,
	"image/webp": true,
}

// GetWallpaper returns the user's wallpaper metadata, or ErrWallpaperNotFound.
func (s *settingsService) GetWallpaper(username string) (*model.WallpaperMeta, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.readWallpaperMetaLocked(username)
}

// OpenWallpaper returns the user's wallpaper image and metadata.
func (s *settingsService) OpenWallpaper(username string) ([]byte, *model.WallpaperMeta, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	meta, err := s.readWallpaperMetaLocked(username)
	if err != nil {
		return nil, nil, err
	}
	image, err := s.fs.ReadFile(s.wallpaperImagePath(username))
	if err != nil {
		return nil, nil, ErrWallpaperNotFound
	}
	return image, meta, nil
}

// maxWallpaperSourceLength bounds the stored client reference to the wallpaper.
const maxWallpaperSourceLength = 2048

// SetWallpaper stores a new wallpaper image with its display settings and the
// client's reference to it.
func (s *settingsService) SetWallpaper(username string, image []byte, display model.WallpaperDisplay, source string) (*model.WallpaperMeta, error) {
	if username == "" || len(image) == 0 || len(image) > config.MaxWallpaperBytes || !wallpaperModes[display.Mode] || len(source) > maxWallpaperSourceLength {
		return nil, ErrInvalidWallpaper
	}
	contentType := http.DetectContentType(image)
	if !wallpaperContentTypes[contentType] {
		return nil, ErrInvalidWallpaper
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.fs.MkdirAll(s.wallpapersDir(), 0755); err != nil {
		return nil, err
	}
	if err := s.writeFileAtomicLocked(s.wallpaperImagePath(username), image); err != nil {
		return nil, err
	}
	meta := &model.WallpaperMeta{
		WallpaperDisplay: display,
		Source:           source,
		ContentType:      contentType,
		UpdatedAt:        time.Now().UTC(),
	}
	if err := s.writeWallpaperMetaLocked(username, meta); err != nil {
		return nil, err
	}
	return meta, nil
}

// SetWallpaperDisplay changes how the user's existing wallpaper is drawn.
func (s *settingsService) SetWallpaperDisplay(username string, display model.WallpaperDisplay) (*model.WallpaperMeta, error) {
	if !wallpaperModes[display.Mode] {
		return nil, ErrInvalidWallpaper
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	meta, err := s.readWallpaperMetaLocked(username)
	if err != nil {
		return nil, err
	}
	meta.WallpaperDisplay = display
	meta.UpdatedAt = time.Now().UTC()
	if err := s.writeWallpaperMetaLocked(username, meta); err != nil {
		return nil, err
	}
	return meta, nil
}

// DeleteWallpaper removes the user's wallpaper. Deleting a missing wallpaper is not an error.
func (s *settingsService) DeleteWallpaper(username string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	for _, path := range []string{s.wallpaperMetaPath(username), s.wallpaperImagePath(username)} {
		exists, err := s.fs.Exists(path)
		if err != nil {
			return err
		}
		if exists {
			if err := s.fs.Remove(path); err != nil {
				return err
			}
		}
	}
	return nil
}

func (s *settingsService) readWallpaperMetaLocked(username string) (*model.WallpaperMeta, error) {
	if username == "" {
		return nil, ErrWallpaperNotFound
	}
	data, err := s.fs.ReadFile(s.wallpaperMetaPath(username))
	if err != nil {
		return nil, ErrWallpaperNotFound
	}
	var meta model.WallpaperMeta
	if err := json.Unmarshal(data, &meta); err != nil {
		return nil, err
	}
	return &meta, nil
}

func (s *settingsService) writeWallpaperMetaLocked(username string, meta *model.WallpaperMeta) error {
	data, err := json.MarshalIndent(meta, "", "  ")
	if err != nil {
		return err
	}
	return s.writeFileAtomicLocked(s.wallpaperMetaPath(username), data)
}

func (s *settingsService) writeFileAtomicLocked(path string, data []byte) error {
	tmpPath := path + ".tmp"
	if err := s.fs.WriteFile(tmpPath, data, 0644); err != nil {
		return err
	}
	return s.fs.Rename(tmpPath, path)
}

func (s *settingsService) wallpapersDir() string {
	return filepath.Join(s.dataDir, config.WallpapersDirName)
}

// Usernames are hex-encoded so they can never form a path.
func (s *settingsService) wallpaperImagePath(username string) string {
	return filepath.Join(s.wallpapersDir(), hex.EncodeToString([]byte(username))+".img")
}

func (s *settingsService) wallpaperMetaPath(username string) string {
	return filepath.Join(s.wallpapersDir(), hex.EncodeToString([]byte(username))+".json")
}
