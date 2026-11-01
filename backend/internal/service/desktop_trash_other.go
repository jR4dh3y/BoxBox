//go:build !linux

package service

import (
	"context"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type unsupportedDesktopTrashService struct{}

func newDesktopTrashService(filesystem.FS, []model.MountPoint, desktopTrashConfig) DesktopTrashService {
	return unsupportedDesktopTrashService{}
}

func (unsupportedDesktopTrashService) List(context.Context) ([]DesktopTrashItem, error) {
	return nil, ErrTrashUnavailable
}

func (unsupportedDesktopTrashService) Trash(context.Context, string) (DesktopTrashItem, error) {
	return DesktopTrashItem{}, ErrTrashUnavailable
}

func (unsupportedDesktopTrashService) Restore(context.Context, string) error {
	return ErrTrashUnavailable
}

func (unsupportedDesktopTrashService) Delete(context.Context, string) error {
	return ErrTrashUnavailable
}
