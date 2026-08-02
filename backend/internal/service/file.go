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

	"github.com/google/uuid"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
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
	ErrDirectoryTooLarge  = errors.New("directory exceeds entry limit")
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

// DirectoryReader contains read-only directory operations.
type DirectoryReader interface {
	// List returns a paginated list of files in a directory
	List(ctx context.Context, path string, opts model.ListOptions) (*model.FileList, error)
	// GetInfo returns metadata for a file or directory
	GetInfo(ctx context.Context, path string) (*model.FileInfo, error)
	// ListMountPoints returns all configured mount points
	ListMountPoints() []model.MountPoint
	// GetDriveStats returns disk usage statistics for all mount points
	GetDriveStats(ctx context.Context) (*model.DriveStatsResponse, error)
	// ResolvePath resolves a virtual path to a filesystem path
	ResolvePath(path string) (*model.MountPoint, string, error)
}

// FileCommander contains mutating filesystem operations.
type FileCommander interface {
	ResolvePath(path string) (*model.MountPoint, string, error)
	WriteFile(ctx context.Context, path string, content []byte) error
	CreateDir(ctx context.Context, path string) error
	Rename(ctx context.Context, oldPath, newPath string) error
	Delete(ctx context.Context, path string) error
	CreateFile(ctx context.Context, path string) (WriteFile, error)
}

// FileStreamer contains streaming-oriented filesystem operations.
type FileStreamer interface {
	OpenFile(ctx context.Context, path string) (File, *model.FileInfo, error)
	ResolvePath(path string) (*model.MountPoint, string, error)
	// GetFilesystem returns the underlying filesystem for advanced operations
	GetFilesystem() filesystem.FS
}

type FileManager interface {
	DirectoryReader
	FileCommander
}

// FileService is the composed compatibility interface used during wiring.
type FileService interface {
	FileManager
	FileStreamer
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
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, ErrMountPointNotFound
		}
		return nil, err
	}
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
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

	// Read directory entries. Afero's entries already carry metadata; keep a
	// local metadata cache so non-name sorts never stat the same entry repeatedly.
	const maxDirectoryEntries = 10_000
	entries, truncated, err := s.fs.ReadDirLimit(fsPath, maxDirectoryEntries)
	if err != nil {
		return nil, err
	}
	if truncated {
		return nil, ErrDirectoryTooLarge
	}

	// Apply filter
	var filtered []fs.DirEntry
	filterLower := strings.ToLower(opts.Filter)
	for _, entry := range entries {
		if !opts.IncludeHidden && strings.HasPrefix(entry.Name(), ".") {
			continue
		}

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
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, ErrMountPointNotFound
		}
		return nil, err
	}
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
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
	fsPath, err = resolveWritablePathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		return err
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
	oldFsPath, err = resolveExistingPathWithinMount(s.fs, oldMount, oldFsPath)
	if err != nil {
		return err
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
	newFsPath, err = resolveWritablePathWithinMount(s.fs, newMount, newFsPath)
	if err != nil {
		return err
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
	fsPath, err = resolveWritablePathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		return err
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
	mount, fsPath, err := s.ResolvePath(path)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) {
			return nil, nil, ErrMountPointNotFound
		}
		return nil, nil, err
	}
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
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
	if err := verifyOpenFileWithinMount(s.fs, mount, file); err != nil {
		_ = file.Close()
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
	fsPath, err = resolveWritablePathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		return nil, err
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
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		return err
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

	temporary := fsPath + ".saving." + uuid.NewString()
	file, err := s.fs.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, info.Mode().Perm())
	if err != nil {
		return err
	}
	complete := false
	defer func() {
		_ = file.Close()
		if !complete {
			_ = s.fs.Remove(temporary)
		}
	}()
	if _, err := file.Write(content); err != nil {
		return err
	}
	if err := file.Close(); err != nil {
		return err
	}
	if err := s.fs.Rename(temporary, fsPath); err != nil {
		return err
	}
	complete = true
	return nil
}

// GetFilesystem returns the underlying filesystem for advanced operations
func (s *fileService) GetFilesystem() filesystem.FS {
	return s.fs
}

// sortEntries sorts directory entries based on the given criteria
func (s *fileService) sortEntries(entries []fs.DirEntry, sortBy, sortDir string) {
	infos := make(map[string]fs.FileInfo, len(entries))
	if sortBy == "size" || sortBy == "modTime" {
		for _, entry := range entries {
			if info, err := entry.Info(); err == nil {
				infos[entry.Name()] = info
			}
		}
	}
	lessFor := func(entry fs.DirEntry) fs.FileInfo {
		if info, ok := infos[entry.Name()]; ok {
			return info
		}
		info, _ := entry.Info()
		return info
	}
	sort.Slice(entries, func(i, j int) bool {
		iName := entries[i].Name()
		jName := entries[j].Name()
		iHidden := strings.HasPrefix(iName, ".")
		jHidden := strings.HasPrefix(jName, ".")

		if iHidden != jHidden {
			return !iHidden
		}

		comparison := 0
		switch sortBy {
		case "name":
			comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
		case "size":
			iInfo := lessFor(entries[i])
			jInfo := lessFor(entries[j])
			iSize := int64(0)
			jSize := int64(0)
			if iInfo != nil {
				iSize = iInfo.Size()
			}
			if jInfo != nil {
				jSize = jInfo.Size()
			}
			comparison = compareInt64(iSize, jSize)
		case "modTime":
			iInfo := lessFor(entries[i])
			jInfo := lessFor(entries[j])
			if iInfo != nil && jInfo != nil {
				comparison = iInfo.ModTime().Compare(jInfo.ModTime())
			}
		case "type":
			// Directories first, then by name
			iDir := entries[i].IsDir()
			jDir := entries[j].IsDir()
			if iDir != jDir {
				return iDir
			} else {
				comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
			}
		default:
			comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
		}
		if comparison == 0 {
			comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
		}

		if sortDir == "desc" {
			return comparison > 0
		}
		return comparison < 0
	})
}

func compareInt64(left, right int64) int {
	switch {
	case left < right:
		return -1
	case left > right:
		return 1
	default:
		return 0
	}
}
