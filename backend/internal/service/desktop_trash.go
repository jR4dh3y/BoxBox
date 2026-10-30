package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

var (
	ErrTrashUnavailable   = errors.New("desktop trash is unavailable")
	ErrTrashItemNotFound  = errors.New("desktop trash item not found")
	ErrTrashConflict      = errors.New("the original path already exists")
	ErrTrashCannotRestore = errors.New("desktop trash item cannot be restored")
	ErrInvalidTrashItem   = errors.New("invalid desktop trash item")
)

// DesktopTrashItem describes one top-level item in the freedesktop trash.
type DesktopTrashItem struct {
	ID           string    `json:"id"`
	Name         string    `json:"name"`
	OriginalPath string    `json:"originalPath"`
	DeletedAt    time.Time `json:"deletedAt"`
	IsDir        bool      `json:"isDir"`
	CanRestore   bool      `json:"canRestore"`
}

// DesktopTrashService provides the trash actions available to the local desktop app.
type DesktopTrashService interface {
	List(ctx context.Context) ([]DesktopTrashItem, error)
	Trash(ctx context.Context, virtualPath string) (DesktopTrashItem, error)
	Restore(ctx context.Context, id string) error
	Delete(ctx context.Context, id string) error
}

// NewDesktopTrashService creates a freedesktop-compatible trash service for the
// current user and the supplied virtual mount points.
func NewDesktopTrashService(fsys filesystem.FS, mountPoints []model.MountPoint) DesktopTrashService {
	dataHome := os.Getenv("XDG_DATA_HOME")
	if dataHome == "" || !filepath.IsAbs(dataHome) {
		if home, err := os.UserHomeDir(); err == nil {
			dataHome = filepath.Join(home, ".local", "share")
		}
	}
	return newDesktopTrashService(fsys, mountPoints, desktopTrashConfig{
		dataHome:      dataHome,
		mountInfoPath: "/proc/self/mountinfo",
		now:           time.Now,
	})
}

type desktopTrashConfig struct {
	dataHome      string
	mountInfoPath string
	now           func() time.Time
}
