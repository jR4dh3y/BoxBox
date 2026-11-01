//go:build !linux

package service

import (
	"context"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func NewDesktopThumbnailRenderer(_ []string) ThumbnailRenderer {
	return func(context.Context, File, *model.FileInfo, int, int) ([]byte, error) {
		return nil, ErrThumbnailUnavailable
	}
}
