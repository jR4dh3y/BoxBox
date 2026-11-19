package service

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io/fs"
	"path/filepath"
	"slices"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/rs/zerolog/log"
)

// errNoRootMount means the mount table cannot place paths, which keeps files.
var errNoRootMount = errors.New("mount table has no root mount")

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

func sameIdentity(a, b model.ShareFileIdentity) bool {
	return a.Size == b.Size && a.ModTime.Equal(b.ModTime) && a.Device == b.Device && a.Inode == b.Inode
}

func (s *shareService) StartExpiryCleanup(ctx context.Context) {
	s.cleanup.Go(func() {
		ticker := time.NewTicker(config.ShareExpiryCleanupInterval)
		defer ticker.Stop()
		for {
			s.deleteExpiredFiles(ctx, time.Now())
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
			}
		}
	})
}

// StopExpiryCleanup waits for a running pass until ctx ends. A pass stops
// before touching any file once its context ends, so the wait is only long
// when a check is stuck on an unresponsive mount.
func (s *shareService) StopExpiryCleanup(ctx context.Context) error {
	done := make(chan struct{})
	go func() {
		s.cleanup.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// expiryPlan is the decision for one expired share, made without the lock.
type expiryPlan struct {
	record   shareRecord
	root     string
	relPath  string
	identity model.ShareFileIdentity
	// kept is why the file stays; nil means it may be removed.
	kept error
}

// deleteExpiredFiles makes one deletion attempt for each delete-on-expiry
// share that has expired.
//
// Resolving links and files can block on an unresponsive mount, and every
// share request needs the lock, so the checks run on a snapshot without it.
// The pass then takes the lock and acts only if the store still matches the
// snapshot; otherwise it leaves every flag for the next pass. The flags are
// cleared and saved before any file is touched, so a crash keeps the file
// instead of retrying it, and a failed save touches no file. Holding the lock
// while removing keeps Create from adding a link to a file being removed.
func (s *shareService) deleteExpiredFiles(ctx context.Context, now time.Time) {
	if s.storageErr != nil {
		return
	}
	s.mu.RLock()
	snapshot, err := s.readStoreLocked()
	s.mu.RUnlock()
	if err != nil {
		log.Error().Err(err).Msg("Skipped expired share cleanup: could not read the share store")
		return
	}
	due := dueForExpiry(snapshot, now)
	if len(due) == 0 {
		return
	}
	// A folder mounted inside a shared folder exposes files that live
	// elsewhere, so each pass needs the host's mount table.
	mounts, err := s.readMounts()
	if err == nil && len(mounts) > 0 && visibleMountAt(mounts, "/") == nil {
		// In a chroot that is not a mount point, the table has no root, so no
		// path can be placed on it and nested mounts would go unseen.
		err = errNoRootMount
	}
	if err != nil {
		log.Error().Err(err).Int("shares", len(due)).Msg("Skipped expired share cleanup: could not read the mount table")
		return
	}
	links := newShareLinks(s, snapshot.Shares, mounts)
	plans := make([]expiryPlan, 0, len(due))
	for _, record := range due {
		plans = append(plans, s.planExpiredFile(record, links.protecting(record.ExpiresAt, now), mounts))
	}
	if ctx.Err() != nil {
		return
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	data, err := s.readStoreLocked()
	if err != nil {
		log.Error().Err(err).Int("shares", len(due)).Msg("Skipped expired share cleanup: could not reread the share store")
		return
	}
	if !sameShares(snapshot, data) {
		log.Info().Int("shares", len(due)).Msg("Share links changed during expired share cleanup; checking again on the next pass")
		return
	}
	for i := range data.Shares {
		if slices.ContainsFunc(due, func(record shareRecord) bool { return record.ID == data.Shares[i].ID }) {
			data.Shares[i].DeleteOnExpiry = nil
		}
	}
	if err := s.saveLocked(data); err != nil {
		log.Error().Err(err).Int("shares", len(due)).Msg("Kept files of expired shares: could not save the share store")
		return
	}
	for _, plan := range plans {
		logger := log.With().
			Str("share", plan.record.ID).
			Str("owner", plan.record.CreatedBy).
			Str("path", shareVirtualPath(plan.record)).
			Logger()
		if err := s.removePlanned(plan); err != nil {
			logger.Warn().Err(err).Msg("Kept file of expired share")
			continue
		}
		logger.Info().Msg("Deleted file of expired share")
	}
}

func dueForExpiry(data *sharesData, now time.Time) []shareRecord {
	var due []shareRecord
	for _, record := range data.Shares {
		if record.DeleteOnExpiry != nil && !record.Revoked && isExpired(record.ExpiresAt, now) {
			due = append(due, record)
		}
	}
	return due
}

// sameShares reports whether two reads of the store hold the same links.
func sameShares(a, b *sharesData) bool {
	left, leftErr := json.Marshal(a.Shares)
	right, rightErr := json.Marshal(b.Shares)
	return leftErr == nil && rightErr == nil && bytes.Equal(left, right)
}

// planExpiredFile decides whether the file of one expired share may be
// removed: it must still be the exact file that was shared, and no other link
// may reach it.
func (s *shareService) planExpiredFile(record shareRecord, reaches []shareReach, mounts []hostMount) expiryPlan {
	plan := expiryPlan{record: record}
	if record.IsFolder || record.DeleteOnExpiry == nil {
		plan.kept = ErrInvalidOperation
		return plan
	}
	plan.identity = *record.DeleteOnExpiry
	// writeEntry rejects read-only and missing mounts and does not follow a
	// final symlink, so a file swapped for a link is kept.
	mount, entry, err := s.mounts.resolve(shareVirtualPath(record), writeEntry)
	if err != nil {
		plan.kept = err
		return plan
	}
	root, err := s.fs.EvalSymlinks(mount.Path)
	if err != nil {
		plan.kept = err
		return plan
	}
	plan.root = root
	plan.relPath = filepath.FromSlash(record.RelPath)
	// A parent folder replaced by a symlink would point the share elsewhere.
	if filepath.Join(root, plan.relPath) != entry {
		plan.kept = errExpiryFileChanged
		return plan
	}
	folders, err := s.folderKeysAbove(entry)
	if err != nil {
		plan.kept = err
		return plan
	}
	target := expiringTarget{
		virtualPath: shareVirtualPath(record),
		realPath:    entry,
		file:        fileKey{device: plan.identity.Device, inode: plan.identity.Inode},
		folders:     folders,
		shownAs:     mountedFoldersOf(mounts, entry),
	}
	for _, reach := range reaches {
		if reach.covers(target) {
			plan.kept = errExpiryFileShared
			return plan
		}
	}
	return plan
}

// removePlanned removes a planned file if it is still the shared file.
func (s *shareService) removePlanned(plan expiryPlan) error {
	if plan.kept != nil {
		return plan.kept
	}
	return s.fs.RemoveFileWithin(plan.root, plan.relPath, func(info fs.FileInfo) error {
		if !sameFile(plan.identity, info) {
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
// identity, so a link through either path is found. A folder that cannot be
// checked could be the one a link shares, so an error keeps the file.
func (s *shareService) folderKeysAbove(realPath string) ([]fileKey, error) {
	var keys []fileKey
	for folder := filepath.Dir(realPath); ; folder = filepath.Dir(folder) {
		info, err := s.fs.Stat(folder)
		if err != nil {
			return nil, err
		}
		if key := keyOf(info); key.known() {
			keys = append(keys, key)
		}
		if parent := filepath.Dir(folder); parent == folder {
			return keys, nil
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
	// shownAs names the file and its folders as the mount table would, so a
	// mount of any of them inside a folder link is found.
	shownAs []mountedFolder
}

// hostMount is one mount from the host's mount table.
type hostMount struct {
	// id and parent place the mount in the mount tree, which decides which of
	// several mounts at or above a path is visible there.
	id     string
	parent string
	path   string
	// device and root name what the mount shows: a folder (or file) of the
	// filesystem on device. Two mounts of one folder share both.
	device string
	root   string
}

// mountedFolder names a folder or file by the filesystem that holds it and its
// path inside that filesystem, as the mount table does.
type mountedFolder struct {
	device string
	path   string
}

// mountedFoldersOf names entry and each folder above it, up to the mount that
// holds them, as the mount table would. It reads only the table, so no mount
// is touched.
func mountedFoldersOf(mounts []hostMount, entry string) []mountedFolder {
	holder := visibleMountAt(mounts, entry)
	if holder == nil {
		return nil
	}
	var names []mountedFolder
	for current := entry; ; current = filepath.Dir(current) {
		relative, err := filepath.Rel(holder.path, current)
		if err != nil {
			return names
		}
		names = append(names, mountedFolder{device: holder.device, path: filepath.Join(holder.root, relative)})
		if current == holder.path {
			return names
		}
	}
}

// visibleMountAt returns the mount that shows path, or nil when the table has
// no root. It walks the mount tree from the root: at each mount, the child
// with the shortest mount point on the way to path is on top, since a mount
// below it would have been made on it instead. That skips mounts hidden by a
// later mount over one of their parent folders.
func visibleMountAt(mounts []hostMount, path string) *hostMount {
	var current *hostMount
	for i := range mounts {
		if mounts[i].path == "/" && !hasChildAt(mounts, mounts[i].id, "/") {
			current = &mounts[i]
		}
	}
	// Each step goes one mount deeper, so a well-formed table ends within its
	// own length; the bound stops a malformed table that loops.
	for range len(mounts) {
		if current == nil {
			return nil
		}
		var next *hostMount
		for i := range mounts {
			mount := &mounts[i]
			if mount.parent != current.id || mount.id == current.id || !pathWithinRoot(mount.path, path) {
				continue
			}
			if next == nil || len(mount.path) <= len(next.path) {
				next = mount
			}
		}
		if next == nil {
			return current
		}
		current = next
	}
	return nil
}

func hasChildAt(mounts []hostMount, parent string, path string) bool {
	for _, mount := range mounts {
		if mount.parent == parent && mount.id != parent && mount.path == path {
			return true
		}
	}
	return false
}

// contains reports whether the target file lies inside folder on the same
// filesystem, wherever each of them is mounted.
func (folder mountedFolder) contains(target expiringTarget) bool {
	if folder.device == "" || len(target.shownAs) == 0 {
		return false
	}
	file := target.shownAs[0]
	return file.device == folder.device && pathWithinRoot(folder.path, file.path)
}

// shareReach is the part of the mounts one active share exposes.
type shareReach struct {
	virtualPath string
	// realPath is empty when the target cannot be resolved now.
	realPath string
	// id is the resolved target's identity, zero when it is unknown.
	id fileKey
	// nested holds what each mount strictly inside a folder link's target
	// shows, since recipients can reach it through the link.
	nested []mountedFolder
	// self names a folder link's target as the mount table would, so a file
	// shown elsewhere by a bind mount of a folder inside it is found.
	self   mountedFolder
	folder bool
	// unknown is set when an I/O error hid the target; it protects every file.
	unknown bool
}

// shareLinks resolves the stored links for one cleanup pass. Each link is
// resolved at most once, however many expired shares are checked against it.
type shareLinks struct {
	service  *shareService
	records  []shareRecord
	mounts   []hostMount
	resolved map[int]shareReach
}

func newShareLinks(s *shareService, records []shareRecord, mounts []hostMount) *shareLinks {
	return &shareLinks{service: s, records: records, mounts: mounts, resolved: make(map[int]shareReach)}
}

// protecting returns the links active at the expired share's deadline or now.
// Checking the deadline keeps the result the same when the server was down at
// the deadline and another link expired in the meantime.
func (l *shareLinks) protecting(deadline, now time.Time) []shareReach {
	var reaches []shareReach
	for i, record := range l.records {
		if !activeAt(record, deadline) && !activeNow(record, now) {
			continue
		}
		reach, ok := l.resolved[i]
		if !ok {
			reach = l.service.resolveReach(record, l.mounts)
			l.resolved[i] = reach
		}
		reaches = append(reaches, reach)
	}
	return reaches
}

func (s *shareService) resolveReach(record shareRecord, mounts []hostMount) shareReach {
	reach := shareReach{virtualPath: shareVirtualPath(record), folder: record.IsFolder}
	_, realPath, err := s.mounts.resolve(reach.virtualPath, readExisting)
	switch {
	case err == nil:
		reach.realPath = realPath
		info, statErr := s.fs.Stat(realPath)
		if statErr != nil {
			// Without its identity the link might reach the file another way.
			reach.unknown = true
			break
		}
		reach.id = keyOf(info)
		if record.IsFolder {
			reach.nested = mountsInside(mounts, realPath)
			if names := mountedFoldersOf(mounts, realPath); len(names) > 0 {
				reach.self = names[0]
			}
		}
	case errors.Is(err, ErrMountPointNotFound), errors.Is(err, ErrPathNotFound), errors.Is(err, ErrPermissionDenied):
		// The share cannot serve anything now; its logical path still protects the file.
	default:
		reach.unknown = true
	}
	return reach
}

// mountsInside returns what each mount strictly inside folder shows. It reads
// only the mount table, so an offline mount cannot stall the pass.
func mountsInside(mounts []hostMount, folder string) []mountedFolder {
	var nested []mountedFolder
	for _, mount := range mounts {
		if mount.path == folder || !pathWithinRoot(folder, mount.path) {
			continue
		}
		// A hidden mount shows nothing inside the folder.
		if visible := visibleMountAt(mounts, mount.path); visible == nil || visible.id != mount.id {
			continue
		}
		nested = append(nested, mountedFolder{device: mount.device, path: mount.root})
	}
	return nested
}

// covers reports whether the active share can reach the expired share's file:
// by the same mount name and path, by the resolved real path, or by file and
// folder identity. The real path catches a configured and a discovered mount
// name for the same folder; identity catches bind mounts and hard links,
// including a folder mounted below a shared folder.
func (r shareReach) covers(target expiringTarget) bool {
	if r.unknown {
		return true
	}
	virtualPath := filepath.FromSlash(target.virtualPath)
	reachVirtual := filepath.FromSlash(r.virtualPath)
	if r.folder {
		return pathWithinRoot(reachVirtual, virtualPath) ||
			(r.realPath != "" && pathWithinRoot(r.realPath, target.realPath)) ||
			(r.id.known() && slices.Contains(target.folders, r.id)) ||
			slices.ContainsFunc(r.nested, func(m mountedFolder) bool { return slices.Contains(target.shownAs, m) }) ||
			r.self.contains(target)
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

// activeNow reports whether a stored link is usable now. It has no creation
// cutoff: now is read before the pass takes the lock, so a link saved while
// the pass waited was created after it and must still count.
func activeNow(record shareRecord, now time.Time) bool {
	return !record.Revoked && !isExpired(record.ExpiresAt, now)
}

func shareVirtualPath(record shareRecord) string {
	return record.MountName + "/" + filepath.ToSlash(record.RelPath)
}
