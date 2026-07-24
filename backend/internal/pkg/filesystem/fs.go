// Package filesystem provides a filesystem abstraction layer wrapping afero.
// This allows for easy testing with in-memory filesystems and consistent
// filesystem operations across the application.
package filesystem

import (
	"io/fs"
	"os"

	"github.com/spf13/afero"
)

// FS provides an abstraction over filesystem operations.
// It wraps afero.Fs to provide a consistent interface for file operations.
type FS interface {
	// ReadDir reads the directory named by dirname and returns a list of directory entries.
	ReadDir(name string) ([]fs.DirEntry, error)

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

	// MkdirAll creates a directory named path, along with any necessary parents.
	MkdirAll(path string, perm os.FileMode) error

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

// Stat returns a FileInfo describing the named file.
func (a *AferoFS) Stat(name string) (fs.FileInfo, error) {
	return a.fs.Stat(name)
}

// Open opens the named file for reading.
func (a *AferoFS) Open(name string) (afero.File, error) {
	return a.fs.Open(name)
}

// Create creates or truncates the named file.
func (a *AferoFS) Create(name string) (afero.File, error) {
	return a.fs.Create(name)
}

// Remove removes the named file or empty directory.
func (a *AferoFS) Remove(name string) error {
	return a.fs.Remove(name)
}

// RemoveAll removes path and any children it contains.
func (a *AferoFS) RemoveAll(path string) error {
	return a.fs.RemoveAll(path)
}

// Rename renames (moves) oldpath to newpath.
func (a *AferoFS) Rename(oldpath, newpath string) error {
	return a.fs.Rename(oldpath, newpath)
}

// MkdirAll creates a directory named path, along with any necessary parents.
func (a *AferoFS) MkdirAll(path string, perm os.FileMode) error {
	return a.fs.MkdirAll(path, perm)
}

// Exists checks if a file or directory exists at the given path.
func (a *AferoFS) Exists(path string) (bool, error) {
	return afero.Exists(a.fs, path)
}

// IsDir checks if the path is a directory.
func (a *AferoFS) IsDir(path string) (bool, error) {
	return afero.IsDir(a.fs, path)
}

// OpenFile opens a file using the given flags and permissions.
func (a *AferoFS) OpenFile(name string, flag int, perm os.FileMode) (afero.File, error) {
	return a.fs.OpenFile(name, flag, perm)
}

// WriteFile writes data to a file, creating it if necessary.
func (a *AferoFS) WriteFile(name string, data []byte, perm os.FileMode) error {
	return afero.WriteFile(a.fs, name, data, perm)
}

// ReadFile reads the entire contents of a file.
func (a *AferoFS) ReadFile(name string) ([]byte, error) {
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
