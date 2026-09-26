package service

import (
	"archive/zip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"io"
	"io/fs"
	"path"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const maxArchiveEntries = 100_000

var archivePathReplacer = strings.NewReplacer("%", "%25", "\\", "%5C", ":", "%3A")

type archiveEntry struct {
	path         string
	relativePath string
	archiveName  string
	info         fs.FileInfo
}

// DirectoryArchive is a validated, bounded ZIP plan for a directory.
type DirectoryArchive struct {
	Name    string
	zipRoot string
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
		Name:    safeArchiveName(rootInfo.Name()),
		zipRoot: sanitizeArchiveComponent(rootInfo.Name()),
		fs:      fsys,
		root:    root,
	}
	if archive.zipRoot == "" {
		archive.zipRoot = "folder"
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
	allocator := newArchiveNameAllocator()
	for i := range archive.entries {
		name, err := allocator.entryPath(archive.zipRoot, archive.entries[i].relativePath, archive.entries[i].info.IsDir())
		if err != nil {
			return nil, err
		}
		archive.entries[i].archiveName = name
	}
	return archive, nil
}

type archiveNameAllocator struct {
	children map[string]map[string]string
	used     map[string]map[string]struct{}
}

func newArchiveNameAllocator() *archiveNameAllocator {
	return &archiveNameAllocator{
		children: make(map[string]map[string]string),
		used:     make(map[string]map[string]struct{}),
	}
}

func (a *archiveNameAllocator) entryPath(root, relative string, directory bool) (string, error) {
	relative = filepath.ToSlash(relative)
	if path.IsAbs(relative) {
		return "", ErrPermissionDenied
	}
	cleaned := path.Clean(relative)
	if cleaned == "." || cleaned == ".." || strings.HasPrefix(cleaned, "../") {
		return "", ErrPermissionDenied
	}
	components := strings.Split(cleaned, "/")
	parent := root
	output := make([]string, len(components))
	for i, component := range components {
		if component == "" || component == "." || component == ".." {
			return "", ErrPermissionDenied
		}
		output[i] = a.component(parent, component)
		parent = path.Join(parent, output[i])
	}
	name := path.Join(root, strings.Join(output, "/"))
	if directory {
		name += "/"
	}
	return name, nil
}

func (a *archiveNameAllocator) component(parent, source string) string {
	if _, ok := a.children[parent]; !ok {
		a.children[parent] = make(map[string]string)
		a.used[parent] = make(map[string]struct{})
	}
	if output, ok := a.children[parent][source]; ok {
		return output
	}

	output := sanitizeArchiveComponent(source)
	if output == "" {
		output = "%00"
	}
	key := archiveNameKey(output)
	if _, exists := a.used[parent][key]; exists {
		output = disambiguateArchiveComponent(output, source, a.used[parent])
		key = archiveNameKey(output)
	}
	a.children[parent][source] = output
	a.used[parent][key] = struct{}{}
	return output
}

func disambiguateArchiveComponent(base, source string, used map[string]struct{}) string {
	hash := sha256.Sum256([]byte(source))
	suffix := "~" + hex.EncodeToString(hash[:6])
	for index := 0; ; index++ {
		candidateSuffix := suffix
		if index > 0 {
			candidateSuffix += "-" + strconv.Itoa(index)
		}
		prefix := truncateArchiveComponent(base, 240-len(candidateSuffix))
		candidate := prefix + candidateSuffix
		if _, exists := used[archiveNameKey(candidate)]; !exists {
			return candidate
		}
	}
}

func truncateArchiveComponent(name string, maxBytes int) string {
	if len(name) <= maxBytes {
		return name
	}
	name = name[:maxBytes]
	for !utf8.ValidString(name) {
		name = name[:len(name)-1]
	}
	return name
}

func archiveNameKey(name string) string {
	return strings.ToLower(strings.TrimRight(name, " ."))
}

func sanitizeArchiveComponent(name string) string {
	name = archivePathReplacer.Replace(name)
	trimmedEnd := len(name)
	for trimmedEnd > 0 && (name[trimmedEnd-1] == '.' || name[trimmedEnd-1] == ' ') {
		trimmedEnd--
	}
	reserved := isWindowsReservedArchiveName(name)
	var output strings.Builder
	for i := 0; i < len(name); i++ {
		value := name[i]
		invalid := value < 0x20 || value == 0x7f || strings.ContainsRune(`<>"|?*`, rune(value))
		trailing := i >= trimmedEnd && (value == '.' || value == ' ')
		if (reserved && i == 0) || invalid || trailing {
			output.WriteByte('%')
			output.WriteByte("0123456789ABCDEF"[value>>4])
			output.WriteByte("0123456789ABCDEF"[value&0x0f])
		} else {
			output.WriteByte(value)
		}
	}
	return output.String()
}

func isWindowsReservedArchiveName(name string) bool {
	stem := strings.ToLower(strings.SplitN(strings.TrimRight(name, " ."), ".", 2)[0])
	switch stem {
	case "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9":
		return true
	default:
		return false
	}
}

// WriteTo streams a ZIP after its traversal and path boundaries have been checked.
func (a *DirectoryArchive) WriteTo(ctx context.Context, destination io.Writer) error {
	if a == nil || a.fs == nil || a.root == "" {
		return ErrInvalidOperation
	}
	zipWriter := zip.NewWriter(destination)
	root := a.zipRoot
	rootHeader := &zip.FileHeader{Name: root + "/", Method: zip.Store}
	rootHeader.SetMode(fs.ModeDir | 0o755)
	_, writeErr := zipWriter.CreateHeader(rootHeader)
	if writeErr == nil {
		for _, entry := range a.entries {
			if err := ctx.Err(); err != nil {
				writeErr = err
				break
			}
			header := &zip.FileHeader{Name: entry.archiveName, Modified: entry.info.ModTime()}
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
