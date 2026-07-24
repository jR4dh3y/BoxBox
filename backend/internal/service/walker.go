package service

import (
	"context"
	"io/fs"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// WalkEntry is one item discovered below a traversal root.
type WalkEntry struct {
	Path         string
	RelativePath string
	DirEntry     fs.DirEntry
	Metadata     fs.FileInfo
}

// WalkOptions controls generic filesystem traversal.
type WalkOptions struct {
	IncludeHidden  bool
	SkipUnreadable bool
	MaxEntries     int
	LoadMetadata   bool
}

// Walker provides one cancellation-aware traversal policy for services.
type Walker interface {
	Walk(ctx context.Context, root string, opts WalkOptions, visit func(WalkEntry) error) error
}

type filesystemWalker struct {
	fs filesystem.FS
}

func NewWalker(fsys filesystem.FS) Walker {
	return &filesystemWalker{fs: fsys}
}

func (w *filesystemWalker) Walk(
	ctx context.Context,
	root string,
	opts WalkOptions,
	visit func(WalkEntry) error,
) error {
	visited := 0
	var walkDir func(string, string) error
	walkDir = func(currentPath, relativeBase string) error {
		if err := ctx.Err(); err != nil {
			return err
		}

		entries, err := w.fs.ReadDir(currentPath)
		if err != nil {
			if opts.SkipUnreadable {
				return nil
			}
			return err
		}

		for _, entry := range entries {
			if err := ctx.Err(); err != nil {
				return err
			}
			if !opts.IncludeHidden && strings.HasPrefix(entry.Name(), ".") {
				continue
			}
			if opts.MaxEntries > 0 && visited >= opts.MaxEntries {
				return nil
			}

			relativePath := filepath.Join(relativeBase, entry.Name())
			path := filepath.Join(currentPath, entry.Name())
			var metadata fs.FileInfo
			if opts.LoadMetadata {
				metadata, err = entry.Info()
				if err != nil {
					if opts.SkipUnreadable {
						continue
					}
					return err
				}
			}
			visited++
			if err := visit(WalkEntry{
				Path:         path,
				RelativePath: relativePath,
				DirEntry:     entry,
				Metadata:     metadata,
			}); err != nil {
				return err
			}

			if entry.IsDir() {
				if err := walkDir(path, relativePath); err != nil {
					return err
				}
			}
		}

		return nil
	}

	return walkDir(root, "")
}
