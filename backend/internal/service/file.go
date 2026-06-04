// Package service provides business logic for the file manager.
package service

import (
	"context"
	"errors"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/homelab/filemanager/internal/model"
	"github.com/homelab/filemanager/internal/pkg/filesystem"
	"github.com/homelab/filemanager/internal/pkg/fileutil"
	"github.com/homelab/filemanager/internal/pkg/validator"
)

// File service errors
var (
	ErrPathNotFound       = errors.New("path not found")
	ErrPathExists         = errors.New("path already exists")
	ErrNotDirectory       = errors.New("path is not a directory")
	ErrNotFile            = errors.New("path is not a file")
	ErrPermissionDenied   = errors.New("permission denied")
	ErrInvalidOperation   = errors.New("invalid operation")
	ErrMountPointNotFound = errors.New("mount point not found")
)

// File represents an open file that can be read and seeked
type File interface {
	io.Reader
	io.Seeker
	io.Closer
}

// WriteFile represents an open file that can be written to
type WriteFile interface {
	io.Writer
	io.Closer
}

// FileService defines the file operations service interface
type FileService interface {
	// List returns a paginated list of files in a directory
	List(ctx context.Context, path string, opts model.ListOptions) (*model.FileList, error)
	// GetInfo returns metadata for a file or directory
	GetInfo(ctx context.Context, path string) (*model.FileInfo, error)
	// CreateDir creates a new directory
	CreateDir(ctx context.Context, path string) error
	// Rename renames/moves a file or directory
	Rename(ctx context.Context, oldPath, newPath string) error
	// Delete removes a file or directory
	Delete(ctx context.Context, path string) error
	// ListMountPoints returns all configured mount points
	ListMountPoints() []model.MountPoint
	// GetDriveStats returns disk usage statistics for all mount points
	GetDriveStats(ctx context.Context) (*model.DriveStatsResponse, error)
	// ResolvePath resolves a virtual path to a filesystem path
	ResolvePath(path string) (*model.MountPoint, string, error)
	// OpenFile opens a file for reading using the filesystem abstraction
	OpenFile(ctx context.Context, path string) (File, *model.FileInfo, error)
	// CreateFile creates a new file for writing using the filesystem abstraction
	CreateFile(ctx context.Context, path string) (WriteFile, error)
	// WriteFile overwrites an existing file using the filesystem abstraction
	WriteFile(ctx context.Context, path string, content []byte) error
	// GetFilesystem returns the underlying filesystem for advanced operations
	GetFilesystem() filesystem.FS
}

// fileService implements FileService
type fileService struct {
	fs          filesystem.FS
	mountPoints []model.MountPoint
}

// FileServiceConfig holds configuration for the file service
type FileServiceConfig struct {
	MountPoints []model.MountPoint
}

// NewFileService creates a new file service
func NewFileService(fsys filesystem.FS, cfg FileServiceConfig) FileService {
	return &fileService{
		fs:          fsys,
		mountPoints: cfg.MountPoints,
	}
}

// ListMountPoints returns all configured mount points
func (s *fileService) ListMountPoints() []model.MountPoint {
	return s.mountPoints
}

// GetDriveStats returns disk usage statistics for all mount points
// Mount points with auto_discover enabled are expanded to their discovered sub-mounts
func (s *fileService) GetDriveStats(ctx context.Context) (*model.DriveStatsResponse, error) {
	// Expand auto-discover mount points
	effectiveMounts := DiscoverMountPoints(s.fs, s.mountPoints)
	drives := make([]model.DriveStats, 0, len(effectiveMounts))

	for _, mount := range effectiveMounts {
		stats, err := getDiskUsage(mount.Path)
		if err != nil {
			// Skip mounts we can't stat, but continue with others
			continue
		}

		drives = append(drives, model.DriveStats{
			Name:       mount.Name,
			Path:       mount.Path,
			Device:     stats.Device,
			FSType:     stats.FSType,
			MountPoint: stats.MountPoint,
			TotalBytes: stats.Total,
			FreeBytes:  stats.Free,
			UsedBytes:  stats.Used,
			UsedPct:    stats.UsedPct,
			ReadOnly:   mount.ReadOnly,
		})
	}

	return &model.DriveStatsResponse{Drives: drives}, nil
}

// diskUsage holds disk usage statistics
type diskUsage struct {
	Total      uint64
	Free       uint64
	Used       uint64
	UsedPct    float64
	Device     string // The underlying device (e.g., /dev/sda1)
	FSType     string // Filesystem type (e.g., ext4, ntfs)
	MountPoint string // Actual mount point in the system
}

// ResolvePath resolves a virtual path to a mount point and filesystem path
func (s *fileService) ResolvePath(path string) (*model.MountPoint, string, error) {
	return validator.ValidatePathAgainstMounts(path, s.mountPoints)
}

// List returns a paginated list of files in a directory
func (s *fileService) List(ctx context.Context, path string, opts model.ListOptions) (*model.FileList, error) {
	// Apply defaults
	if opts.Page < 1 {
		opts.Page = 1
	}
	if opts.PageSize < 1 {
		opts.PageSize = 50
	}
	if opts.SortBy == "" {
		opts.SortBy = "name"
	}
	if opts.SortDir == "" {
		opts.SortDir = "asc"
	}

	// Resolve the path to filesystem path
	_, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, ErrMountPointNotFound
		}
		return nil, err
	}

	// Check if path exists and is a directory
	info, err := s.fs.Stat(fsPath)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, ErrPathNotFound
		}
		return nil, err
	}
	if !info.IsDir() {
		return nil, ErrNotDirectory
	}

	// Read directory entries
	entries, err := s.fs.ReadDir(fsPath)
	if err != nil {
		return nil, err
	}

	// Apply filter
	var filtered []fs.DirEntry
	filterLower := strings.ToLower(opts.Filter)
	for _, entry := range entries {
		if opts.Filter == "" || strings.Contains(strings.ToLower(entry.Name()), filterLower) {
			filtered = append(filtered, entry)
		}
	}

	totalCount := len(filtered)

	// Sort entries
	s.sortEntries(filtered, opts.SortBy, opts.SortDir)

	// Paginate
	start := (opts.Page - 1) * opts.PageSize
	end := start + opts.PageSize
	if start > len(filtered) {
		start = len(filtered)
	}
	if end > len(filtered) {
		end = len(filtered)
	}
	page := filtered[start:end]

	// Convert to FileInfo
	items := make([]model.FileInfo, 0, len(page))
	for _, entry := range page {
		entryInfo, err := entry.Info()
		if err != nil {
			continue
		}
		items = append(items, fileutil.ToFileInfo(entry.Name(), filepath.Join(path, entry.Name()), entryInfo))
	}

	return &model.FileList{
		Path:       path,
		Items:      items,
		TotalCount: totalCount,
		Page:       opts.Page,
		PageSize:   opts.PageSize,
	}, nil
}

// GetInfo returns metadata for a file or directory
func (s *fileService) GetInfo(ctx context.Context, path string) (*model.FileInfo, error) {
	// Resolve the path to filesystem path
	_, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, ErrMountPointNotFound
		}
		return nil, err
	}

	// Get file info
	info, err := s.fs.Stat(fsPath)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, ErrPathNotFound
		}
		return nil, err
	}

	fileInfo := fileutil.ToFileInfo(info.Name(), path, info)
	return &fileInfo, nil
}

// CreateDir creates a new directory
func (s *fileService) CreateDir(ctx context.Context, path string) error {
	// Resolve the path to filesystem path
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return ErrMountPointNotFound
		}
		return err
	}

	// Check if mount is read-only
	if mount.ReadOnly {
		return ErrPermissionDenied
	}

	// Check if path already exists
	exists, err := s.fs.Exists(fsPath)
	if err != nil {
		return err
	}
	if exists {
		return ErrPathExists
	}

	// Create directory
	return s.fs.MkdirAll(fsPath, 0755)
}

// Rename renames/moves a file or directory
func (s *fileService) Rename(ctx context.Context, oldPath, newPath string) error {
	// Resolve old path
	oldMount, oldFsPath, err := s.ResolvePath(oldPath)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return ErrMountPointNotFound
		}
		return err
	}

	// Check if old mount is read-only
	if oldMount.ReadOnly {
		return ErrPermissionDenied
	}

	// Resolve new path
	newMount, newFsPath, err := s.ResolvePath(newPath)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return ErrMountPointNotFound
		}
		return err
	}

	// Check if new mount is read-only
	if newMount.ReadOnly {
		return ErrPermissionDenied
	}

	// Check if old path exists
	exists, err := s.fs.Exists(oldFsPath)
	if err != nil {
		return err
	}
	if !exists {
		return ErrPathNotFound
	}

	// Check if new path already exists
	exists, err = s.fs.Exists(newFsPath)
	if err != nil {
		return err
	}
	if exists {
		return ErrPathExists
	}

	// Perform rename
	return s.fs.Rename(oldFsPath, newFsPath)
}

// Delete removes a file or directory
func (s *fileService) Delete(ctx context.Context, path string) error {
	// Resolve the path to filesystem path
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return ErrMountPointNotFound
		}
		return err
	}

	// Check if mount is read-only
	if mount.ReadOnly {
		return ErrPermissionDenied
	}

	// Check if path exists
	exists, err := s.fs.Exists(fsPath)
	if err != nil {
		return err
	}
	if !exists {
		return ErrPathNotFound
	}

	// Remove file or directory
	return s.fs.RemoveAll(fsPath)
}

// OpenFile opens a file for reading using the filesystem abstraction
func (s *fileService) OpenFile(ctx context.Context, path string) (File, *model.FileInfo, error) {
	// Resolve the path to filesystem path
	_, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, nil, ErrMountPointNotFound
		}
		return nil, nil, err
	}

	// Get file info
	info, err := s.fs.Stat(fsPath)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, nil, ErrPathNotFound
		}
		return nil, nil, err
	}

	// Check if it's a directory
	if info.IsDir() {
		return nil, nil, ErrNotFile
	}

	// Open the file
	file, err := s.fs.Open(fsPath)
	if err != nil {
		return nil, nil, err
	}

	fileInfo := fileutil.ToFileInfo(info.Name(), path, info)
	return file, &fileInfo, nil
}

// CreateFile creates a new file for writing using the filesystem abstraction
func (s *fileService) CreateFile(ctx context.Context, path string) (WriteFile, error) {
	// Resolve the path to filesystem path
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, ErrMountPointNotFound
		}
		return nil, err
	}

	// Check if mount is read-only
	if mount.ReadOnly {
		return nil, ErrPermissionDenied
	}

	// New-file creation must not truncate an existing item.
	exists, err := s.fs.Exists(fsPath)
	if err != nil {
		return nil, err
	}
	if exists {
		return nil, ErrPathExists
	}

	// Ensure parent directory exists
	parentDir := filepath.Dir(fsPath)
	if err := s.fs.MkdirAll(parentDir, 0755); err != nil {
		return nil, err
	}

	// Create the file
	file, err := s.fs.OpenFile(fsPath, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
	if err != nil {
		return nil, err
	}

	return file, nil
}

// WriteFile overwrites an existing file using the filesystem abstraction
func (s *fileService) WriteFile(ctx context.Context, path string, content []byte) error {
	// Resolve the path to filesystem path
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return ErrMountPointNotFound
		}
		return err
	}

	// Check if mount is read-only
	if mount.ReadOnly {
		return ErrPermissionDenied
	}

	info, err := s.fs.Stat(fsPath)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return ErrPathNotFound
		}
		return err
	}
	if info.IsDir() {
		return ErrNotFile
	}

	return s.fs.WriteFile(fsPath, content, info.Mode().Perm())
}

// GetFilesystem returns the underlying filesystem for advanced operations
func (s *fileService) GetFilesystem() filesystem.FS {
	return s.fs
}

// sortEntries sorts directory entries based on the given criteria
func (s *fileService) sortEntries(entries []fs.DirEntry, sortBy, sortDir string) {
	sort.Slice(entries, func(i, j int) bool {
		var less bool
		iName := entries[i].Name()
		jName := entries[j].Name()
		iHidden := strings.HasPrefix(iName, ".")
		jHidden := strings.HasPrefix(jName, ".")

		if iHidden != jHidden {
			return !iHidden
		}

		switch sortBy {
		case "name":
			less = strings.ToLower(iName) < strings.ToLower(jName)
		case "size":
			iInfo, _ := entries[i].Info()
			jInfo, _ := entries[j].Info()
			iSize := int64(0)
			jSize := int64(0)
			if iInfo != nil {
				iSize = iInfo.Size()
			}
			if jInfo != nil {
				jSize = jInfo.Size()
			}
			less = iSize < jSize
		case "modTime":
			iInfo, _ := entries[i].Info()
			jInfo, _ := entries[j].Info()
			if iInfo != nil && jInfo != nil {
				less = iInfo.ModTime().Before(jInfo.ModTime())
			}
		case "type":
			// Directories first, then by name
			iDir := entries[i].IsDir()
			jDir := entries[j].IsDir()
			if iDir != jDir {
				less = iDir
			} else {
				less = strings.ToLower(iName) < strings.ToLower(jName)
			}
		default:
			less = strings.ToLower(iName) < strings.ToLower(jName)
		}

		if sortDir == "desc" {
			return !less
		}
		return less
	})
}
