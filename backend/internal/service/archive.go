package service

import (
	"archive/zip"
	"context"
	"errors"
	"io"
	"io/fs"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const maxArchiveEntries = 100_000

var archivePathReplacer = strings.NewReplacer("%", "%25", "\\", "%5C", ":", "%3A")

type archiveEntry struct {
	path         string
	relativePath string
	info         fs.FileInfo
}

// DirectoryArchive is a validated, bounded ZIP plan for a directory.
type DirectoryArchive struct {
	Name    string
	fs      filesystem.FS
	root    string
	entries []archiveEntry
}

func prepareDirectoryArchive(ctx context.Context, fsys filesystem.FS, root, boundary string) (*DirectoryArchive, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	root, err := fsys.EvalSymlinks(root)
	if err != nil {
		return nil, err
	}
	boundary, err = fsys.EvalSymlinks(boundary)
	if err != nil {
		return nil, err
	}
	if !pathWithinRoot(boundary, root) {
		return nil, ErrPermissionDenied
	}
	rootInfo, err := fsys.Stat(root)
	if err != nil {
		return nil, err
	}
	if !rootInfo.IsDir() {
		return nil, ErrNotDirectory
	}

	archive := &DirectoryArchive{
		Name: safeArchiveName(rootInfo.Name()),
		fs:   fsys,
		root: root,
	}
	err = NewWalker(fsys).Walk(ctx, root, WalkOptions{
		IncludeHidden: true,
		MaxEntries:    maxArchiveEntries,
		LoadMetadata:  true,
	}, func(entry WalkEntry) error {
		if !pathWithinRoot(root, entry.Path) {
			return ErrPermissionDenied
		}
		resolved, err := fsys.EvalSymlinks(entry.Path)
		if err != nil {
			return err
		}
		if !pathWithinRoot(root, resolved) {
			return ErrPermissionDenied
		}
		if entry.Metadata.IsDir() || entry.Metadata.Mode().IsRegular() {
			archive.entries = append(archive.entries, archiveEntry{
				path:         entry.Path,
				relativePath: entry.RelativePath,
				info:         entry.Metadata,
			})
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Slice(archive.entries, func(i, j int) bool {
		return archive.entries[i].relativePath < archive.entries[j].relativePath
	})
	if err := validateArchiveEntryNames(archive); err != nil {
		return nil, err
	}
	return archive, nil
}

func validateArchiveEntryNames(archive *DirectoryArchive) error {
	if _, ok := archiveCollisionKey(archive.Name); !ok {
		return ErrInvalidOperation
	}
	seen := make(map[string]struct{}, len(archive.entries))
	for _, entry := range archive.entries {
		name, err := archiveEntryName(archive.Name, entry.relativePath, entry.info.IsDir())
		if err != nil {
			return err
		}
		key, ok := archiveCollisionKey(name)
		if !ok {
			return ErrInvalidOperation
		}
		if _, exists := seen[key]; exists {
			return ErrInvalidOperation
		}
		seen[key] = struct{}{}
	}
	return nil
}

func archiveCollisionKey(name string) (string, bool) {
	components := strings.Split(strings.TrimSuffix(name, "/"), "/")
	for index, component := range components {
		component = strings.TrimRight(component, " .")
		if component == "" || component == "." || component == ".." {
			return "", false
		}
		components[index] = strings.ToLower(component)
	}
	return strings.Join(components, "/"), true
}

// WriteTo streams a ZIP after its traversal and path boundaries have been checked.
func (a *DirectoryArchive) WriteTo(ctx context.Context, destination io.Writer) error {
	if a == nil || a.fs == nil || a.root == "" {
		return ErrInvalidOperation
	}
	zipWriter := zip.NewWriter(destination)
	root := a.Name
	rootHeader := &zip.FileHeader{Name: root + "/", Method: zip.Store}
	rootHeader.SetMode(fs.ModeDir | 0o755)
	_, writeErr := zipWriter.CreateHeader(rootHeader)
	if writeErr == nil {
		for _, entry := range a.entries {
			if err := ctx.Err(); err != nil {
				writeErr = err
				break
			}
			name, err := archiveEntryName(root, entry.relativePath, entry.info.IsDir())
			if err != nil {
				writeErr = err
				break
			}
			header := &zip.FileHeader{Name: name, Modified: entry.info.ModTime()}
			if entry.info.IsDir() {
				header.Method = zip.Store
				header.SetMode(fs.ModeDir | entry.info.Mode().Perm())
				if _, writeErr = zipWriter.CreateHeader(header); writeErr != nil {
					break
				}
				continue
			}

			file, err := a.fs.Open(entry.path)
			if err != nil {
				writeErr = err
				break
			}
			if err = verifyOpenFileWithinRoot(a.fs, a.root, file); err != nil {
				_ = file.Close()
				writeErr = err
				break
			}
			fileInfo, err := file.Stat()
			if err != nil || !fileInfo.Mode().IsRegular() {
				_ = file.Close()
				if err != nil {
					writeErr = err
				} else {
					writeErr = ErrNotFile
				}
				break
			}
			header.Method = zip.Deflate
			header.SetMode(fileInfo.Mode())
			fileWriter, err := zipWriter.CreateHeader(header)
			if err == nil {
				_, err = io.Copy(fileWriter, archiveContextReader{ctx: ctx, reader: file})
			}
			closeErr := file.Close()
			if err != nil || closeErr != nil {
				writeErr = errors.Join(err, closeErr)
				break
			}
		}
	}
	return errors.Join(writeErr, zipWriter.Close())
}

type archiveContextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r archiveContextReader) Read(buffer []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.reader.Read(buffer)
}

func archiveEntryName(root, relative string, directory bool) (string, error) {
	relative = sanitizeArchivePath(filepath.ToSlash(relative))
	cleaned := path.Clean(relative)
	if cleaned == "." || cleaned == ".." || strings.HasPrefix(cleaned, "../") || path.IsAbs(cleaned) {
		return "", ErrPermissionDenied
	}
	name := path.Join(root, cleaned)
	if directory {
		name += "/"
	}
	return name, nil
}

func safeArchiveName(name string) string {
	name = strings.Trim(sanitizeArchivePath(name), "/")
	if name == "" || name == "." || name == ".." {
		return "folder"
	}
	return name
}

func sanitizeArchivePath(name string) string {
	return archivePathReplacer.Replace(name)
}
