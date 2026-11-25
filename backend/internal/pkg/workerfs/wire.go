//go:build linux

// Package workerfs serves filesystem.FS from a worker process to the API. The
// worker runs as one NAS identity; the API sees ordinary paths, errors and
// open files, while the kernel checks every access against that identity.
package workerfs

import (
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"syscall"
	"time"

	"github.com/spf13/afero"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// rawString is a name or path sent as bytes. Linux names need not be UTF-8,
// and a JSON string would replace such bytes, so the API would then ask for
// a path that does not exist.
type rawString string

func (s rawString) MarshalJSON() ([]byte, error) { return json.Marshal([]byte(s)) }

func (s *rawString) UnmarshalJSON(data []byte) error {
	var raw []byte
	if err := json.Unmarshal(data, &raw); err != nil {
		return err
	}
	*s = rawString(raw)
	return nil
}

type operation string

const (
	opReadDir              operation = "readDir"
	opReadDirLimit         operation = "readDirLimit"
	opReadDirPreview       operation = "readDirPreview"
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
	Op            operation   `json:"op"`
	Path          rawString   `json:"path"`
	Path2         rawString   `json:"path2,omitempty"`
	Flag          int         `json:"flag,omitempty"`
	Perm          fs.FileMode `json:"perm,omitempty"`
	Limit         int         `json:"limit,omitempty"`
	IncludeHidden bool        `json:"includeHidden,omitempty"`
	Data          []byte      `json:"data,omitempty"`
}

type response struct {
	Err       *wireError `json:"err,omitempty"`
	Info      *wireInfo  `json:"info,omitempty"`
	Infos     []wireInfo `json:"infos,omitempty"`
	Text      rawString  `json:"text,omitempty"`
	OK        bool       `json:"ok,omitempty"`
	Truncated bool       `json:"truncated,omitempty"`
	Data      []byte     `json:"data,omitempty"`
}

// wireInfo carries what the services read from fs.FileInfo. Sys() is not
// carried; nothing outside the desktop app reads it.
type wireInfo struct {
	Name rawString   `json:"name"`
	Size int64       `json:"size"`
	Mode fs.FileMode `json:"mode"`
	// Seconds and nanoseconds: UnixNano only covers the years 1678 to 2262.
	ModSec  int64 `json:"modSec"`
	ModNsec int   `json:"modNsec"`
	// Err is a directory entry's failed Info. Name and Mode's type bits stay
	// valid, as they do for an os.ReadDir entry.
	Err *wireError `json:"err,omitempty"`
}

func toWireInfo(info fs.FileInfo) wireInfo {
	return wireInfo{Name: rawString(info.Name()), Size: info.Size(), Mode: info.Mode(), ModSec: info.ModTime().Unix(), ModNsec: info.ModTime().Nanosecond()}
}

type fileInfo struct{ wire wireInfo }

func (i fileInfo) Name() string       { return string(i.wire.Name) }
func (i fileInfo) Size() int64        { return i.wire.Size }
func (i fileInfo) Mode() fs.FileMode  { return i.wire.Mode }
func (i fileInfo) ModTime() time.Time { return time.Unix(i.wire.ModSec, int64(i.wire.ModNsec)) }
func (i fileInfo) IsDir() bool        { return i.wire.Mode.IsDir() }
func (i fileInfo) Sys() any           { return nil }

type dirEntry struct{ info fileInfo }

func (d dirEntry) Name() string      { return d.info.Name() }
func (d dirEntry) IsDir() bool       { return d.info.IsDir() }
func (d dirEntry) Type() fs.FileMode { return d.info.Mode().Type() }
func (d dirEntry) Info() (fs.FileInfo, error) {
	if d.info.wire.Err != nil {
		return nil, d.info.wire.Err.decode()
	}
	return d.info, nil
}

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
	Path     rawString    `json:"path,omitempty"`
	Path2    rawString    `json:"path2,omitempty"`
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
	"too-large":    fdconn.ErrMessageTooLarge,
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
		wire.Wrapper, wire.Op, wire.Path, inner = wrapPath, pathErr.Op, rawString(pathErr.Path), pathErr.Err
	case errors.As(err, &linkErr):
		wire.Wrapper, wire.Op, wire.Path, wire.Path2, inner = wrapLink, linkErr.Op, rawString(linkErr.Old), rawString(linkErr.New), linkErr.Err
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
		return &fs.PathError{Op: w.Op, Path: string(w.Path), Err: inner}
	case wrapLink:
		return &os.LinkError{Op: w.Op, Old: string(w.Path), New: string(w.Path2), Err: inner}
	case wrapSyscall:
		return os.NewSyscallError(w.Op, inner)
	}
	return inner
}
