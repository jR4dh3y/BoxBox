package service

import (
	"context"
	"errors"
	"io/fs"
	"path/filepath"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/rs/zerolog/log"
)

// Reasons a delete-on-expiry share keeps its file. Every reason is final.
var (
	errExpiryFileChanged = errors.New("file changed since it was shared")
	errExpiryFileShared  = errors.New("another active share still reaches the file")
)

// identifyDeletableFile records the file a delete-on-expiry share may remove.
// The selected entry must itself be a regular file on a writable mount: for a
// symlink, deleting its target would also break the link and anything else
// that uses the target.
func (s *shareService) identifyDeletableFile(virtualPath string, mount *model.MountPoint, fsPath string) (*model.ShareFileIdentity, error) {
	if mount.ReadOnly {
		return nil, ErrPermissionDenied
	}
	if err := s.mounts.requireWritablePath(fsPath, false); err != nil {
		return nil, err
	}
	_, entry, err := s.mounts.resolve(virtualPath, readEntry)
	if err != nil {
		return nil, err
	}
	info, err := s.fs.Lstat(entry)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, ErrInvalidOperation
	}
	identity := fileIdentity(info)
	return &identity, nil
}

func fileIdentity(info fs.FileInfo) model.ShareFileIdentity {
	device, inode := fileID(info)
	return model.ShareFileIdentity{
		Size:    info.Size(),
		ModTime: info.ModTime().UTC(),
		Device:  device,
		Inode:   inode,
	}
}

func sameFile(want model.ShareFileIdentity, info fs.FileInfo) bool {
	got := fileIdentity(info)
	return info.Mode().IsRegular() &&
		got.Size == want.Size &&
		got.ModTime.Equal(want.ModTime) &&
		got.Device == want.Device &&
		got.Inode == want.Inode
}

func (s *shareService) StartExpiryCleanup(ctx context.Context) {
	go func() {
		ticker := time.NewTicker(config.ShareExpiryCleanupInterval)
		defer ticker.Stop()
		for {
			s.deleteExpiredFiles(time.Now())
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
			}
		}
	}()
}

// deleteExpiredFiles makes one deletion attempt for each delete-on-expiry
// share that has expired. The flag is cleared and saved before any file is
// touched, so a crash or a failed save keeps the file instead of retrying a
// deletion later. Holding the lock keeps Create and Revoke out of the pass.
func (s *shareService) deleteExpiredFiles(now time.Time) {
	if s.storageErr != nil {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	data := s.loadLocked()
	var due []shareRecord
	for i := range data.Shares {
		record := &data.Shares[i]
		if record.DeleteOnExpiry == nil || record.Revoked || !isExpired(record.ExpiresAt, now) {
			continue
		}
		due = append(due, *record)
		record.DeleteOnExpiry = nil
	}
	if len(due) == 0 {
		return
	}
	if err := s.saveLocked(data); err != nil {
		log.Error().Err(err).Int("shares", len(due)).Msg("Kept files of expired shares: could not save the share store")
		return
	}

	reaches := s.activeShareReachesLocked(data, now)
	for _, record := range due {
		logger := log.With().
			Str("share", record.ID).
			Str("owner", record.CreatedBy).
			Str("path", shareVirtualPath(record)).
			Logger()
		if err := s.deleteExpiredFile(record, reaches); err != nil {
			logger.Warn().Err(err).Msg("Kept file of expired share")
			continue
		}
		logger.Info().Msg("Deleted file of expired share")
	}
}

// deleteExpiredFile deletes the file of one expired share when it is still
// the exact file that was shared. Any error means the file was kept.
func (s *shareService) deleteExpiredFile(record shareRecord, reaches []shareReach) error {
	if record.IsFolder || record.DeleteOnExpiry == nil {
		return ErrInvalidOperation
	}
	// writeEntry rejects read-only and missing mounts and does not follow a
	// final symlink, so a file swapped for a link is kept.
	mount, entry, err := s.mounts.resolve(shareVirtualPath(record), writeEntry)
	if err != nil {
		return err
	}
	root, err := s.fs.EvalSymlinks(mount.Path)
	if err != nil {
		return err
	}
	relPath := filepath.FromSlash(record.RelPath)
	// A parent folder replaced by a symlink would point the share elsewhere.
	if filepath.Join(root, relPath) != entry {
		return errExpiryFileChanged
	}
	for _, reach := range reaches {
		if reach.covers(record, entry) {
			return errExpiryFileShared
		}
	}
	identity := *record.DeleteOnExpiry
	return s.fs.RemoveFileWithin(root, relPath, func(info fs.FileInfo) error {
		if !sameFile(identity, info) {
			return errExpiryFileChanged
		}
		return nil
	})
}

// shareReach is the part of the mounts one active share exposes.
type shareReach struct {
	virtualPath string
	// realPath is empty when the target cannot be resolved now.
	realPath string
	folder   bool
	// unknown is set when an I/O error hid the target; it protects every file.
	unknown bool
}

func (s *shareService) activeShareReachesLocked(data *sharesData, now time.Time) []shareReach {
	reaches := make([]shareReach, 0, len(data.Shares))
	for _, record := range data.Shares {
		if record.Revoked || isExpired(record.ExpiresAt, now) {
			continue
		}
		reach := shareReach{virtualPath: shareVirtualPath(record), folder: record.IsFolder}
		_, realPath, err := s.mounts.resolve(reach.virtualPath, readExisting)
		switch {
		case err == nil:
			reach.realPath = realPath
		case errors.Is(err, ErrMountPointNotFound), errors.Is(err, ErrPathNotFound), errors.Is(err, ErrPermissionDenied):
			// The share cannot serve anything now; its logical path still protects the file.
		default:
			reach.unknown = true
		}
		reaches = append(reaches, reach)
	}
	return reaches
}

// covers reports whether the active share can reach the expired share's file,
// either by the same mount name and path or by the resolved real path. The real
// path catches a configured and a discovered mount name for the same folder.
func (r shareReach) covers(expired shareRecord, realPath string) bool {
	if r.unknown {
		return true
	}
	virtualPath := filepath.FromSlash(shareVirtualPath(expired))
	reachVirtual := filepath.FromSlash(r.virtualPath)
	if r.folder {
		return pathWithinRoot(reachVirtual, virtualPath) ||
			(r.realPath != "" && pathWithinRoot(r.realPath, realPath))
	}
	return reachVirtual == virtualPath || r.realPath == realPath
}

func shareVirtualPath(record shareRecord) string {
	return record.MountName + "/" + filepath.ToSlash(record.RelPath)
}
