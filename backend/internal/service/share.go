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
	"path/filepath"
	"sort"
	"sync"
	"time"

	"github.com/google/uuid"
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

// ShareService manages single-file share links.
type ShareService interface {
	// Create shares an existing regular file and returns the new share.
	Create(ctx context.Context, username string, path string, permissions model.SharePermissions, expiresAt time.Time) (*model.Share, error)
	// List returns active (non-revoked, non-expired) shares, newest first.
	List() ([]model.Share, error)
	// Revoke permanently disables a share by ID.
	Revoke(id string) error
	// ResolveForRecipient returns the share for a token, or ErrShareNotFound for
	// unknown, expired, or revoked tokens.
	ResolveForRecipient(token string) (*model.Share, error)
	// OpenForRecipient opens the shared file for reading after re-validating the
	// target against the live mount list.
	OpenForRecipient(ctx context.Context, token string) (File, *model.FileInfo, error)
	// WriteForRecipient atomically overwrites the shared file from body and
	// returns the number of bytes written.
	WriteForRecipient(ctx context.Context, token string, body io.Reader) (int64, error)
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
	mu             sync.RWMutex
}

// shareRecord is the persistence shape for a share. It is separate from
// model.Share so internal routing fields stay out of any marshaled API response.
type shareRecord struct {
	ID          string                 `json:"id"`
	Token       string                 `json:"token"`
	MountName   string                 `json:"mountName"`
	RelPath     string                 `json:"relPath"`
	Permissions model.SharePermissions `json:"permissions"`
	FileName    string                 `json:"fileName"`
	CreatedAt   time.Time              `json:"createdAt"`
	ExpiresAt   time.Time              `json:"expiresAt,omitempty"`
	Revoked     bool                   `json:"revoked"`
	CreatedBy   string                 `json:"createdBy"`
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
	return &shareService{
		fs:             fsys,
		filePath:       filepath.Join(dataDir, config.SharesFileName),
		maxUploadBytes: maxUploadBytes,
		mounts:         mounts,
	}
}

func (s *shareService) Create(ctx context.Context, username string, path string, permissions model.SharePermissions, expiresAt time.Time) (*model.Share, error) {
	if username == "" {
		return nil, ErrInvalidOperation
	}

	mount, fsPath, err := validator.ValidatePathAgainstMounts(path, s.mounts())
	if err != nil {
		return nil, err
	}
	if permissions.Write && mount.ReadOnly {
		return nil, ErrPermissionDenied
	}
	fsPath, err = resolveExistingPathWithinMount(s.fs, mount, fsPath)
	if err != nil {
		return nil, err
	}
	info, err := s.statTarget(fsPath)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, ErrNotFile
	}
	relPath, err := filepath.Rel(mount.Path, fsPath)
	if err != nil {
		return nil, err
	}

	token, err := generateShareToken()
	if err != nil {
		return nil, err
	}
	share := model.Share{
		ID:          uuid.NewString(),
		Token:       token,
		MountName:   mount.Name,
		RelPath:     relPath,
		Permissions: permissions,
		FileName:    info.Name(),
		CreatedAt:   time.Now().UTC(),
		ExpiresAt:   expiresAt,
		CreatedBy:   username,
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

func (s *shareService) List() ([]model.Share, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	data := s.loadLocked()
	now := time.Now()
	shares := make([]model.Share, 0, len(data.Shares))
	for _, record := range data.Shares {
		if record.Revoked || isExpired(record.ExpiresAt, now) {
			continue
		}
		shares = append(shares, fromShareRecord(record))
	}
	sort.Slice(shares, func(i, j int) bool { return shares[i].CreatedAt.After(shares[j].CreatedAt) })
	return shares, nil
}

func (s *shareService) Revoke(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	data := s.loadLocked()
	for i := range data.Shares {
		if data.Shares[i].ID == id {
			data.Shares[i].Revoked = true
			return s.saveLocked(data)
		}
	}
	return ErrShareNotFound
}

func (s *shareService) ResolveForRecipient(token string) (*model.Share, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	now := time.Now()
	for _, record := range s.loadLocked().Shares {
		if record.Token != token {
			continue
		}
		if record.Revoked || isExpired(record.ExpiresAt, now) {
			return nil, ErrShareNotFound
		}
		share := fromShareRecord(record)
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

func (s *shareService) WriteForRecipient(ctx context.Context, token string, body io.Reader) (int64, error) {
	share, err := s.ResolveForRecipient(token)
	if err != nil {
		return 0, err
	}
	if !share.Permissions.Write {
		return 0, ErrPermissionDenied
	}

	mount, fsPath, err := s.resolveShareTarget(share)
	if err != nil {
		return 0, err
	}
	if mount.ReadOnly {
		return 0, ErrPermissionDenied
	}
	info, err := s.statTarget(fsPath)
	if err != nil {
		return 0, err
	}
	if !info.Mode().IsRegular() {
		return 0, ErrNotFile
	}

	// Write to a temporary file next to the resolved target, then rename over
	// it. Renaming the resolved path never writes through a symlink and leaves
	// the original intact if the upload fails partway.
	temporary := fsPath + ".share." + uuid.NewString()
	file, err := s.fs.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, info.Mode().Perm())
	if err != nil {
		return 0, err
	}
	complete := false
	defer func() {
		_ = file.Close()
		if !complete {
			_ = s.fs.Remove(temporary)
		}
	}()

	written, copyErr := io.Copy(file, io.LimitReader(body, s.maxUploadBytes+1))
	if copyErr != nil {
		return 0, copyErr
	}
	if written == 0 {
		return 0, ErrInvalidOperation
	}
	if written > s.maxUploadBytes {
		return 0, ErrShareTooLarge
	}
	if err := file.Sync(); err != nil {
		return 0, err
	}
	if err := file.Close(); err != nil {
		return 0, err
	}
	if err := s.fs.Rename(temporary, fsPath); err != nil {
		return 0, err
	}
	complete = true
	return written, nil
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
		return nil, "", err
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
	if err := s.fs.MkdirAll(filepath.Dir(s.filePath), 0755); err != nil {
		return err
	}
	fileData, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}
	// Write to a temporary sibling, then rename so a crash mid-write can never
	// leave a half-written shares file behind.
	temporary := s.filePath + ".tmp." + uuid.NewString()
	if err := s.fs.WriteFile(temporary, fileData, 0644); err != nil {
		_ = s.fs.Remove(temporary)
		return err
	}
	if err := s.fs.Rename(temporary, s.filePath); err != nil {
		_ = s.fs.Remove(temporary)
		return err
	}
	return nil
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
		ID:          share.ID,
		Token:       share.Token,
		MountName:   share.MountName,
		RelPath:     share.RelPath,
		Permissions: share.Permissions,
		FileName:    share.FileName,
		CreatedAt:   share.CreatedAt,
		ExpiresAt:   share.ExpiresAt,
		Revoked:     share.Revoked,
		CreatedBy:   share.CreatedBy,
	}
}

func fromShareRecord(record shareRecord) model.Share {
	return model.Share{
		ID:          record.ID,
		Token:       record.Token,
		MountName:   record.MountName,
		RelPath:     record.RelPath,
		Permissions: record.Permissions,
		FileName:    record.FileName,
		CreatedAt:   record.CreatedAt,
		ExpiresAt:   record.ExpiresAt,
		Revoked:     record.Revoked,
		CreatedBy:   record.CreatedBy,
	}
}
