// Package filesystem provides a filesystem abstraction layer wrapping afero.
// This allows for easy testing with in-memory filesystems and consistent
// filesystem operations across the application.
package filesystem

import (
	"errors"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/afero"
)

// FS provides an abstraction over filesystem operations.
// It wraps afero.Fs to provide a consistent interface for file operations.
type FS interface {
	// ReadDir reads the directory named by dirname and returns a list of directory entries.
	ReadDir(name string) ([]fs.DirEntry, error)

	// ReadDirLimit reads at most limit entries and reports whether more exist.
	ReadDirLimit(name string, limit int) ([]fs.DirEntry, bool, error)

	// EvalSymlinks returns the path after evaluating symbolic links.
	EvalSymlinks(path string) (string, error)

	// Stat returns a FileInfo describing the named file.
	Stat(name string) (fs.FileInfo, error)

	// Open opens the named file for reading.
	Open(name string) (afero.File, error)

	// Create creates or truncates the named file.
	Create(name string) (afero.File, error)

	// Remove removes the named file or empty directory.
	Remove(name string) error

	// RemoveAll removes path and any children it contains.
	RemoveAll(path string) error

	// Rename renames (moves) oldpath to newpath.
	Rename(oldpath, newpath string) error

	// RenameNoReplace publishes oldpath at newpath only if newpath does not exist.
	RenameNoReplace(oldpath, newpath string) error

	// MkdirAll creates a directory named path, along with any necessary parents.
	MkdirAll(path string, perm os.FileMode) error

	// Chmod changes the mode bits of the named file or directory.
	Chmod(name string, mode os.FileMode) error

	// Exists checks if a file or directory exists at the given path.
	Exists(path string) (bool, error)

	// IsDir checks if the path is a directory.
	IsDir(path string) (bool, error)

	// OpenFile opens a file using the given flags and permissions.
	OpenFile(name string, flag int, perm os.FileMode) (afero.File, error)

	// WriteFile writes data to a file, creating it if necessary.
	WriteFile(name string, data []byte, perm os.FileMode) error

	// ReadFile reads the entire contents of a file.
	ReadFile(name string) ([]byte, error)
}

// AferoFS implements FS using afero.Fs
type AferoFS struct {
	fs afero.Fs
}

var errInvalidFilesystemPath = errors.New("invalid filesystem path")

func validateFilesystemPath(path string) error {
	if strings.ContainsRune(path, '\x00') || strings.Contains(path, "..") {
		return errInvalidFilesystemPath
	}
	return nil
}

// NewOsFS creates a new AferoFS using the real OS filesystem
func NewOsFS() *AferoFS {
	return &AferoFS{fs: afero.NewOsFs()}
}

// NewMemMapFS creates a new AferoFS using an in-memory filesystem (for testing)
func NewMemMapFS() *AferoFS {
	return &AferoFS{fs: afero.NewMemMapFs()}
}

// ReadDir reads the directory named by dirname and returns a list of directory entries.
func (a *AferoFS) ReadDir(name string) ([]fs.DirEntry, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, err
	}
	name = filepath.Clean("/" + name)
	if _, ok := a.fs.(*afero.OsFs); ok {
		return os.ReadDir(name)
	}
	entries, err := afero.ReadDir(a.fs, name)
	if err != nil {
		return nil, err
	}

	dirEntries := make([]fs.DirEntry, len(entries))
	for i, entry := range entries {
		dirEntries[i] = &dirEntry{info: entry}
	}
	return dirEntries, nil
}

func (a *AferoFS) ReadDirLimit(name string, limit int) ([]fs.DirEntry, bool, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, false, err
	}
	name = filepath.Clean("/" + name)
	if limit < 1 {
		entries, err := a.ReadDir(name)
		return entries, false, err
	}
	if _, ok := a.fs.(*afero.OsFs); ok {
		directory, err := os.Open(name)
		if err != nil {
			return nil, false, err
		}
		defer directory.Close()
		entries, err := directory.ReadDir(limit + 1)
		if err != nil && !errors.Is(err, io.EOF) {
			return nil, false, err
		}
		truncated := len(entries) > limit
		if truncated {
			entries = entries[:limit]
		}
		return entries, truncated, nil
	}

	directory, err := a.fs.Open(name)
	if err != nil {
		return nil, false, err
	}
	defer directory.Close()
	infos, err := directory.Readdir(limit + 1)
	if err != nil && !errors.Is(err, io.EOF) {
		return nil, false, err
	}
	truncated := len(infos) > limit
	if truncated {
		infos = infos[:limit]
	}
	entries := make([]fs.DirEntry, len(infos))
	for index, info := range infos {
		entries[index] = &dirEntry{info: info}
	}
	return entries, truncated, nil
}

func (a *AferoFS) EvalSymlinks(path string) (string, error) {
	if err := validateFilesystemPath(path); err != nil {
		return "", err
	}
	path = filepath.Clean("/" + path)
	if _, ok := a.fs.(*afero.OsFs); ok {
		return filepath.EvalSymlinks(path)
	}
	// MemMapFs does not implement symbolic links.
	return filepath.Clean(path), nil
}

// Stat returns a FileInfo describing the named file.
func (a *AferoFS) Stat(name string) (fs.FileInfo, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, err
	}
	name = filepath.Clean("/" + name)
	return a.fs.Stat(name)
}

// Open opens the named file for reading.
func (a *AferoFS) Open(name string) (afero.File, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, err
	}
	name = filepath.Clean("/" + name)
	return a.fs.Open(name)
}

// Create creates or truncates the named file.
func (a *AferoFS) Create(name string) (afero.File, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, err
	}
	name = filepath.Clean("/" + name)
	return a.fs.Create(name)
}

// Remove removes the named file or empty directory.
func (a *AferoFS) Remove(name string) error {
	if err := validateFilesystemPath(name); err != nil {
		return err
	}
	name = filepath.Clean("/" + name)
	return a.fs.Remove(name)
}

// RemoveAll removes path and any children it contains.
func (a *AferoFS) RemoveAll(path string) error {
	if err := validateFilesystemPath(path); err != nil {
		return err
	}
	path = filepath.Clean("/" + path)
	return a.fs.RemoveAll(path)
}

// Rename renames (moves) oldpath to newpath.
func (a *AferoFS) Rename(oldpath, newpath string) error {
	if err := validateFilesystemPath(oldpath); err != nil {
		return err
	}
	if err := validateFilesystemPath(newpath); err != nil {
		return err
	}
	oldpath = filepath.Clean("/" + oldpath)
	newpath = filepath.Clean("/" + newpath)
	return a.fs.Rename(oldpath, newpath)
}

func (a *AferoFS) RenameNoReplace(oldpath, newpath string) error {
	if err := validateFilesystemPath(oldpath); err != nil {
		return err
	}
	if err := validateFilesystemPath(newpath); err != nil {
		return err
	}
	oldpath = filepath.Clean("/" + oldpath)
	newpath = filepath.Clean("/" + newpath)
	if _, ok := a.fs.(*afero.OsFs); ok {
		return renameNoReplaceWithLink(oldpath, newpath, os.Link)
	}
	if _, err := a.fs.Stat(newpath); err == nil {
		return fs.ErrExist
	} else if !errors.Is(err, fs.ErrNotExist) {
		return err
	}
	return a.fs.Rename(oldpath, newpath)
}

func renameNoReplaceWithLink(oldpath, newpath string, link func(string, string) error) error {
	if err := link(oldpath, newpath); err == nil {
		if err := os.Remove(oldpath); err != nil {
			_ = os.Remove(newpath)
			return err
		}
		return nil
	} else if errors.Is(err, fs.ErrExist) {
		return err
	}

	return copyNoReplace(oldpath, newpath)
}

func copyNoReplace(oldpath, newpath string) error {
	source, err := os.Open(oldpath)
	if err != nil {
		return err
	}
	info, err := source.Stat()
	if err != nil {
		_ = source.Close()
		return err
	}
	destination, err := os.OpenFile(newpath, os.O_WRONLY|os.O_CREATE|os.O_EXCL, info.Mode().Perm())
	if err != nil {
		_ = source.Close()
		return err
	}

	_, copyErr := io.Copy(destination, source)
	sourceCloseErr := source.Close()
	destinationCloseErr := destination.Close()
	if err := errors.Join(copyErr, sourceCloseErr, destinationCloseErr); err != nil {
		_ = os.Remove(newpath)
		return err
	}
	if err := os.Chmod(newpath, info.Mode().Perm()); err != nil {
		_ = os.Remove(newpath)
		return err
	}
	if err := os.Remove(oldpath); err != nil {
		_ = os.Remove(newpath)
		return err
	}
	return nil
}

// MkdirAll creates a directory named path, along with any necessary parents.
func (a *AferoFS) MkdirAll(path string, perm os.FileMode) error {
	if err := validateFilesystemPath(path); err != nil {
		return err
	}
	path = filepath.Clean("/" + path)
	return a.fs.MkdirAll(path, perm)
}

// Chmod changes the mode bits of the named file or directory.
func (a *AferoFS) Chmod(name string, mode os.FileMode) error {
	if err := validateFilesystemPath(name); err != nil {
		return err
	}
	name = filepath.Clean("/" + name)
	return a.fs.Chmod(name, mode)
}

// Exists checks if a file or directory exists at the given path.
func (a *AferoFS) Exists(path string) (bool, error) {
	if err := validateFilesystemPath(path); err != nil {
		return false, err
	}
	path = filepath.Clean("/" + path)
	return afero.Exists(a.fs, path)
}

// IsDir checks if the path is a directory.
func (a *AferoFS) IsDir(path string) (bool, error) {
	if err := validateFilesystemPath(path); err != nil {
		return false, err
	}
	path = filepath.Clean("/" + path)
	return afero.IsDir(a.fs, path)
}

// OpenFile opens a file using the given flags and permissions.
func (a *AferoFS) OpenFile(name string, flag int, perm os.FileMode) (afero.File, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, err
	}
	name = filepath.Clean("/" + name)
	return a.fs.OpenFile(name, flag, perm)
}

// WriteFile writes data to a file, creating it if necessary.
func (a *AferoFS) WriteFile(name string, data []byte, perm os.FileMode) error {
	if err := validateFilesystemPath(name); err != nil {
		return err
	}
	name = filepath.Clean("/" + name)
	return afero.WriteFile(a.fs, name, data, perm)
}

// ReadFile reads the entire contents of a file.
func (a *AferoFS) ReadFile(name string) ([]byte, error) {
	if err := validateFilesystemPath(name); err != nil {
		return nil, err
	}
	name = filepath.Clean("/" + name)
	return afero.ReadFile(a.fs, name)
}

// dirEntry wraps fs.FileInfo to implement fs.DirEntry
type dirEntry struct {
	info fs.FileInfo
}

func (d *dirEntry) Name() string {
	return d.info.Name()
}

func (d *dirEntry) IsDir() bool {
	return d.info.IsDir()
}

func (d *dirEntry) Type() fs.FileMode {
	return d.info.Mode().Type()
}

func (d *dirEntry) Info() (fs.FileInfo, error) {
	return d.info, nil
}
