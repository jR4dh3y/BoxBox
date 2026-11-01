package service

import (
	"context"
	"errors"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const (
	DesktopClipboardCopy DesktopClipboardOperation = "copy"
	DesktopClipboardCut  DesktopClipboardOperation = "cut"

	clipboardMaxItems       = 256
	clipboardMaxPayloadSize = 1 << 20
	gnomeCopiedFilesMIME    = "x-special/gnome-copied-files"
	uriListMIME             = "text/uri-list"
	kdeCutSelectionMIME     = "application/x-kde-cutselection"
)

var (
	ErrDesktopClipboardUnavailable = errors.New("desktop clipboard integration is unavailable")
	ErrDesktopClipboardChanged     = errors.New("desktop clipboard changed before the cut could be cleared")
	errUnsupportedClipboardData    = errors.New("clipboard does not contain supported local files")
)

type DesktopClipboardOperation string

type DesktopClipboardItem struct {
	Path  string `json:"path"`
	Name  string `json:"name"`
	IsDir bool   `json:"isDir"`
}

type DesktopClipboardContents struct {
	Operation DesktopClipboardOperation `json:"operation"`
	Items     []DesktopClipboardItem    `json:"items"`
}

type DesktopClipboard interface {
	Get(context.Context) (*DesktopClipboardContents, error)
	Set(context.Context, DesktopClipboardOperation, []string) error
	ClearCut(context.Context, []string) error
}

type clipboardAdapter interface {
	Read(context.Context) (map[string][]byte, error)
	Write(context.Context, DesktopClipboardOperation, map[string][]byte) error
	Clear(context.Context) error
}

type desktopClipboardService struct {
	fs      filesystem.FS
	mounts  *mounts
	adapter clipboardAdapter
	mu      sync.Mutex
}

func NewDesktopClipboard(fsys filesystem.FS, mountPoints []model.MountPoint) DesktopClipboard {
	return &desktopClipboardService{
		fs:      fsys,
		mounts:  newMounts(fsys, fixedMounts(mountPoints)),
		adapter: newLinuxClipboardAdapter(),
	}
}

func (s *desktopClipboardService) Get(ctx context.Context) (*DesktopClipboardContents, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.get(ctx)
}

func (s *desktopClipboardService) get(ctx context.Context) (*DesktopClipboardContents, error) {
	formats, err := s.adapter.Read(ctx)
	if err != nil {
		return nil, err
	}
	operation, uris, err := decodeClipboardFormats(formats)
	if errors.Is(err, errUnsupportedClipboardData) {
		return nil, nil
	}
	if err != nil {
		return nil, nil
	}

	items := make([]DesktopClipboardItem, 0, len(uris))
	for _, uri := range uris {
		item, err := s.itemFromFileURI(uri, false)
		if err != nil {
			return nil, nil
		}
		items = append(items, item)
	}
	if len(items) == 0 {
		return nil, nil
	}
	return &DesktopClipboardContents{Operation: operation, Items: items}, nil
}

func (s *desktopClipboardService) Set(ctx context.Context, operation DesktopClipboardOperation, paths []string) error {
	if operation != DesktopClipboardCopy && operation != DesktopClipboardCut {
		return ErrInvalidOperation
	}
	if len(paths) == 0 || len(paths) > clipboardMaxItems {
		return ErrInvalidOperation
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	files := make([]clipboardFile, 0, len(paths))
	seen := make(map[string]struct{}, len(paths))
	for _, path := range paths {
		// Clipboard JSON contains literal names, but mounts.resolve decodes escapes.
		if strings.Contains(path, "%") {
			return ErrInvalidOperation
		}
		if _, exists := seen[path]; exists {
			return ErrInvalidOperation
		}
		seen[path] = struct{}{}

		_, fsPath, err := s.mounts.resolve(path, readExisting)
		if err != nil {
			return err
		}
		info, err := s.fs.Stat(fsPath)
		if err != nil {
			return ErrPathNotFound
		}
		if !info.IsDir() && !info.Mode().IsRegular() {
			return ErrInvalidOperation
		}
		absolute, err := filepath.Abs(fsPath)
		if err != nil {
			return ErrInvalidOperation
		}
		files = append(files, clipboardFile{
			path:  path,
			name:  info.Name(),
			isDir: info.IsDir(),
			uri:   (&url.URL{Scheme: "file", Path: absolute}).String(),
		})
	}

	formats, err := encodeClipboardFormats(operation, files)
	if err != nil {
		return err
	}
	return s.adapter.Write(ctx, operation, formats)
}

func (s *desktopClipboardService) ClearCut(ctx context.Context, expectedPaths []string) error {
	if len(expectedPaths) == 0 || len(expectedPaths) > clipboardMaxItems {
		return ErrInvalidOperation
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	formats, err := s.adapter.Read(ctx)
	if err != nil {
		return err
	}
	operation, uris, err := decodeClipboardFormats(formats)
	if err != nil || operation != DesktopClipboardCut {
		return ErrDesktopClipboardChanged
	}
	paths := make([]string, 0, len(uris))
	for _, uri := range uris {
		item, err := s.itemFromFileURI(uri, true)
		if err != nil {
			return ErrDesktopClipboardChanged
		}
		paths = append(paths, item.Path)
	}
	if !equalStringSlices(paths, expectedPaths) {
		return ErrDesktopClipboardChanged
	}
	return s.adapter.Clear(ctx)
}

type clipboardFile struct {
	path  string
	name  string
	isDir bool
	uri   string
}

func encodeClipboardFormats(operation DesktopClipboardOperation, files []clipboardFile) (map[string][]byte, error) {
	if operation != DesktopClipboardCopy && operation != DesktopClipboardCut {
		return nil, ErrInvalidOperation
	}
	if len(files) == 0 || len(files) > clipboardMaxItems {
		return nil, ErrInvalidOperation
	}

	verb := string(operation)
	uriLines := make([]string, 0, len(files))
	for _, file := range files {
		if file.uri == "" || len(file.uri) > clipboardMaxPayloadSize {
			return nil, ErrInvalidOperation
		}
		uriLines = append(uriLines, file.uri)
	}
	gnome := verb + "\n" + strings.Join(uriLines, "\n") + "\n"
	uriList := strings.Join(uriLines, "\r\n") + "\r\n"
	kdeCut := "0"
	if operation == DesktopClipboardCut {
		kdeCut = "1"
	}
	if len(gnome)+len(uriList)+len(kdeCut) > clipboardMaxPayloadSize {
		return nil, ErrInvalidOperation
	}
	return map[string][]byte{
		gnomeCopiedFilesMIME: []byte(gnome),
		uriListMIME:          []byte(uriList),
		kdeCutSelectionMIME:  []byte(kdeCut),
	}, nil
}

func decodeClipboardFormats(formats map[string][]byte) (DesktopClipboardOperation, []string, error) {
	if data, ok := formats[gnomeCopiedFilesMIME]; ok {
		lines := splitClipboardLines(data)
		if len(lines) < 2 {
			return "", nil, errUnsupportedClipboardData
		}
		operation := DesktopClipboardOperation(lines[0])
		if operation != DesktopClipboardCopy && operation != DesktopClipboardCut {
			return "", nil, errUnsupportedClipboardData
		}
		uris, err := parseClipboardURIList([]byte(strings.Join(lines[1:], "\n")))
		return operation, uris, err
	}

	data, ok := formats[uriListMIME]
	if !ok {
		return "", nil, errUnsupportedClipboardData
	}
	uris, err := parseClipboardURIList(data)
	if err != nil {
		return "", nil, err
	}
	operation := DesktopClipboardCopy
	if marker, ok := formats[kdeCutSelectionMIME]; ok {
		switch strings.TrimSpace(string(marker)) {
		case "0":
		case "1":
			operation = DesktopClipboardCut
		default:
			return "", nil, errUnsupportedClipboardData
		}
	}
	return operation, uris, nil
}

func splitClipboardLines(data []byte) []string {
	lines := strings.Split(strings.ReplaceAll(string(data), "\r\n", "\n"), "\n")
	for len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	return lines
}

func parseClipboardURIList(data []byte) ([]string, error) {
	if len(data) > clipboardMaxPayloadSize {
		return nil, errUnsupportedClipboardData
	}
	uris := make([]string, 0)
	for _, line := range splitClipboardLines(data) {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if len(uris) == clipboardMaxItems {
			return nil, errUnsupportedClipboardData
		}
		uris = append(uris, line)
	}
	if len(uris) == 0 {
		return nil, errUnsupportedClipboardData
	}
	return uris, nil
}

func (s *desktopClipboardService) itemFromFileURI(value string, allowMissing bool) (DesktopClipboardItem, error) {
	parsed, err := url.Parse(value)
	if err != nil || parsed.Scheme != "file" || (parsed.Host != "" && !strings.EqualFold(parsed.Host, "localhost")) ||
		parsed.User != nil || parsed.RawQuery != "" || parsed.ForceQuery || parsed.Fragment != "" || !filepath.IsAbs(parsed.Path) {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}
	if strings.ContainsRune(parsed.Path, '\x00') {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}

	absolute, err := filepath.Abs(filepath.Clean(parsed.Path))
	if err != nil {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}
	resolved, err := filepath.EvalSymlinks(absolute)
	if err != nil && allowMissing && os.IsNotExist(err) {
		parent, parentErr := filepath.EvalSymlinks(filepath.Dir(absolute))
		if parentErr == nil {
			resolved = filepath.Join(parent, filepath.Base(absolute))
			err = nil
		}
	}
	if err != nil {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}

	var selectedMount *model.MountPoint
	selectedRelative := ""
	selectedRootLength := -1
	for _, mount := range s.mounts.list() {
		root, err := filepath.Abs(mount.Path)
		if err != nil {
			continue
		}
		root, err = filepath.EvalSymlinks(root)
		if err != nil {
			continue
		}
		relative, err := filepath.Rel(root, resolved)
		if err != nil || filepath.IsAbs(relative) || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			continue
		}
		if len(root) > selectedRootLength {
			mountCopy := mount
			selectedMount = &mountCopy
			selectedRelative = relative
			selectedRootLength = len(root)
		}
	}
	if selectedMount == nil {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}

	virtualPath := selectedMount.Name
	name := selectedMount.Name
	if selectedRelative != "." {
		virtualPath += "/" + filepath.ToSlash(selectedRelative)
		name = filepath.Base(selectedRelative)
	}
	_, fsPath, err := s.mounts.resolve(virtualPath, readExisting)
	if err != nil {
		if !allowMissing {
			return DesktopClipboardItem{}, errUnsupportedClipboardData
		}
		// A successful move removes the original path before its cut clipboard is cleared.
		_, fsPath, err = s.mounts.resolve(virtualPath, writeMaybeMissing)
		if err != nil {
			return DesktopClipboardItem{}, errUnsupportedClipboardData
		}
	}
	// The virtual-path resolver decodes percent escapes. Never substitute a sibling
	// when a native filename contains an escape-looking literal sequence.
	if fsPath != resolved {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}
	item := DesktopClipboardItem{Path: virtualPath, Name: name}
	if info, statErr := s.fs.Stat(fsPath); statErr == nil {
		item.IsDir = info.IsDir()
	} else if !allowMissing {
		return DesktopClipboardItem{}, errUnsupportedClipboardData
	}
	return item, nil
}

func equalStringSlices(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
