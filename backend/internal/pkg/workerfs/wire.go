//go:build linux

// Package workerfs serves filesystem.FS from a worker process to the API. The
// worker runs as one NAS identity; the API sees ordinary paths, errors and
// open files, while the kernel checks every access against that identity.
package workerfs

import (
	"errors"
	"io/fs"
	"os"
	"syscall"
	"time"

	"github.com/spf13/afero"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type operation string

const (
	opReadDir              operation = "readDir"
	opReadDirLimit         operation = "readDirLimit"
	opReadDirFileInfoLimit operation = "readDirFileInfoLimit"
	opEvalSymlinks         operation = "evalSymlinks"
	opReadlink             operation = "readlink"
	opStat                 operation = "stat"
	opLstat                operation = "lstat"
	opOpenFile             operation = "openFile"
	opRemove               operation = "remove"
	opRemoveAll            operation = "removeAll"
	opRename               operation = "rename"
	opRenameNoReplace      operation = "renameNoReplace"
	opMkdirAllWithin       operation = "mkdirAllWithin"
	opMkdirAll             operation = "mkdirAll"
	opMkdir                operation = "mkdir"
	opChmod                operation = "chmod"
	opExists               operation = "exists"
	opIsDir                operation = "isDir"
	opWriteFile            operation = "writeFile"
	opReadFile             operation = "readFile"
)

type request struct {
	Op    operation   `json:"op"`
	Path  string      `json:"path"`
	Path2 string      `json:"path2,omitempty"`
	Flag  int         `json:"flag,omitempty"`
	Perm  fs.FileMode `json:"perm,omitempty"`
	Limit int         `json:"limit,omitempty"`
	Data  []byte      `json:"data,omitempty"`
}

type response struct {
	Err       *wireError `json:"err,omitempty"`
	Info      *wireInfo  `json:"info,omitempty"`
	Infos     []wireInfo `json:"infos,omitempty"`
	Text      string     `json:"text,omitempty"`
	OK        bool       `json:"ok,omitempty"`
	Truncated bool       `json:"truncated,omitempty"`
	Data      []byte     `json:"data,omitempty"`
}

// wireInfo carries what the services read from fs.FileInfo. Sys() is not
// carried; nothing outside the desktop app reads it.
type wireInfo struct {
	Name    string      `json:"name"`
	Size    int64       `json:"size"`
	Mode    fs.FileMode `json:"mode"`
	ModTime int64       `json:"modTime"`
}

func toWireInfo(info fs.FileInfo) wireInfo {
	return wireInfo{Name: info.Name(), Size: info.Size(), Mode: info.Mode(), ModTime: info.ModTime().UnixNano()}
}

type fileInfo struct{ wire wireInfo }

func (i fileInfo) Name() string       { return i.wire.Name }
func (i fileInfo) Size() int64        { return i.wire.Size }
func (i fileInfo) Mode() fs.FileMode  { return i.wire.Mode }
func (i fileInfo) ModTime() time.Time { return time.Unix(0, i.wire.ModTime) }
func (i fileInfo) IsDir() bool        { return i.wire.Mode.IsDir() }
func (i fileInfo) Sys() any           { return nil }

type dirEntry struct{ info fileInfo }

func (d dirEntry) Name() string               { return d.info.Name() }
func (d dirEntry) IsDir() bool                { return d.info.IsDir() }
func (d dirEntry) Type() fs.FileMode          { return d.info.Mode().Type() }
func (d dirEntry) Info() (fs.FileInfo, error) { return d.info, nil }

type errorWrapper string

const (
	wrapNone    errorWrapper = ""
	wrapPath    errorWrapper = "path"
	wrapLink    errorWrapper = "link"
	wrapSyscall errorWrapper = "syscall"
)

// wireError keeps the errors.Is behaviour the services depend on: errno
// values (EXDEV, ENOTDIR, ...) and the fs sentinels.
type wireError struct {
	Wrapper  errorWrapper `json:"wrapper,omitempty"`
	Op       string       `json:"op,omitempty"`
	Path     string       `json:"path,omitempty"`
	Path2    string       `json:"path2,omitempty"`
	Errno    int          `json:"errno,omitempty"`
	Sentinel string       `json:"sentinel,omitempty"`
	Message  string       `json:"message"`
}

var sentinels = map[string]error{
	"not-exist":    fs.ErrNotExist,
	"exist":        fs.ErrExist,
	"permission":   fs.ErrPermission,
	"invalid":      fs.ErrInvalid,
	"invalid-path": filesystem.ErrInvalidPath,
	"no-readlink":  afero.ErrNoReadlink,
}

func encodeError(err error) *wireError {
	if err == nil {
		return nil
	}
	wire := &wireError{}
	inner := err
	var pathErr *fs.PathError
	var linkErr *os.LinkError
	var syscallErr *os.SyscallError
	switch {
	case errors.As(err, &pathErr):
		wire.Wrapper, wire.Op, wire.Path, inner = wrapPath, pathErr.Op, pathErr.Path, pathErr.Err
	case errors.As(err, &linkErr):
		wire.Wrapper, wire.Op, wire.Path, wire.Path2, inner = wrapLink, linkErr.Op, linkErr.Old, linkErr.New, linkErr.Err
	case errors.As(err, &syscallErr):
		wire.Wrapper, wire.Op, inner = wrapSyscall, syscallErr.Syscall, syscallErr.Err
	}
	wire.Message = inner.Error()
	var errno syscall.Errno
	if errors.As(inner, &errno) {
		wire.Errno = int(errno)
		return wire
	}
	for name, sentinel := range sentinels {
		if errors.Is(inner, sentinel) {
			wire.Sentinel = name
			return wire
		}
	}
	return wire
}

func (w *wireError) decode() error {
	var inner error
	switch {
	case w.Errno != 0:
		inner = syscall.Errno(w.Errno)
	case sentinels[w.Sentinel] != nil:
		inner = sentinels[w.Sentinel]
	default:
		inner = errors.New(w.Message)
	}
	switch w.Wrapper {
	case wrapPath:
		return &fs.PathError{Op: w.Op, Path: w.Path, Err: inner}
	case wrapLink:
		return &os.LinkError{Op: w.Op, Old: w.Path, New: w.Path2, Err: inner}
	case wrapSyscall:
		return os.NewSyscallError(w.Op, inner)
	}
	return inner
}
