package service

import (
	"context"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// SystemService provides system-level information
type SystemService interface {
	// GetAllDrives returns all mounted filesystems from the system
	GetAllDrives(ctx context.Context) (*model.SystemDrivesResponse, error)
}

// systemService implements SystemService
type systemService struct{}

// NewSystemService creates a new system service
func NewSystemService() SystemService {
	return &systemService{}
}

// GetAllDrives returns all mounted filesystems using platform-specific implementation
func (s *systemService) GetAllDrives(ctx context.Context) (*model.SystemDrivesResponse, error) {
	drives, err := getAllSystemDrives()
	if err != nil {
		return nil, err
	}
	return &model.SystemDrivesResponse{Drives: drives}, nil
}
