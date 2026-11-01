package service

import (
	"github.com/fsnotify/fsnotify"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type DesktopWatcher struct {
	fs     filesystem.FS
	mounts *mounts
}

func NewDesktopWatcher(fsys filesystem.FS, list []model.MountPoint) *DesktopWatcher {
	return &DesktopWatcher{fs: fsys, mounts: newMounts(fsys, fixedMounts(list))}
}

// Watch tracks the visible directory, not a recursive scan of the user's home.
func (s *DesktopWatcher) Watch(path string) (*fsnotify.Watcher, error) {
	_, resolved, err := s.mounts.resolve(path, readExisting)
	if err != nil {
		return nil, err
	}
	info, err := s.fs.Stat(resolved)
	if err != nil {
		return nil, err
	}
	if !info.IsDir() {
		return nil, ErrNotDirectory
	}
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}
	if err := watcher.Add(resolved); err != nil {
		watcher.Close()
		return nil, err
	}
	return watcher, nil
}
