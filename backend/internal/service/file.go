// Package service provides business logic for the file manager.
package service

import (
	"context"
	"errors"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"uuid"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
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

const defaultDirectoryEntryLimit = 10_000

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
	// List returns all files in a directory, subject to the server entry limit.
	List(ctx context.Context, path string) (*model.FileList, error)
	// GetInfo returns metadata for a file or directory
	GetInfo(ctx context.Context, path string) (*model.FileInfo, error)
	// EntryInfo is GetInfo without following a final symlink
	EntryInfo(ctx context.Context, path string) (*model.FileInfo, error)
	// ListMountPoints returns all configured mount points
	ListMountPoints() []model.MountPoint
	// GetDriveStats returns disk usage statistics for all mount points
	GetDriveStats(ctx context.Context) (*model.DriveStatsResponse, error)
}

// FileCommander contains mutating filesystem operations.
type FileCommander interface {
	WriteFile(ctx context.Context, path string, content []byte) error
	CreateDir(ctx context.Context, path string) error
	Rename(ctx context.Context, oldPath, newPath string) error
	Delete(ctx context.Context, path string) error
	CreateFile(ctx context.Context, path string) (WriteFile, error)
}

// FileStreamer contains streaming-oriented filesystem operations.
type FileStreamer interface {
	OpenFile(ctx context.Context, path string) (File, *model.FileInfo, error)
	PrepareDirectoryArchive(ctx context.Context, path string) (*DirectoryArchive, error)
	// ResolveForWrite confines a virtual path to a writable mount; the path may not exist yet
	ResolveForWrite(path string) (*model.MountPoint, string, error)
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
	fs                  filesystem.FS
	mounts              *mounts
	directoryEntryLimit int
}

// FileServiceConfig holds configuration for the file service
type FileServiceConfig struct {
	MountPoints []model.MountPoint
	// DirectoryEntryLimit uses the default 10,000-entry cap when zero; negative disables the cap.
	DirectoryEntryLimit int
}

// NewFileService creates a new file service
func NewFileService(fsys filesystem.FS, cfg FileServiceConfig) FileService {
	directoryEntryLimit := cfg.DirectoryEntryLimit
	if directoryEntryLimit == 0 {
		directoryEntryLimit = defaultDirectoryEntryLimit
	}

	return &fileService{
		fs:                  fsys,
		mounts:              newMounts(fsys, fixedMounts(cfg.MountPoints)),
		directoryEntryLimit: directoryEntryLimit,
	}
}

// ListMountPoints returns all configured mount points
func (s *fileService) ListMountPoints() []model.MountPoint {
	return s.mounts.list()
}

// GetDriveStats returns disk usage statistics for all mount points
// Mount points with auto_discover enabled are expanded to their discovered sub-mounts
func (s *fileService) GetDriveStats(ctx context.Context) (*model.DriveStatsResponse, error) {
	// Expand auto-discover mount points
	effectiveMounts := DiscoverMountPoints(s.fs, s.mounts.list())
	drives := make([]model.DriveStats, 0, len(effectiveMounts))

	for _, mount := range effectiveMounts {
		// Places are folder shortcuts, not storage; they are not drive cards.
		if mount.Kind == model.MountKindPlace {
			continue
		}
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

// ResolveForWrite confines a virtual path to a writable mount; the path may not exist yet
func (s *fileService) ResolveForWrite(path string) (*model.MountPoint, string, error) {
	return s.mounts.resolve(path, writeMaybeMissing)
}

// List returns every entry of a directory, hidden ones included, in directory order, subject to
// the server entry limit. The client sorts and filters, so one read serves every view of the folder.
func (s *fileService) List(ctx context.Context, path string) (*model.FileList, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	// Resolve the path to filesystem path
	_, fsPath, err := s.mounts.resolve(path, readExisting)
	if err != nil {
		return nil, err
	}

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
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	const maxDirectoryEntries = 10_000
	entries, truncated, err := s.fs.ReadDirFileInfoLimit(fsPath, maxDirectoryEntries)
	if err != nil {
		return nil, err
	}
	if truncated {
		return nil, ErrDirectoryTooLarge
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	items := make([]model.FileInfo, 0, len(entries))
	modeStrings := make(map[fs.FileMode]string)
	parentPath := filepath.Clean(path)
	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		mode := entry.Mode()
		permissions, ok := modeStrings[mode]
		if !ok {
			permissions = mode.String()
			modeStrings[mode] = permissions
		}
		items = append(items, fileutil.ToFileInfoWithPermissions(
			entry.Name(),
			joinListingPath(parentPath, entry.Name()),
			entry,
			permissions,
		))
	}

	return &model.FileList{
		Path:  path,
		Items: items,
	}, nil
}

// GetInfo returns metadata for a file or directory
func (s *fileService) GetInfo(ctx context.Context, path string) (*model.FileInfo, error) {
	// Resolve the path to filesystem path
	_, fsPath, err := s.mounts.resolve(path, readExisting)
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

// EntryInfo describes the entry itself, so a symlink to a folder is not a folder.
func (s *fileService) EntryInfo(ctx context.Context, path string) (*model.FileInfo, error) {
	_, fsPath, err := s.mounts.resolve(path, readEntry)
	if err != nil {
		return nil, err
	}
	info, err := s.fs.Lstat(fsPath)
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
	_, fsPath, err := s.mounts.resolve(path, writeMaybeMissing)
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
	// Resolve old path; a symlink is renamed itself, not its target
	_, oldFsPath, err := s.mounts.resolve(oldPath, writeEntry)
	if err != nil {
		return err
	}

	if err := s.mounts.requireWritablePath(oldFsPath, true); err != nil {
		return err
	}

	// Resolve new path
	_, newFsPath, err := s.mounts.resolve(newPath, writeMaybeMissing)
	if err != nil {
		return err
	}

	if err := s.mounts.requireWritablePath(newFsPath, true); err != nil {
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
	// Resolve the path to filesystem path; a symlink is removed itself, not its target
	_, fsPath, err := s.mounts.resolve(path, writeEntry)
	if err != nil {
		return err
	}

	if err := s.mounts.requireWritablePath(fsPath, true); err != nil {
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
	mount, fsPath, err := s.mounts.resolve(path, readExisting)
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
	if !info.Mode().IsRegular() {
		return nil, nil, ErrNotFile
	}
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

func (s *fileService) PrepareDirectoryArchive(ctx context.Context, path string) (*DirectoryArchive, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	mount, fsPath, err := s.mounts.resolve(path, readExisting)
	if err != nil {
		return nil, err
	}
	return prepareDirectoryArchive(ctx, s.fs, fsPath, mount.Path)
}

// CreateFile creates a new file for writing using the filesystem abstraction
func (s *fileService) CreateFile(ctx context.Context, path string) (WriteFile, error) {
	// Resolve the path to filesystem path
	_, fsPath, err := s.mounts.resolve(path, writeMaybeMissing)
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
	_, fsPath, err := s.mounts.resolve(path, writeExisting)
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

	temporary := fsPath + ".saving." + uuid.New().String()
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

func joinListingPath(parent, name string) string {
	if parent == "." {
		return name
	}
	if parent[len(parent)-1] == filepath.Separator {
		return parent + name
	}
	return parent + string(filepath.Separator) + name
}
