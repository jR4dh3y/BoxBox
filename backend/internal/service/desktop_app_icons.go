package service

import (
	"bytes"
	"context"
	_ "embed"
	"fmt"
	"image/png"
	"time"
)

//go:embed desktop_app_icon.py
var desktopAppIconScript string

// DesktopAppIcon contains only a bounded raster image, never a host path or executable SVG.
type DesktopAppIcon struct {
	PNG []byte
}

func (s *desktopAppsService) Icon(ctx context.Context, appID string) (DesktopAppIcon, error) {
	if !validDesktopID(appID) {
		return DesktopAppIcon{}, ErrDesktopApplicationUnavailable
	}
	app, exists := s.installedDesktopApps()[appID]
	if !exists || app.application.Icon == "" || !app.acceptsFiles {
		return DesktopAppIcon{}, ErrDesktopApplicationUnavailable
	}
	s.cacheMu.Lock()
	if cached, ok := s.iconCache[appID]; ok && time.Since(cached.loadedAt) < desktopCacheTTL {
		s.cacheMu.Unlock()
		return cached.icon, nil
	}
	s.cacheMu.Unlock()
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	data, err := s.commands.Output(ctx, "python3", "-I", "-c", desktopAppIconScript, app.application.Icon)
	if err != nil {
		return DesktopAppIcon{}, fmt.Errorf("%w: icon helper: %w", ErrDesktopApplicationUnavailable, err)
	}
	if len(data) > 128*1024 {
		return DesktopAppIcon{}, fmt.Errorf("%w: icon exceeds size limit", ErrDesktopApplicationUnavailable)
	}
	config, err := png.DecodeConfig(bytes.NewReader(data))
	if err != nil {
		return DesktopAppIcon{}, fmt.Errorf("%w: invalid icon PNG: %w", ErrDesktopApplicationUnavailable, err)
	}
	if config.Width > 64 || config.Height > 64 {
		return DesktopAppIcon{}, fmt.Errorf("%w: icon dimensions exceed 64 pixels", ErrDesktopApplicationUnavailable)
	}
	icon := DesktopAppIcon{PNG: append([]byte(nil), data...)}
	s.cacheMu.Lock()
	if len(s.iconCache) >= desktopIconCacheLimit {
		var oldest string
		var oldestAt time.Time
		for id, entry := range s.iconCache {
			if oldest == "" || entry.loadedAt.Before(oldestAt) {
				oldest, oldestAt = id, entry.loadedAt
			}
		}
		delete(s.iconCache, oldest)
	}
	s.iconCache[appID] = desktopIconCacheEntry{loadedAt: time.Now(), icon: icon}
	s.cacheMu.Unlock()
	return icon, nil
}
