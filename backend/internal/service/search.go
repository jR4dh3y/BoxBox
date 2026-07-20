// Package service provides business logic for the file manager.
package service

import (
	"context"
	"errors"
	"io/fs"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
)

// Search service errors
var (
	ErrEmptyQuery  = errors.New("search query cannot be empty")
	errSearchLimit = errors.New("search result limit reached")
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
	walker      Walker
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
		walker:      NewWalker(fsys),
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

	// Perform a bounded recursive search. This prevents one request from
	// retaining an arbitrarily large result set in memory.
	queryLower := strings.ToLower(query)
	results := make([]model.FileInfo, 0, 64)
	const maxSearchResults = 1000
	err = s.walker.Walk(ctx, fsPath, WalkOptions{
		IncludeHidden:  true,
		SkipUnreadable: true,
	}, func(entry WalkEntry) error {
		if len(results) >= maxSearchResults {
			return errSearchLimit
		}
		if !strings.Contains(strings.ToLower(entry.DirEntry.Name()), queryLower) {
			return nil
		}
		info, err := entry.DirEntry.Info()
		if err != nil {
			return nil
		}
		virtualPath := filepath.ToSlash(filepath.Join(path, entry.RelativePath))
		results = append(results, fileutil.ToFileInfo(info.Name(), virtualPath, info))
		return nil
	})
	if err != nil && !errors.Is(err, errSearchLimit) {
		return nil, err
	}

	return results, nil
}
