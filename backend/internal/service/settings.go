package service

import (
	"encoding/json"
	"path/filepath"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type SettingsService interface {
	GetDriveNames() (map[string]string, error)
	SetDriveName(mountPoint, customName string) error
	DeleteDriveName(mountPoint string) error
}

type settingsService struct {
	fs       filesystem.FS
	filePath string
	mu       sync.RWMutex
}

type DriveNamesData struct {
	Mappings map[string]string `json:"mappings"`
}

type SettingsServiceConfig struct {
	DataDir string
}

func NewSettingsService(fsys filesystem.FS, cfg SettingsServiceConfig) SettingsService {
	dataDir := cfg.DataDir
	if dataDir == "" {
		dataDir = config.DefaultDataDir
	}
	filePath := filepath.Join(dataDir, config.DriveNamesFileName)
	return &settingsService{fs: fsys, filePath: filePath}
}

func (s *settingsService) load() (*DriveNamesData, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	data := &DriveNamesData{
		Mappings: make(map[string]string),
	}

	exists, err := s.fs.Exists(s.filePath)
	if err != nil {
		return nil, err
	}
	if !exists {
		return data, nil
	}

	file, err := s.fs.ReadFile(s.filePath)
	if err != nil {
		return nil, err
	}

	if len(file) == 0 {
		return data, nil
	}

	if err := json.Unmarshal(file, data); err != nil {
		return nil, err
	}

	return data, nil
}

func (s *settingsService) save(data *DriveNamesData) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	fileData, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}

	if err := s.fs.WriteFile(s.filePath, fileData, 0644); err != nil {
		return err
	}

	return nil
}

func (s *settingsService) GetDriveNames() (map[string]string, error) {
	data, err := s.load()
	if err != nil {
		return nil, err
	}

	result := make(map[string]string)
	for k, v := range data.Mappings {
		result[k] = v
	}

	return result, nil
}

func (s *settingsService) SetDriveName(mountPoint, customName string) error {
	data, err := s.load()
	if err != nil {
		return err
	}

	if data.Mappings == nil {
		data.Mappings = make(map[string]string)
	}

	data.Mappings[mountPoint] = customName

	return s.save(data)
}

func (s *settingsService) DeleteDriveName(mountPoint string) error {
	data, err := s.load()
	if err != nil {
		return err
	}

	if data.Mappings == nil {
		return nil
	}

	delete(data.Mappings, mountPoint)

	return s.save(data)
}
