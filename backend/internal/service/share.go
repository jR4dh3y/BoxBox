package service

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"io/fs"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
	"uuid"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
	"github.com/rs/zerolog/log"
)

// Share service errors
var (
	// ErrShareNotFound is the uniform failure for unknown, expired, and revoked
	// share tokens so recipients cannot distinguish between them.
	ErrShareNotFound = errors.New("share not found")
	// ErrShareTooLarge is returned when a recipient upload exceeds the size limit.
	ErrShareTooLarge = errors.New("share upload exceeds size limit")
)

// ShareService manages file and folder share links.
type ShareService interface {
	// Create shares an existing file or directory and returns the new share.
	Create(ctx context.Context, username string, path string, settings ShareSettings, expiresAt time.Time) (*model.Share, error)
	// List returns the user's active (non-revoked, non-expired) shares, newest first.
	List(username string) ([]model.Share, error)
	// Update changes the permissions and upload limit on one of the user's folder shares.
	Update(username string, id string, settings ShareUpdateSettings) (*model.Share, error)
	// Revoke permanently disables one of the user's shares by ID.
	Revoke(username string, id string) error
	// ResolveForRecipient returns the share for a token, or ErrShareNotFound for
	// unknown, expired, or revoked tokens.
	ResolveForRecipient(token string) (*model.Share, error)
	// OpenForRecipient opens the shared file for reading after re-validating the
	// target against the live mount list.
	OpenForRecipient(ctx context.Context, token string) (File, *model.FileInfo, error)
	// InfoForRecipient returns metadata for either a shared file or folder.
	InfoForRecipient(ctx context.Context, token string) (*model.FileInfo, string, error)
	// ListForRecipient returns entries below a shared folder. relativePath is
	// always relative to the folder root.
	ListForRecipient(ctx context.Context, token string, relativePath string) (*model.ShareDirectoryResponse, error)
	// OpenForRecipientPath opens a file below a shared folder.
	OpenForRecipientPath(ctx context.Context, token string, relativePath string) (File, *model.FileInfo, error)
	// WriteForRecipientPath creates or replaces a file below a writable shared
	// folder and returns the number of bytes written and its name.
	WriteForRecipientPath(ctx context.Context, token string, relativePath string, body io.Reader) (int64, string, error)
	// DeleteForRecipientPath removes a file or subfolder below a deletable share.
	DeleteForRecipientPath(ctx context.Context, token string, relativePath string) error
	// PrepareDirectoryArchive validates a recipient folder and prepares its ZIP archive.
	PrepareDirectoryArchive(ctx context.Context, token string, relativePath string) (*DirectoryArchive, error)
}

// ShareSettings contains the recipient capabilities and per-link upload limit.
type ShareSettings struct {
	Permissions    model.SharePermissions
	MaxUploadBytes int64
}

// ShareUpdateSettings distinguishes an omitted upload limit from an explicit one.
type ShareUpdateSettings struct {
	Permissions    model.SharePermissions
	MaxUploadBytes *int64
}

// ShareServiceConfig holds configuration for the share service.
// Mounts supplies the live mount list at access time so shares whose mount was
// removed, renamed, or flipped read-only after creation are re-validated.
type ShareServiceConfig struct {
	DataDir        string
	MaxUploadBytes int64
	Mounts         func() []model.MountPoint
}

type shareService struct {
	fs             filesystem.FS
	filePath       string
	maxUploadBytes int64
	mounts         func() []model.MountPoint
	storageErr     error
	mu             sync.RWMutex
}

// shareRecord is the persistence shape for a share. It is separate from
// model.Share so internal routing fields stay out of any marshaled API response.
type shareRecord struct {
	ID             string                 `json:"id"`
	Token          string                 `json:"token"`
	MountName      string                 `json:"mountName"`
	RelPath        string                 `json:"relPath"`
	IsFolder       bool                   `json:"isFolder"`
	Permissions    model.SharePermissions `json:"permissions"`
	MaxUploadBytes int64                  `json:"maxUploadBytes,omitempty"`
	LegacyReplace  bool                   `json:"legacyReplace,omitempty"`
	FileName       string                 `json:"fileName"`
	CreatedAt      time.Time              `json:"createdAt"`
	ExpiresAt      time.Time              `json:"expiresAt,omitempty"`
	Revoked        bool                   `json:"revoked"`
	CreatedBy      string                 `json:"createdBy"`
}

type sharesData struct {
	Shares []shareRecord `json:"shares"`
}

func NewShareService(fsys filesystem.FS, cfg ShareServiceConfig) ShareService {
	dataDir := cfg.DataDir
	if dataDir == "" {
		dataDir = config.DefaultDataDir
	}
	maxUploadBytes := cfg.MaxUploadBytes
	if maxUploadBytes <= 0 {
		maxUploadBytes = int64(config.DefaultMaxUploadMB) * 1024 * 1024
	}
	mounts := cfg.Mounts
	if mounts == nil {
		mounts = func() []model.MountPoint { return nil }
	}
	shareSvc := &shareService{
		fs:             fsys,
		filePath:       filepath.Join(dataDir, config.SharesFileName),
		maxUploadBytes: maxUploadBytes,
		mounts:         mounts,
	}
	shareSvc.storageErr = shareSvc.secureExistingStore()
	return shareSvc
}

func (s *shareService) Create(ctx context.Context, username string, path string, settings ShareSettings, expiresAt time.Time) (*model.Share, error) {
	if s.storageErr != nil {
		return nil, s.storageErr
	}
	if username == "" {
		return nil, ErrInvalidOperation
	}
	maxUploadBytes, err := s.resolveUploadLimit(settings.MaxUploadBytes)
	if err != nil {
		return nil, err
	}
	mount, fsPath, err := validator.ValidatePathAgainstMounts(path, s.mounts())
	if err != nil {
		return nil, err
	}
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		return nil, err
	}
	info, err := s.statTarget(fsPath)
	if err != nil {
		return nil, err
	}
	isFolder := info.IsDir()
	permissions := settings.Permissions
	if !isFolder && !info.Mode().IsRegular() {
		return nil, ErrNotFile
	}
	if isFolder {
		if permissions.Delete && !permissions.Upload {
			return nil, ErrInvalidOperation
		}
		permissions.View = true
		permissions.Download = true
		if !permissions.Upload {
			permissions.Delete = false
			permissions.LegacyReplace = false
		}
	} else {
		// A file link always includes the complete file experience. There is no
		// useful standalone "edit" mode for a single file share.
		permissions = model.SharePermissions{View: true, Download: true}
	}
	if (permissions.Upload || permissions.Delete) && mount.ReadOnly {
		return nil, ErrPermissionDenied
	}
	mountRoot, err := s.fs.EvalSymlinks(mount.Path)
	if err != nil {
		return nil, err
	}
	relPath, err := filepath.Rel(mountRoot, fsPath)
	if err != nil {
		return nil, err
	}
	if relPath == "." {
		relPath = ""
	}

	token, err := generateShareToken()
	if err != nil {
		return nil, err
	}
	share := model.Share{
		ID:             uuid.New().String(),
		Token:          token,
		MountName:      mount.Name,
		RelPath:        relPath,
		IsFolder:       isFolder,
		Permissions:    permissions,
		MaxUploadBytes: maxUploadBytes,
		FileName:       info.Name(),
		CreatedAt:      time.Now().UTC(),
		ExpiresAt:      expiresAt,
		CreatedBy:      username,
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	data := s.loadLocked()
	data.Shares = append(data.Shares, toShareRecord(share))
	if err := s.saveLocked(data); err != nil {
		return nil, err
	}
	return &share, nil
}

func (s *shareService) resolveUploadLimit(requested int64) (int64, error) {
	if requested < 0 || requested > s.maxUploadBytes {
		return 0, ErrInvalidOperation
	}
	if requested == 0 {
		return s.maxUploadBytes, nil
	}
	return requested, nil
}

func (s *shareService) List(username string) ([]model.Share, error) {
	if s.storageErr != nil {
		return nil, s.storageErr
	}
	if username == "" {
		return nil, ErrInvalidOperation
	}

	s.mu.RLock()
	defer s.mu.RUnlock()

	data := s.loadLocked()
	now := time.Now()
	shares := make([]model.Share, 0, len(data.Shares))
	for _, record := range data.Shares {
		if record.CreatedBy != username || record.Revoked || isExpired(record.ExpiresAt, now) {
			continue
		}
		shares = append(shares, s.shareFromRecord(record))
	}
	sort.Slice(shares, func(i, j int) bool { return shares[i].CreatedAt.After(shares[j].CreatedAt) })
	return shares, nil
}

func (s *shareService) Revoke(username string, id string) error {
	if s.storageErr != nil {
		return s.storageErr
	}
	if username == "" {
		return ErrInvalidOperation
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	data := s.loadLocked()
	for i := range data.Shares {
		if data.Shares[i].ID == id && data.Shares[i].CreatedBy == username {
			data.Shares[i].Revoked = true
			return s.saveLocked(data)
		}
	}
	return ErrShareNotFound
}

func (s *shareService) Update(username string, id string, settings ShareUpdateSettings) (*model.Share, error) {
	if s.storageErr != nil {
		return nil, s.storageErr
	}
	if username == "" {
		return nil, ErrInvalidOperation
	}
	requestedPermissions := settings.Permissions
	if !requestedPermissions.View && !requestedPermissions.Download && !requestedPermissions.Upload && !requestedPermissions.Delete {
		return nil, ErrInvalidOperation
	}
	if requestedPermissions.Delete && !requestedPermissions.Upload {
		return nil, ErrInvalidOperation
	}
	permissions := model.SharePermissions{
		View:          true,
		Download:      true,
		Upload:        requestedPermissions.Upload,
		Delete:        requestedPermissions.Delete,
		LegacyReplace: requestedPermissions.LegacyReplace && requestedPermissions.Upload && !requestedPermissions.Delete,
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	data := s.loadLocked()
	for i := range data.Shares {
		record := &data.Shares[i]
		if record.ID != id {
			continue
		}
		if record.CreatedBy != username || record.Revoked || isExpired(record.ExpiresAt, time.Now()) || !record.IsFolder {
			return nil, ErrShareNotFound
		}
		share := s.shareFromRecord(*record)
		mount, _, err := s.resolveShareTarget(&share)
		if err != nil {
			return nil, err
		}
		if (permissions.Upload || permissions.Delete) && mount.ReadOnly {
			return nil, ErrPermissionDenied
		}
		maxUploadBytes := share.MaxUploadBytes
		if settings.MaxUploadBytes != nil {
			maxUploadBytes, err = s.resolveUploadLimit(*settings.MaxUploadBytes)
			if err != nil {
				return nil, err
			}
		}
		share.Permissions = permissions
		share.MaxUploadBytes = maxUploadBytes
		data.Shares[i] = toShareRecord(share)
		if err := s.saveLocked(data); err != nil {
			return nil, err
		}
		return &share, nil
	}
	return nil, ErrShareNotFound
}

func (s *shareService) ResolveForRecipient(token string) (*model.Share, error) {
	if s.storageErr != nil {
		return nil, s.storageErr
	}

	s.mu.RLock()
	defer s.mu.RUnlock()

	return s.resolveActiveShareLocked(token, time.Now())
}

func (s *shareService) resolveActiveShareLocked(token string, now time.Time) (*model.Share, error) {
	for _, record := range s.loadLocked().Shares {
		if record.Token != token {
			continue
		}
		if record.Revoked || isExpired(record.ExpiresAt, now) {
			return nil, ErrShareNotFound
		}
		share := s.shareFromRecord(record)
		return &share, nil
	}
	return nil, ErrShareNotFound
}

func (s *shareService) OpenForRecipient(ctx context.Context, token string) (File, *model.FileInfo, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return nil, nil, err
	}

	mount, fsPath, err := s.resolveShareTarget(share)
	if err != nil {
		return nil, nil, err
	}
	info, err := s.statTarget(fsPath)
	if err != nil {
		return nil, nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, nil, ErrNotFile
	}
	file, err := s.fs.Open(fsPath)
	if err != nil {
		return nil, nil, err
	}
	if err := verifyOpenFileWithinMount(s.fs, mount, file); err != nil {
		_ = file.Close()
		return nil, nil, err
	}

	fileInfo := fileutil.ToFileInfo(info.Name(), share.MountName+"/"+share.RelPath, info)
	return file, &fileInfo, nil
}

func (s *shareService) InfoForRecipient(ctx context.Context, token string) (*model.FileInfo, string, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return nil, "", err
	}
	_, fsPath, err := s.resolveShareTarget(share)
	if err != nil {
		return nil, "", err
	}
	info, err := s.statTarget(fsPath)
	if err != nil {
		return nil, "", err
	}
	if share.IsFolder && !info.IsDir() {
		return nil, "", ErrNotDirectory
	}
	if !share.IsFolder && !info.Mode().IsRegular() {
		return nil, "", ErrNotFile
	}
	result := fileutil.ToFileInfo(info.Name(), share.MountName+"/"+share.RelPath, info)
	if share.IsFolder {
		return &result, "inode/directory", nil
	}
	return &result, fileutil.DetectMimeType(info.Name()), nil
}

func (s *shareService) ListForRecipient(ctx context.Context, token string, relativePath string) (*model.ShareDirectoryResponse, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return nil, err
	}
	if !share.Permissions.View {
		return nil, ErrPermissionDenied
	}
	_, target, _, err := s.resolveFolderPath(share, relativePath, false)
	if err != nil {
		return nil, err
	}
	info, err := s.statTarget(target)
	if err != nil {
		return nil, err
	}
	if !info.IsDir() {
		return nil, ErrNotDirectory
	}

	const maxShareEntries = 10_000
	entries, truncated, err := s.fs.ReadDirLimit(target, maxShareEntries)
	if err != nil {
		return nil, err
	}
	if truncated {
		return nil, ErrDirectoryTooLarge
	}

	cleanPath, err := cleanShareRelativePath(relativePath)
	if err != nil {
		return nil, err
	}
	items := make([]model.ShareItem, 0, len(entries))
	for _, entry := range entries {
		itemPath := filepath.ToSlash(filepath.Join(cleanPath, entry.Name()))
		_, resolvedEntry, _, resolveErr := s.resolveFolderPath(share, itemPath, false)
		if resolveErr != nil {
			// Do not expose broken links or entries that escape the shared root.
			continue
		}
		entryInfo, statErr := s.statTarget(resolvedEntry)
		if statErr != nil || (!entryInfo.IsDir() && !entryInfo.Mode().IsRegular()) {
			continue
		}
		item := model.ShareItem{
			Name:    entryInfo.Name(),
			Path:    itemPath,
			Size:    entryInfo.Size(),
			IsDir:   entryInfo.IsDir(),
			ModTime: entryInfo.ModTime(),
		}
		if !entryInfo.IsDir() {
			item.MimeType = fileutil.DetectMimeType(entryInfo.Name())
		}
		items = append(items, item)
	}

	sort.SliceStable(items, func(i, j int) bool {
		if items[i].IsDir != items[j].IsDir {
			return items[i].IsDir
		}
		return strings.ToLower(items[i].Name) < strings.ToLower(items[j].Name)
	})
	return &model.ShareDirectoryResponse{Path: cleanPath, Items: items}, nil
}

func (s *shareService) PrepareDirectoryArchive(ctx context.Context, token string, relativePath string) (*DirectoryArchive, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return nil, err
	}
	if !share.Permissions.Download {
		return nil, ErrPermissionDenied
	}
	_, target, root, err := s.resolveFolderPath(share, relativePath, false)
	if err != nil {
		return nil, err
	}
	return prepareDirectoryArchive(ctx, s.fs, target, root)
}

func (s *shareService) OpenForRecipientPath(ctx context.Context, token string, relativePath string) (File, *model.FileInfo, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return nil, nil, err
	}
	_, target, _, err := s.resolveFolderPath(share, relativePath, false)
	if err != nil {
		return nil, nil, err
	}
	info, err := s.statTarget(target)
	if err != nil {
		return nil, nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, nil, ErrNotFile
	}
	file, err := s.fs.Open(target)
	if err != nil {
		return nil, nil, err
	}
	mount, _, err := s.resolveShareTarget(share)
	if err != nil {
		_ = file.Close()
		return nil, nil, err
	}
	if err := verifyOpenFileWithinMount(s.fs, mount, file); err != nil {
		_ = file.Close()
		return nil, nil, err
	}
	cleanPath, err := cleanShareRelativePath(relativePath)
	if err != nil {
		_ = file.Close()
		return nil, nil, err
	}
	fileInfo := fileutil.ToFileInfo(info.Name(), cleanPath, info)
	return file, &fileInfo, nil
}

func (s *shareService) WriteForRecipientPath(ctx context.Context, token string, relativePath string, body io.Reader) (int64, string, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return 0, "", err
	}
	if !share.IsFolder || !share.Permissions.Upload {
		return 0, "", ErrPermissionDenied
	}
	if err := ctx.Err(); err != nil {
		return 0, "", err
	}

	mount, target, _, err := s.resolveFolderPath(share, relativePath, true)
	if err != nil {
		return 0, "", err
	}
	if mount.ReadOnly {
		return 0, "", ErrPermissionDenied
	}
	canReplace := share.Permissions.Delete || share.Permissions.LegacyReplace
	replacementMode := os.FileMode(0o644)
	if exists, existsErr := s.fs.Exists(target); existsErr != nil {
		return 0, "", existsErr
	} else if exists {
		if !canReplace {
			return 0, "", ErrPermissionDenied
		}
		info, statErr := s.fs.Stat(target)
		if statErr != nil {
			return 0, "", statErr
		}
		if info.IsDir() || !info.Mode().IsRegular() {
			return 0, "", ErrNotFile
		}
		replacementMode = info.Mode().Perm()
	}
	parentInfo, err := s.statTarget(filepath.Dir(target))
	if err != nil {
		return 0, "", err
	}
	if !parentInfo.IsDir() {
		return 0, "", ErrNotDirectory
	}

	temporary := target + ".share." + uuid.New().String()
	file, err := s.fs.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, replacementMode)
	if err != nil {
		return 0, "", err
	}
	if err := s.fs.Chmod(temporary, replacementMode); err != nil {
		_ = file.Close()
		_ = s.fs.Remove(temporary)
		return 0, "", err
	}
	complete := false
	defer func() {
		_ = file.Close()
		if !complete {
			_ = s.fs.Remove(temporary)
		}
	}()

	written, copyErr := io.Copy(file, io.LimitReader(body, share.MaxUploadBytes+1))
	if copyErr != nil {
		return 0, "", copyErr
	}
	if written == 0 {
		return 0, "", ErrInvalidOperation
	}
	if written > share.MaxUploadBytes {
		return 0, "", ErrShareTooLarge
	}
	if err := file.Sync(); err != nil {
		return 0, "", err
	}
	if err := file.Close(); err != nil {
		return 0, "", err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	share, err = s.resolveActiveShareLocked(token, time.Now())
	if err != nil {
		return 0, "", err
	}
	if !share.IsFolder || !share.Permissions.Upload {
		return 0, "", ErrPermissionDenied
	}
	canReplace = share.Permissions.Delete || share.Permissions.LegacyReplace
	if written > share.MaxUploadBytes {
		return 0, "", ErrShareTooLarge
	}
	finalMount, finalPath, _, err := s.resolveFolderPath(share, relativePath, true)
	if err != nil {
		return 0, "", err
	}
	if finalMount.ReadOnly || finalPath != target {
		return 0, "", ErrPermissionDenied
	}
	if exists, existsErr := s.fs.Exists(finalPath); existsErr != nil {
		return 0, "", existsErr
	} else if exists {
		if !canReplace {
			return 0, "", ErrPermissionDenied
		}
		finalInfo, statErr := s.fs.Stat(finalPath)
		if statErr != nil {
			return 0, "", statErr
		}
		if finalInfo.IsDir() || !finalInfo.Mode().IsRegular() {
			return 0, "", ErrNotFile
		}
		if err := s.fs.Chmod(temporary, finalInfo.Mode().Perm()); err != nil {
			return 0, "", err
		}
	}
	if canReplace {
		err = s.fs.Rename(temporary, finalPath)
	} else {
		err = s.fs.RenameNoReplace(temporary, finalPath)
		if errors.Is(err, fs.ErrExist) {
			return 0, "", ErrPermissionDenied
		}
	}
	if err != nil {
		return 0, "", err
	}
	complete = true
	return written, filepath.Base(finalPath), nil
}

func (s *shareService) DeleteForRecipientPath(ctx context.Context, token string, relativePath string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	s.mu.RLock()
	defer s.mu.RUnlock()

	share, err := s.resolveActiveShareLocked(token, time.Now())
	if err != nil {
		return err
	}
	if !share.IsFolder || !share.Permissions.Delete {
		return ErrPermissionDenied
	}
	mount, target, root, err := s.resolveFolderPath(share, relativePath, false)
	if err != nil {
		return err
	}
	cleanPath, err := cleanShareRelativePath(relativePath)
	if err != nil {
		return err
	}
	entryPath := filepath.Join(root, filepath.FromSlash(cleanPath))
	if mount.ReadOnly || cleanPath == "" || !pathWithinRoot(root, target) || !pathWithinRoot(root, entryPath) {
		return ErrPermissionDenied
	}
	components := strings.Split(cleanPath, "/")
	current := root
	for index, component := range components {
		current = filepath.Join(current, component)
		info, err := s.fs.Lstat(current)
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			if index != len(components)-1 {
				return ErrPermissionDenied
			}
			return s.fs.Remove(current)
		}
		if index < len(components)-1 && !info.IsDir() {
			return ErrNotDirectory
		}
		if index == len(components)-1 && !info.IsDir() && !info.Mode().IsRegular() {
			return ErrNotFile
		}
	}
	return s.fs.RemoveAll(entryPath)
}

func (s *shareService) resolveFolderPath(share *model.Share, relativePath string, allowMissing bool) (*model.MountPoint, string, string, error) {
	if !share.IsFolder {
		return nil, "", "", ErrNotDirectory
	}
	mount, root, err := s.resolveShareTarget(share)
	if err != nil {
		return nil, "", "", err
	}
	rootInfo, err := s.statTarget(root)
	if err != nil {
		return nil, "", "", err
	}
	if !rootInfo.IsDir() {
		return nil, "", "", ErrNotDirectory
	}
	cleanPath, err := cleanShareRelativePath(relativePath)
	if err != nil {
		return nil, "", "", err
	}
	if cleanPath == "" {
		return mount, root, root, nil
	}
	candidate := filepath.Join(root, filepath.FromSlash(cleanPath))
	if allowMissing {
		resolved, resolveErr := resolveWritablePathWithinMount(s.fs, mount, candidate)
		if resolveErr != nil {
			return nil, "", "", resolveErr
		}
		if !pathWithinRoot(root, resolved) {
			return nil, "", "", ErrPermissionDenied
		}
		return mount, resolved, root, nil
	}
	resolved, resolveErr := resolveExistingPathWithinMount(s.fs, mount, candidate)
	if resolveErr != nil {
		return nil, "", "", resolveErr
	}
	if !pathWithinRoot(root, resolved) {
		return nil, "", "", ErrPermissionDenied
	}
	return mount, resolved, root, nil
}

func cleanShareRelativePath(relativePath string) (string, error) {
	if strings.ContainsRune(relativePath, 0) {
		return "", validator.ErrInvalidPath
	}
	normalized := strings.ReplaceAll(relativePath, "\\", "/")
	if strings.HasPrefix(normalized, "/") {
		return "", validator.ErrPathTraversal
	}
	cleaned := path.Clean(normalized)
	if cleaned == "." {
		return "", nil
	}
	if cleaned == ".." || strings.HasPrefix(cleaned, "../") {
		return "", validator.ErrPathTraversal
	}
	return cleaned, nil
}

// resolveShareTarget re-resolves a share's mount-relative path against the live
// mount list. A mount that no longer exists makes the share unresolvable, which
// is reported as ErrShareNotFound so recipients see a uniform failure.
func (s *shareService) resolveShareTarget(share *model.Share) (*model.MountPoint, string, error) {
	mount, fsPath, err := validator.ValidatePathAgainstMounts(share.MountName+"/"+share.RelPath, s.mounts())
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) || errors.Is(err, validator.ErrMountPointNotFound) {
			return nil, "", ErrShareNotFound
		}
		return nil, "", err
	}
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		if errors.Is(err, ErrPathNotFound) || errors.Is(err, ErrPermissionDenied) {
			return nil, "", ErrShareNotFound
		}
		return nil, "", err
	}
	info, err := s.statTarget(fsPath)
	if err != nil {
		if errors.Is(err, ErrPathNotFound) {
			return nil, "", ErrShareNotFound
		}
		return nil, "", err
	}
	if (share.IsFolder && !info.IsDir()) || (!share.IsFolder && !info.Mode().IsRegular()) {
		return nil, "", ErrShareNotFound
	}
	return mount, fsPath, nil
}

// statTarget stats a resolved share target, mapping a missing file to
// ErrPathNotFound across both the OS and in-memory filesystems (whose Stat
// errors do not uniformly satisfy fs.ErrNotExist).
func (s *shareService) statTarget(fsPath string) (fs.FileInfo, error) {
	exists, err := s.fs.Exists(fsPath)
	if err != nil {
		return nil, err
	}
	if !exists {
		return nil, ErrPathNotFound
	}
	return s.fs.Stat(fsPath)
}

// secureExistingStore tightens storage created by earlier BoxBox versions
// before its bearer tokens can be served.
func (s *shareService) secureExistingStore() error {
	exists, err := s.fs.Exists(s.filePath)
	if err != nil || !exists {
		return err
	}
	if err := s.fs.Chmod(filepath.Dir(s.filePath), 0o700); err != nil {
		return err
	}
	return s.fs.Chmod(s.filePath, 0o600)
}

func (s *shareService) loadLocked() *sharesData {
	data := &sharesData{Shares: []shareRecord{}}
	exists, err := s.fs.Exists(s.filePath)
	if err != nil || !exists {
		return data
	}
	file, err := s.fs.ReadFile(s.filePath)
	if err != nil {
		log.Warn().Err(err).Str("path", s.filePath).Msg("Could not read shares file, starting empty")
		return data
	}
	if len(file) == 0 {
		return data
	}
	if err := json.Unmarshal(file, data); err != nil {
		log.Warn().Err(err).Str("path", s.filePath).Msg("Corrupt shares file, starting empty")
		return &sharesData{Shares: []shareRecord{}}
	}
	return data
}

func (s *shareService) saveLocked(data *sharesData) error {
	directory := filepath.Dir(s.filePath)
	if err := s.fs.MkdirAll(directory, 0o700); err != nil {
		return err
	}
	if err := s.fs.Chmod(directory, 0o700); err != nil {
		return err
	}
	fileData, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}
	// Write to a temporary sibling, then rename so a crash mid-write can never
	// leave a half-written shares file behind.
	temporary := s.filePath + ".tmp." + uuid.New().String()
	if err := s.fs.WriteFile(temporary, fileData, 0o600); err != nil {
		_ = s.fs.Remove(temporary)
		return err
	}
	if err := s.fs.Chmod(temporary, 0o600); err != nil {
		_ = s.fs.Remove(temporary)
		return err
	}
	if err := s.fs.Rename(temporary, s.filePath); err != nil {
		_ = s.fs.Remove(temporary)
		return err
	}
	return s.fs.Chmod(s.filePath, 0o600)
}

func generateShareToken() (string, error) {
	buf := make([]byte, 32)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(buf), nil
}

func isExpired(expiresAt time.Time, now time.Time) bool {
	return !expiresAt.IsZero() && !expiresAt.After(now)
}

func toShareRecord(share model.Share) shareRecord {
	return shareRecord{
		ID:             share.ID,
		Token:          share.Token,
		MountName:      share.MountName,
		RelPath:        share.RelPath,
		IsFolder:       share.IsFolder,
		Permissions:    share.Permissions,
		MaxUploadBytes: share.MaxUploadBytes,
		LegacyReplace:  share.Permissions.LegacyReplace,
		FileName:       share.FileName,
		CreatedAt:      share.CreatedAt,
		ExpiresAt:      share.ExpiresAt,
		Revoked:        share.Revoked,
		CreatedBy:      share.CreatedBy,
	}
}

func fromShareRecord(record shareRecord) model.Share {
	permissions := record.Permissions
	permissions.LegacyReplace = record.LegacyReplace || permissions.LegacyReplace
	if !permissions.Upload {
		permissions.Delete = false
		permissions.LegacyReplace = false
	}
	share := model.Share{
		ID:             record.ID,
		Token:          record.Token,
		MountName:      record.MountName,
		RelPath:        record.RelPath,
		IsFolder:       record.IsFolder,
		Permissions:    permissions,
		MaxUploadBytes: record.MaxUploadBytes,
		FileName:       record.FileName,
		CreatedAt:      record.CreatedAt,
		ExpiresAt:      record.ExpiresAt,
		Revoked:        record.Revoked,
		CreatedBy:      record.CreatedBy,
	}
	if !share.IsFolder {
		share.Permissions = model.SharePermissions{View: true, Download: true}
	} else {
		share.Permissions.View = true
		share.Permissions.Download = true
	}
	return share
}

func (s *shareService) shareFromRecord(record shareRecord) model.Share {
	share := fromShareRecord(record)
	if share.MaxUploadBytes <= 0 || share.MaxUploadBytes > s.maxUploadBytes {
		share.MaxUploadBytes = s.maxUploadBytes
	}
	return share
}
