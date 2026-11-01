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
	app, exists := readInstalledDesktopApps(s.commands)[appID]
	if !exists || app.application.Icon == "" || !app.acceptsFiles {
		return DesktopAppIcon{}, ErrDesktopApplicationUnavailable
	}
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	data, err := s.commands.Output(ctx, "python3", "-I", "-c", desktopAppIconScript, app.application.Icon)
	if err != nil {
		return DesktopAppIcon{}, fmt.Errorf("%w: icon helper: %v", ErrDesktopApplicationUnavailable, err)
	}
	if len(data) > 128*1024 {
		return DesktopAppIcon{}, fmt.Errorf("%w: icon exceeds size limit", ErrDesktopApplicationUnavailable)
	}
	config, err := png.DecodeConfig(bytes.NewReader(data))
	if err != nil || config.Width > 64 || config.Height > 64 {
		return DesktopAppIcon{}, fmt.Errorf("%w: invalid icon PNG: %v", ErrDesktopApplicationUnavailable, err)
	}
	return DesktopAppIcon{PNG: data}, nil
}
