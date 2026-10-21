//go:build linux

package service

import (
	"context"
	"errors"
	"fmt"
	"net/url"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"golang.org/x/sys/unix"
)

type desktopTrashService struct {
	fs     filesystem.FS
	mounts *mounts
	cfg    desktopTrashConfig
	mu     sync.Mutex
}

type desktopTrashEntry struct {
	item        DesktopTrashItem
	location    desktopTrashLocation
	storageName string
	infoName    string
}

func newDesktopTrashService(fsys filesystem.FS, mountPoints []model.MountPoint, cfg desktopTrashConfig) *desktopTrashService {
	if cfg.mountInfoPath == "" {
		cfg.mountInfoPath = "/proc/self/mountinfo"
	}
	if cfg.now == nil {
		cfg.now = time.Now
	}
	return &desktopTrashService{fs: fsys, mounts: newMounts(fsys, fixedMounts(mountPoints)), cfg: cfg}
}

func (s *desktopTrashService) List(ctx context.Context) ([]DesktopTrashItem, error) {
	entries, err := s.scan(ctx)
	if err != nil {
		return nil, err
	}
	items := make([]DesktopTrashItem, len(entries))
	for i, entry := range entries {
		items[i] = entry.item
	}
	sort.Slice(items, func(i, j int) bool {
		if items[i].DeletedAt.Equal(items[j].DeletedAt) {
			return items[i].Name < items[j].Name
		}
		return items[i].DeletedAt.After(items[j].DeletedAt)
	})
	return items, nil
}

func (s *desktopTrashService) Trash(ctx context.Context, virtualPath string) (DesktopTrashItem, error) {
	if err := ctx.Err(); err != nil {
		return DesktopTrashItem{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	_, sourcePath, sourceName, sourceParentFD, isDir, err := s.resolveTrashSource(virtualPath)
	if err != nil {
		return DesktopTrashItem{}, err
	}
	defer unix.Close(sourceParentFD)

	dataHome, err := s.dataHome(true)
	if err != nil {
		return DesktopTrashItem{}, fmt.Errorf("%w: %v", ErrTrashUnavailable, err)
	}
	mountInfo, err := s.readMountInfo()
	if err != nil {
		return DesktopTrashItem{}, fmt.Errorf("%w: could not inspect mounted filesystems", ErrTrashUnavailable)
	}
	sourceMount, err := mountInfoForPath(mountInfo, filepath.Dir(sourcePath))
	if err != nil {
		return DesktopTrashItem{}, fmt.Errorf("%w: source filesystem is not mounted", ErrTrashUnavailable)
	}
	homeMount, err := mountInfoForPath(mountInfo, dataHome)
	if err != nil {
		return DesktopTrashItem{}, fmt.Errorf("%w: home trash filesystem is not mounted", ErrTrashUnavailable)
	}

	location := desktopTrashLocation{root: dataHome, relative: "Trash", originBase: dataHome, home: true}
	if sourceMount.path != homeMount.path {
		if isRemoteFilesystem(sourceMount.fstype) {
			return DesktopTrashItem{}, fmt.Errorf("%w: trash on %s is unsupported", ErrTrashUnavailable, sourceMount.fstype)
		}
		location, err = s.newVolumeTrashLocation(sourceMount.path)
		if err != nil {
			return DesktopTrashItem{}, err
		}
	}

	dirs, err := s.openTrashDirs(location, true)
	if err != nil {
		return DesktopTrashItem{}, fmt.Errorf("%w: could not prepare trash on %s", ErrTrashUnavailable, location.volumeRootOrHome())
	}
	defer dirs.close()

	originalInfoPath, err := trashInfoOriginalPath(location, sourcePath)
	if err != nil {
		return DesktopTrashItem{}, err
	}
	deletedAt := s.cfg.now().Local()
	infoContents := []byte("[Trash Info]\nPath=" + (&url.URL{Path: originalInfoPath}).EscapedPath() +
		"\nDeletionDate=" + deletedAt.Format("2006-01-02T15:04:05") + "\n")

	for collision := 0; collision < 10000; collision++ {
		if err := ctx.Err(); err != nil {
			return DesktopTrashItem{}, err
		}
		storageName := collisionName(filepath.Base(sourcePath), collision)
		infoName := storageName + ".trashinfo"
		infoFD, openErr := unix.Openat(dirs.infoFD, infoName, unix.O_WRONLY|unix.O_CREAT|unix.O_EXCL|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0600)
		if errors.Is(openErr, unix.EEXIST) {
			continue
		}
		if openErr != nil {
			return DesktopTrashItem{}, fmt.Errorf("%w: could not create trash metadata", ErrTrashUnavailable)
		}
		writeErr := writeAndClose(infoFD, infoContents)
		if writeErr != nil {
			_ = unix.Unlinkat(dirs.infoFD, infoName, 0)
			return DesktopTrashItem{}, fmt.Errorf("%w: could not write trash metadata", ErrTrashUnavailable)
		}
		if err := ctx.Err(); err != nil {
			_ = unix.Unlinkat(dirs.infoFD, infoName, 0)
			return DesktopTrashItem{}, err
		}

		moveErr := unix.Renameat2(sourceParentFD, sourceName, dirs.filesFD, storageName, unix.RENAME_NOREPLACE)
		if errors.Is(moveErr, unix.EEXIST) {
			_ = unix.Unlinkat(dirs.infoFD, infoName, 0)
			continue
		}
		if moveErr != nil {
			_ = unix.Unlinkat(dirs.infoFD, infoName, 0)
			if errors.Is(moveErr, unix.EXDEV) {
				return DesktopTrashItem{}, fmt.Errorf("%w: Trash must be on the same filesystem", ErrTrashUnavailable)
			}
			return DesktopTrashItem{}, fmt.Errorf("move into trash: %w", moveErr)
		}

		item := DesktopTrashItem{
			ID:           trashItemID(location, storageName),
			Name:         filepath.Base(sourcePath),
			OriginalPath: sourcePath,
			DeletedAt:    deletedAt,
			IsDir:        isDir,
		}
		item.CanRestore = s.canRestoreTo(item.OriginalPath)
		return item, nil
	}
	return DesktopTrashItem{}, fmt.Errorf("%w: too many items share this name", ErrTrashUnavailable)
}

func (s *desktopTrashService) Restore(ctx context.Context, id string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	entry, err := s.findEntry(ctx, id)
	if err != nil {
		return err
	}
	if entry.item.OriginalPath == "" {
		return ErrTrashCannotRestore
	}
	destination, mount, mountRoot, relative, err := s.resolveOriginalDestination(entry.item.OriginalPath)
	if err != nil {
		return err
	}
	if destinationExists(entry.item.OriginalPath) {
		return ErrTrashConflict
	}

	dirs, err := s.openTrashDirs(entry.location, false)
	if err != nil {
		return ErrTrashItemNotFound
	}
	defer dirs.close()

	rootFD, err := openAbsoluteDir(mountRoot, false)
	if err != nil {
		return ErrTrashCannotRestore
	}
	defer unix.Close(rootFD)
	parentRelative := filepath.Dir(relative)
	destinationParentFD, err := openDirectoryRelative(rootFD, parentRelative, true)
	if err != nil {
		return ErrTrashCannotRestore
	}
	defer unix.Close(destinationParentFD)

	// Resolve again after creating missing parents, so a changed symlink cannot
	// redirect the restore to a different path inside the mount.
	virtualPath := virtualPathFor(mount.Name, relative)
	resolvedMount, resolved, err := s.mounts.resolve(virtualPath, writeMaybeMissing)
	if err != nil || resolvedMount.Name != mount.Name || resolved != destination {
		return ErrTrashCannotRestore
	}
	if destinationExists(destination) {
		return ErrTrashConflict
	}

	if err := unix.Renameat2(dirs.filesFD, entry.storageName, destinationParentFD, filepath.Base(relative), unix.RENAME_NOREPLACE); err != nil {
		if errors.Is(err, unix.EEXIST) || errors.Is(err, unix.ENOTEMPTY) {
			return ErrTrashConflict
		}
		if errors.Is(err, unix.ENOENT) {
			return ErrTrashItemNotFound
		}
		return fmt.Errorf("restore from trash: %w", err)
	}
	_ = unix.Unlinkat(dirs.infoFD, entry.infoName, 0)
	return nil
}

func (s *desktopTrashService) Delete(ctx context.Context, id string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	entry, err := s.findEntry(ctx, id)
	if err != nil {
		return err
	}
	dirs, err := s.openTrashDirs(entry.location, false)
	if err != nil {
		return ErrTrashItemNotFound
	}
	defer dirs.close()
	if err := removeTreeAt(dirs.filesFD, entry.storageName); err != nil {
		if errors.Is(err, unix.ENOENT) {
			return ErrTrashItemNotFound
		}
		return fmt.Errorf("delete trash item: %w", err)
	}
	_ = unix.Unlinkat(dirs.infoFD, entry.infoName, 0)
	return nil
}

func (s *desktopTrashService) resolveTrashSource(virtualPath string) (*model.MountPoint, string, string, int, bool, error) {
	mount, resolved, err := s.mounts.resolve(virtualPath, writeExisting)
	if err != nil {
		return nil, "", "", -1, false, err
	}
	relative, err := virtualRelativePath(mount.Name, virtualPath)
	if err != nil || relative == "." {
		return nil, "", "", -1, false, ErrInvalidOperation
	}
	root, err := s.fs.EvalSymlinks(mount.Path)
	if err != nil {
		return nil, "", "", -1, false, err
	}
	parentRelative := filepath.Dir(relative)
	parentPath := root
	if parentRelative != "." {
		parentVirtual := virtualPathFor(mount.Name, parentRelative)
		parentMount, resolvedParent, resolveErr := s.mounts.resolve(parentVirtual, readExisting)
		if resolveErr != nil {
			return nil, "", "", -1, false, resolveErr
		}
		if parentMount.Name != mount.Name || !pathWithinRoot(root, resolvedParent) {
			return nil, "", "", -1, false, ErrPermissionDenied
		}
		parentPath = resolvedParent
	}
	name := filepath.Base(relative)
	path := filepath.Join(parentPath, name)
	if !pathWithinRoot(root, path) || path == root {
		return nil, "", "", -1, false, ErrInvalidOperation
	}
	resolvedPath, err := s.fs.EvalSymlinks(path)
	if err != nil || resolvedPath != resolved {
		return nil, "", "", -1, false, ErrPermissionDenied
	}
	parentFD, err := openAbsoluteDir(parentPath, false)
	if err != nil {
		return nil, "", "", -1, false, ErrPathNotFound
	}
	var stat unix.Stat_t
	if err := unix.Fstatat(parentFD, name, &stat, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		_ = unix.Close(parentFD)
		if errors.Is(err, unix.ENOENT) {
			return nil, "", "", -1, false, ErrPathNotFound
		}
		return nil, "", "", -1, false, err
	}
	return mount, path, name, parentFD, stat.Mode&unix.S_IFMT == unix.S_IFDIR, nil
}

func virtualRelativePath(mountName, requested string) (string, error) {
	decoded, err := url.PathUnescape(requested)
	if err != nil {
		return "", ErrInvalidOperation
	}
	if doubleDecoded, err := url.PathUnescape(decoded); err == nil {
		decoded = doubleDecoded
	}
	if strings.ContainsRune(decoded, 0) {
		return "", ErrInvalidOperation
	}
	clean := filepath.Clean(decoded)
	clean = strings.TrimLeft(clean, string(filepath.Separator))
	name := strings.TrimLeft(mountName, string(filepath.Separator))
	if clean == name {
		return ".", nil
	}
	prefix := name + string(filepath.Separator)
	if !strings.HasPrefix(clean, prefix) {
		return "", ErrInvalidOperation
	}
	relative := strings.TrimPrefix(clean, prefix)
	if filepath.IsAbs(relative) || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", ErrInvalidOperation
	}
	return relative, nil
}

func virtualPathFor(mountName, relative string) string {
	segments := strings.Split(filepath.ToSlash(relative), "/")
	for i := range segments {
		segments[i] = url.PathEscape(segments[i])
	}
	return url.PathEscape(mountName) + "/" + strings.Join(segments, "/")
}

func (s *desktopTrashService) resolveOriginalDestination(originalPath string) (string, *model.MountPoint, string, string, error) {
	if !filepath.IsAbs(originalPath) || filepath.Clean(originalPath) != originalPath {
		return "", nil, "", "", ErrTrashCannotRestore
	}
	for _, candidate := range s.mounts.list() {
		root, err := s.fs.EvalSymlinks(candidate.Path)
		if err != nil || !pathWithinRoot(root, originalPath) || originalPath == root {
			continue
		}
		relative, err := filepath.Rel(root, originalPath)
		if err != nil || relative == "." {
			continue
		}
		virtualPath := virtualPathFor(candidate.Name, relative)
		mount, resolved, err := s.mounts.resolve(virtualPath, writeMaybeMissing)
		if err != nil || mount.Name != candidate.Name || resolved != originalPath {
			continue
		}
		return resolved, mount, root, relative, nil
	}
	return "", nil, "", "", ErrTrashCannotRestore
}

func (s *desktopTrashService) canRestoreTo(originalPath string) bool {
	if originalPath == "" || destinationExists(originalPath) {
		return false
	}
	_, _, _, _, err := s.resolveOriginalDestination(originalPath)
	return err == nil
}

func (s *desktopTrashService) scan(ctx context.Context) ([]desktopTrashEntry, error) {
	locations, err := s.trashLocations()
	if err != nil {
		return nil, err
	}
	entries := make([]desktopTrashEntry, 0)
	for _, location := range locations {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		dirs, err := s.openTrashDirs(location, false)
		if err != nil {
			if errors.Is(err, unix.ENOENT) || errors.Is(err, ErrInvalidTrashItem) || errors.Is(err, unix.ELOOP) {
				continue
			}
			continue
		}
		items, readErr := readDirFD(dirs.filesFD)
		if readErr != nil {
			dirs.close()
			return nil, fmt.Errorf("%w: could not read trash contents", ErrTrashUnavailable)
		}
		for _, file := range items {
			if err := ctx.Err(); err != nil {
				dirs.close()
				return nil, err
			}
			var stat unix.Stat_t
			if err := unix.Fstatat(dirs.filesFD, file.Name(), &stat, unix.AT_SYMLINK_NOFOLLOW); err != nil {
				continue
			}
			entry := desktopTrashEntry{
				location:    location,
				storageName: file.Name(),
				infoName:    file.Name() + ".trashinfo",
				item: DesktopTrashItem{
					ID:        trashItemID(location, file.Name()),
					Name:      file.Name(),
					DeletedAt: time.Unix(stat.Mtim.Sec, stat.Mtim.Nsec).Local(),
					IsDir:     stat.Mode&unix.S_IFMT == unix.S_IFDIR,
				},
			}
			if dirs.infoFD >= 0 {
				contents, readInfoErr := readFileAt(dirs.infoFD, entry.infoName)
				if readInfoErr == nil {
					originalPath, deletedAt, parsed := parseTrashInfo(location, contents)
					if parsed {
						entry.item.OriginalPath = originalPath
						entry.item.Name = filepath.Base(originalPath)
						if !deletedAt.IsZero() {
							entry.item.DeletedAt = deletedAt
						}
						entry.item.CanRestore = s.canRestoreTo(originalPath)
					}
				}
			}
			entries = append(entries, entry)
		}
		dirs.close()
	}
	return entries, nil
}
