package service

import (
	"context"
	"errors"
	"io/fs"
	"path/filepath"
	"slices"
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
// touched, so a crash during the pass keeps the file instead of retrying it.
// A failed save touches no file and leaves the flag for the next pass, since
// no decision about the file was made. Holding the lock keeps Create and
// Revoke out of the pass.
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

	for _, record := range due {
		// Reread the store and resolve each active link again right before each
		// deletion, so a link added by another process or a retargeted symlink
		// still protects the file.
		reaches := s.activeShareReachesLocked(s.loadLocked(), record.ExpiresAt, now)
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
	identity := *record.DeleteOnExpiry
	target := expiringTarget{
		virtualPath: shareVirtualPath(record),
		realPath:    entry,
		file:        fileKey{device: identity.Device, inode: identity.Inode},
		folders:     s.folderKeysAbove(entry),
	}
	for _, reach := range reaches {
		if reach.covers(target) {
			return errExpiryFileShared
		}
	}
	return s.fs.RemoveFileWithin(root, relPath, func(info fs.FileInfo) error {
		if !sameFile(identity, info) {
			return errExpiryFileChanged
		}
		return nil
	})
}

// fileKey names a file or folder on this host. It is zero where the platform
// has no stable file ID.
type fileKey struct {
	device uint64
	inode  uint64
}

func keyOf(info fs.FileInfo) fileKey {
	device, inode := fileID(info)
	return fileKey{device: device, inode: inode}
}

func (k fileKey) known() bool { return k.inode != 0 }

// folderKeysAbove returns the identity of every folder that contains realPath,
// up to the filesystem root. A folder bind-mounted at two paths has one
// identity, so a link through either path is found.
func (s *shareService) folderKeysAbove(realPath string) []fileKey {
	var keys []fileKey
	for folder := filepath.Dir(realPath); ; folder = filepath.Dir(folder) {
		if info, err := s.fs.Stat(folder); err == nil {
			if key := keyOf(info); key.known() {
				keys = append(keys, key)
			}
		}
		if parent := filepath.Dir(folder); parent == folder {
			return keys
		}
	}
}

// expiringTarget is the file an expired share is about to delete.
type expiringTarget struct {
	virtualPath string
	realPath    string
	file        fileKey
	// folders holds the identities of the folders that contain the file.
	folders []fileKey
}

// shareReach is the part of the mounts one active share exposes.
type shareReach struct {
	virtualPath string
	// realPath is empty when the target cannot be resolved now.
	realPath string
	// id is the resolved target's identity, zero when it is unknown.
	id     fileKey
	folder bool
	// unknown is set when an I/O error hid the target; it protects every file.
	unknown bool
}

// activeShareReachesLocked returns the links active at the expired share's
// deadline or now. Checking the deadline keeps the result the same when the
// server was down at the deadline and another link expired in the meantime.
func (s *shareService) activeShareReachesLocked(data *sharesData, deadline, now time.Time) []shareReach {
	reaches := make([]shareReach, 0, len(data.Shares))
	for _, record := range data.Shares {
		if !activeAt(record, deadline) && !activeAt(record, now) {
			continue
		}
		reach := shareReach{virtualPath: shareVirtualPath(record), folder: record.IsFolder}
		_, realPath, err := s.mounts.resolve(reach.virtualPath, readExisting)
		switch {
		case err == nil:
			reach.realPath = realPath
			if info, statErr := s.fs.Stat(realPath); statErr == nil {
				reach.id = keyOf(info)
			}
		case errors.Is(err, ErrMountPointNotFound), errors.Is(err, ErrPathNotFound), errors.Is(err, ErrPermissionDenied):
			// The share cannot serve anything now; its logical path still protects the file.
		default:
			reach.unknown = true
		}
		reaches = append(reaches, reach)
	}
	return reaches
}

// covers reports whether the active share can reach the expired share's file:
// by the same mount name and path, by the resolved real path, or by file and
// folder identity. The real path catches a configured and a discovered mount
// name for the same folder; identity catches bind mounts and hard links.
func (r shareReach) covers(target expiringTarget) bool {
	if r.unknown {
		return true
	}
	virtualPath := filepath.FromSlash(target.virtualPath)
	reachVirtual := filepath.FromSlash(r.virtualPath)
	if r.folder {
		return pathWithinRoot(reachVirtual, virtualPath) ||
			(r.realPath != "" && pathWithinRoot(r.realPath, target.realPath)) ||
			(r.id.known() && slices.Contains(target.folders, r.id))
	}
	return reachVirtual == virtualPath || r.realPath == target.realPath ||
		(r.id.known() && r.id == target.file)
}

// activeAt reports whether a link could be used at t. A link revoked before
// RevokedAt was recorded was revoked before delete-on-expiry existed, so it
// cannot overlap a delete-on-expiry deadline.
func activeAt(record shareRecord, t time.Time) bool {
	return !record.CreatedAt.After(t) && !isExpired(record.ExpiresAt, t) &&
		(!record.Revoked || record.RevokedAt.After(t))
}

func shareVirtualPath(record shareRecord) string {
	return record.MountName + "/" + filepath.ToSlash(record.RelPath)
}
