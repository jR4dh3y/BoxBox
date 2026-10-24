package service

import (
	"crypto/rand"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

var ErrDesktopPreferences = errors.New("desktop preferences unavailable")
var ErrDesktopPreferencesInput = errors.New("invalid desktop preferences")

// DesktopPreferences stores renderer preferences independently of the ephemeral loopback origin.
type DesktopPreferences struct {
	mu   sync.Mutex
	fs   filesystem.FS
	path string
}

func NewDesktopPreferences(fs filesystem.FS, dataDir string) *DesktopPreferences {
	return &DesktopPreferences{fs: fs, path: filepath.Join(dataDir, "desktop", "preferences.json")}
}

func (s *DesktopPreferences) Get() (json.RawMessage, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	info, err := s.fs.Lstat(s.path)
	if errors.Is(err, os.ErrNotExist) {
		return json.RawMessage("null"), nil
	}
	if err != nil || !info.Mode().IsRegular() || info.Size() > 128<<10 {
		return nil, ErrDesktopPreferences
	}
	file, err := s.fs.Open(s.path)
	if err != nil {
		return nil, ErrDesktopPreferences
	}
	defer file.Close()
	value, err := io.ReadAll(io.LimitReader(file, (128<<10)+1))
	if err != nil || len(value) > 128<<10 || !validPreferences(value) {
		return nil, ErrDesktopPreferences
	}
	return value, nil
}

func validPreferences(value []byte) bool {
	var object map[string]json.RawMessage
	return len(value) <= 128<<10 && json.Unmarshal(value, &object) == nil && object != nil
}

func (s *DesktopPreferences) Set(value json.RawMessage) error {
	if !validPreferences(value) {
		return ErrDesktopPreferencesInput
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	directory := filepath.Dir(s.path)
	if err := s.fs.MkdirAll(directory, 0700); err != nil {
		return ErrDesktopPreferences
	}
	info, err := s.fs.Lstat(directory)
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return ErrDesktopPreferences
	}
	if err = s.fs.Chmod(directory, 0700); err != nil {
		return ErrDesktopPreferences
	}
	temporary := s.path + ".tmp." + rand.Text()
	file, err := s.fs.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		return ErrDesktopPreferences
	}
	defer s.fs.Remove(temporary)
	if _, err = file.Write(value); err != nil {
		file.Close()
		return ErrDesktopPreferences
	}
	if err = file.Sync(); err != nil {
		file.Close()
		return ErrDesktopPreferences
	}
	if err = file.Close(); err != nil {
		return ErrDesktopPreferences
	}
	if err = s.fs.Rename(temporary, s.path); err != nil {
		return ErrDesktopPreferences
	}
	dir, err := s.fs.Open(directory)
	if err != nil {
		return ErrDesktopPreferences
	}
	defer dir.Close()
	if err = dir.Sync(); err != nil {
		return ErrDesktopPreferences
	}
	return nil
}
