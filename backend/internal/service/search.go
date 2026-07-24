// Package service provides business logic for the file manager.
package service

import (
	"context"
	"errors"
	"io/fs"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
)

// Search service errors
var (
	ErrEmptyQuery = errors.New("search query cannot be empty")
)

// SearchService defines the search operations service interface
type SearchService interface {
	// Search performs a recursive search within a directory
	Search(ctx context.Context, path, query string) ([]model.FileInfo, error)
}

// searchService implements SearchService
type searchService struct {
	fs          filesystem.FS
	mountPoints []model.MountPoint
}

// SearchServiceConfig holds configuration for the search service
type SearchServiceConfig struct {
	MountPoints []model.MountPoint
}

// NewSearchService creates a new search service
func NewSearchService(fsys filesystem.FS, cfg SearchServiceConfig) SearchService {
	return &searchService{
		fs:          fsys,
		mountPoints: cfg.MountPoints,
	}
}

// Search performs a recursive case-insensitive search within a directory
func (s *searchService) Search(ctx context.Context, path, query string) ([]model.FileInfo, error) {
	// Validate query is not empty
	query = strings.TrimSpace(query)
	if query == "" {
		return nil, ErrEmptyQuery
	}

	// Resolve the path to filesystem path
	_, fsPath, err := validator.ValidatePathAgainstMounts(path, s.mountPoints)
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

	// Perform recursive search
	queryLower := strings.ToLower(query)
	var results []model.FileInfo

	err = s.searchRecursive(ctx, fsPath, path, queryLower, &results)
	if err != nil {
		return nil, err
	}

	return results, nil
}

// searchRecursive performs the recursive directory traversal for search
func (s *searchService) searchRecursive(ctx context.Context, fsPath, virtualPath, queryLower string, results *[]model.FileInfo) error {
	// Check for context cancellation
	select {
	case <-ctx.Done():
		return ctx.Err()
	default:
	}

	// Read directory entries
	entries, err := s.fs.ReadDir(fsPath)
	if err != nil {
		// Skip directories we can't read (permission errors, etc.)
		return nil
	}

	for _, entry := range entries {
		// Check for context cancellation
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		name := entry.Name()
		entryFsPath := filepath.Join(fsPath, name)
		entryVirtualPath := virtualPath + "/" + name
		if virtualPath == "" {
			entryVirtualPath = name
		}

		// Check if name matches query (case-insensitive)
		if strings.Contains(strings.ToLower(name), queryLower) {
			entryInfo, err := entry.Info()
			if err != nil {
				continue // Skip entries we can't stat
			}
			*results = append(*results, fileutil.ToFileInfo(name, entryVirtualPath, entryInfo))
		}

		// Recurse into directories
		if entry.IsDir() {
			if err := s.searchRecursive(ctx, entryFsPath, entryVirtualPath, queryLower, results); err != nil {
				return err
			}
		}
	}

	return nil
}


