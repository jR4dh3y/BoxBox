package service

import (
	"context"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// afterExpiry is a cleanup time past the one-hour expiry createExpiringShare uses.
func afterExpiry() time.Time { return time.Now().Add(2 * time.Hour) }

func newExpiryTestService(fsys filesystem.FS, mounts func() []model.MountPoint) *shareService {
	shares := newShareTestService(fsys, mounts).(*shareService)
	shares.readMounts = rootOnlyMountTable
	return shares
}

// rootOnlyMountTable keeps the tests independent of the host's mounts.
func rootOnlyMountTable() ([]hostMount, error) {
	return []hostMount{{id: "1", parent: "0", path: "/", device: "0:1", root: "/"}}, nil
}

func createExpiringShare(t *testing.T, shares *shareService, path string, deleteOnExpiry bool) *model.Share {
	t.Helper()
	share, err := shares.Create(context.Background(), "owner", path, ShareSettings{DeleteOnExpiry: deleteOnExpiry}, time.Now().Add(time.Hour))
	if err != nil {
		t.Fatalf("Create(%q) = %v", path, err)
	}
	return share
}

func assertFileExists(t *testing.T, fsys filesystem.FS, path string, want bool) {
	t.Helper()
	exists, err := fsys.Exists(path)
	if err != nil {
		t.Fatal(err)
	}
	if exists != want {
		t.Fatalf("%s exists = %v, want %v", path, exists, want)
	}
}

func storedDeleteOnExpiry(t *testing.T, shares *shareService, id string) bool {
	t.Helper()
	shares.mu.RLock()
	defer shares.mu.RUnlock()
	for _, record := range shares.loadLocked().Shares {
		if record.ID == id {
			return record.DeleteOnExpiry != nil
		}
	}
	t.Fatalf("share %s is not stored", id)
	return false
}

func TestShareExpiryDeletesSharedFileOnce(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	share := createExpiringShare(t, shares, "media/file.txt", true)
	if share.DeleteOnExpiry == nil {
		t.Fatal("created share does not report delete on expiry")
	}

	shares.deleteExpiredFiles(context.Background(), time.Now())
	assertFileExists(t, fsys, "/data/media/file.txt", true)

	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", false)
	if storedDeleteOnExpiry(t, shares, share.ID) {
		t.Fatal("deletion request was not consumed")
	}

	// A later file at the same path is not the shared file.
	if err := fsys.WriteFile("/data/media/file.txt", []byte("shared content"), 0o644); err != nil {
		t.Fatal(err)
	}
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
}

func TestShareExpiryCreateRejectsUnsafeTargets(t *testing.T) {
	tests := []struct {
		name      string
		path      string
		expiresAt time.Time
		wantErr   error
	}{
		{name: "folder", path: "media/subdir", expiresAt: time.Now().Add(time.Hour), wantErr: ErrInvalidOperation},
		{name: "no expiry", path: "media/file.txt", wantErr: ErrInvalidOperation},
		{name: "past expiry", path: "media/file.txt", expiresAt: time.Now().Add(-time.Minute), wantErr: ErrInvalidOperation},
		{name: "read-only mount", path: "archive/file.txt", expiresAt: time.Now().Add(time.Hour), wantErr: ErrPermissionDenied},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			setupShareTestFS(fsys)
			shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
			_, err := shares.Create(context.Background(), "owner", test.path, ShareSettings{DeleteOnExpiry: true}, test.expiresAt)
			if !errors.Is(err, test.wantErr) {
				t.Fatalf("Create() error = %v, want %v", err, test.wantErr)
			}
		})
	}
}

func TestShareExpiryKeepsFileReachedByAnotherActiveShare(t *testing.T) {
	tests := []struct {
		name   string
		mounts []model.MountPoint
		other  string
	}{
		{name: "same file", other: "media/file.txt"},
		{name: "mount root folder", other: "media"},
		{
			name:   "same file through another mount name",
			mounts: []model.MountPoint{{Name: "media", Path: "/data/media"}, {Name: "media-alias", Path: "/data/media"}},
			other:  "media-alias/file.txt",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			setupShareTestFS(fsys)
			mounts := test.mounts
			if mounts == nil {
				mounts = shareTestMounts()
			}
			shares := newExpiryTestService(fsys, fixedMounts(mounts))
			expiring := createExpiringShare(t, shares, "media/file.txt", true)
			other, err := shares.Create(context.Background(), "someone-else", test.other, ShareSettings{}, time.Time{})
			if err != nil {
				t.Fatal(err)
			}

			shares.deleteExpiredFiles(context.Background(), afterExpiry())
			assertFileExists(t, fsys, "/data/media/file.txt", true)
			if storedDeleteOnExpiry(t, shares, expiring.ID) {
				t.Fatal("deletion request was not consumed")
			}

			// The one attempt was made at expiry; revoking the other link later
			// must not delete a file whose link expired long ago.
			if err := shares.Revoke("someone-else", other.ID); err != nil {
				t.Fatal(err)
			}
			shares.deleteExpiredFiles(context.Background(), afterExpiry())
			assertFileExists(t, fsys, "/data/media/file.txt", true)
		})
	}
}

func TestShareExpiryIgnoresSharesThatCannotReachTheFile(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	_ = fsys.MkdirAll("/data/media/sub-old", 0o755)
	_ = fsys.WriteFile("/data/media/sub-old/file.txt", []byte("old"), 0o644)
	_ = fsys.MkdirAll("/data/media/sub", 0o755)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	createExpiringShare(t, shares, "media/sub-old/file.txt", true)
	// A folder whose name is a prefix of the file's folder does not contain it.
	if _, err := shares.Create(context.Background(), "owner", "media/sub", ShareSettings{}, time.Time{}); err != nil {
		t.Fatal(err)
	}
	// Shares of the same file that ended before the deadline do not protect it.
	if _, err := shares.Create(context.Background(), "owner", "media/sub-old/file.txt", ShareSettings{}, time.Now().Add(30*time.Minute)); err != nil {
		t.Fatal(err)
	}
	revoked, err := shares.Create(context.Background(), "owner", "media/sub-old/file.txt", ShareSettings{}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if err := shares.Revoke("owner", revoked.ID); err != nil {
		t.Fatal(err)
	}

	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/sub-old/file.txt", false)
}

// The result must not depend on whether the server was running at the deadline.
func TestShareExpiryKeepsFileSharedAtTheDeadlineAfterDowntime(t *testing.T) {
	tests := []struct {
		name  string
		other func(t *testing.T, shares *shareService, deadline time.Time) func(now time.Time)
	}{
		{"link that expired before the late pass", func(t *testing.T, shares *shareService, deadline time.Time) func(time.Time) {
			if _, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{}, deadline.Add(time.Minute)); err != nil {
				t.Fatal(err)
			}
			return func(time.Time) {}
		}},
		{"link revoked after the deadline", func(t *testing.T, shares *shareService, deadline time.Time) func(time.Time) {
			other, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{}, time.Time{})
			if err != nil {
				t.Fatal(err)
			}
			return func(now time.Time) {
				shares.mu.Lock()
				defer shares.mu.Unlock()
				data := shares.loadLocked()
				for i := range data.Shares {
					if data.Shares[i].ID == other.ID {
						data.Shares[i].Revoked = true
						data.Shares[i].RevokedAt = now
					}
				}
				if err := shares.saveLocked(data); err != nil {
					t.Fatal(err)
				}
			}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			setupShareTestFS(fsys)
			shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
			expiring := createExpiringShare(t, shares, "media/file.txt", true)
			test.other(t, shares, expiring.ExpiresAt)(expiring.ExpiresAt.Add(30 * time.Minute))

			shares.deleteExpiredFiles(context.Background(), afterExpiry())
			assertFileExists(t, fsys, "/data/media/file.txt", true)
		})
	}
}

func TestShareExpiryRevokeNeverDeletes(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	share := createExpiringShare(t, shares, "media/file.txt", true)

	if err := shares.Revoke("owner", share.ID); err != nil {
		t.Fatal(err)
	}
	if storedDeleteOnExpiry(t, shares, share.ID) {
		t.Fatal("revoke kept the deletion request")
	}
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
}

func TestShareExpiryKeepsChangedOrUnavailableFile(t *testing.T) {
	tests := []struct {
		name   string
		change func(t *testing.T, fsys filesystem.FS, mounts *[]model.MountPoint)
		// survivor is a file that must still exist after the pass.
		survivor string
	}{
		{
			name: "content changed",
			change: func(t *testing.T, fsys filesystem.FS, _ *[]model.MountPoint) {
				if err := fsys.WriteFile("/data/media/file.txt", []byte("edited after sharing"), 0o644); err != nil {
					t.Fatal(err)
				}
			},
			survivor: "/data/media/file.txt",
		},
		{
			name: "renamed away",
			change: func(t *testing.T, fsys filesystem.FS, _ *[]model.MountPoint) {
				if err := fsys.Rename("/data/media/file.txt", "/data/media/renamed.txt"); err != nil {
					t.Fatal(err)
				}
			},
			survivor: "/data/media/renamed.txt",
		},
		{
			name: "replaced by another file",
			change: func(t *testing.T, fsys filesystem.FS, _ *[]model.MountPoint) {
				if err := fsys.Rename("/data/media/file.txt", "/data/media/original.txt"); err != nil {
					t.Fatal(err)
				}
				if err := fsys.WriteFile("/data/media/file.txt", []byte("a different file"), 0o644); err != nil {
					t.Fatal(err)
				}
			},
			survivor: "/data/media/file.txt",
		},
		{
			name: "mount became read-only",
			change: func(_ *testing.T, _ filesystem.FS, mounts *[]model.MountPoint) {
				*mounts = []model.MountPoint{{Name: "media", Path: "/data/media", ReadOnly: true}}
			},
			survivor: "/data/media/file.txt",
		},
		{
			name: "mount removed",
			change: func(_ *testing.T, _ filesystem.FS, mounts *[]model.MountPoint) {
				*mounts = nil
			},
			survivor: "/data/media/file.txt",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			setupShareTestFS(fsys)
			mounts := shareTestMounts()
			shares := newExpiryTestService(fsys, func() []model.MountPoint { return mounts })
			share := createExpiringShare(t, shares, "media/file.txt", true)

			test.change(t, fsys, &mounts)
			shares.deleteExpiredFiles(context.Background(), afterExpiry())
			assertFileExists(t, fsys, test.survivor, true)
			if storedDeleteOnExpiry(t, shares, share.ID) {
				t.Fatal("deletion request was not consumed")
			}
		})
	}
}

func TestShareExpiryKeepsFilesOfLegacyShares(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	legacy := `{"shares":[{"id":"legacy","token":"legacy-token","mountName":"media","relPath":"file.txt",` +
		`"isFolder":false,"permissions":{"view":true,"download":true},"fileName":"file.txt",` +
		`"createdAt":"2020-01-01T00:00:00Z","expiresAt":"2020-01-02T00:00:00Z","revoked":false,"createdBy":"owner"}]}`
	if err := fsys.WriteFile("/data/shares.json", []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))

	shares.deleteExpiredFiles(context.Background(), time.Now())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
}

// failingStoreFS fails to publish the share store, as when its disk is full.
type failingStoreFS struct {
	filesystem.FS
	fail bool
}

func (f *failingStoreFS) Rename(oldpath, newpath string) error {
	if f.fail && strings.Contains(oldpath, "shares.json.tmp.") {
		return errors.New("disk full")
	}
	return f.FS.Rename(oldpath, newpath)
}

func TestShareExpiryKeepsFileWhenTheStoreCannotBeSaved(t *testing.T) {
	backing := filesystem.NewMemMapFS()
	setupShareTestFS(backing)
	fsys := &failingStoreFS{FS: backing}
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	createExpiringShare(t, shares, "media/file.txt", true)

	fsys.fail = true
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, backing, "/data/media/file.txt", true)
}

func newDiskExpiryTestService(t *testing.T) (*shareService, string) {
	t.Helper()
	root := t.TempDir()
	media := filepath.Join(root, "media")
	if err := os.MkdirAll(media, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(media, "file.txt"), []byte("shared content"), 0o644); err != nil {
		t.Fatal(err)
	}
	shares := NewShareService(filesystem.NewOsFS(), ShareServiceConfig{
		DataDir: filepath.Join(root, "data"),
		Mounts:  fixedMounts([]model.MountPoint{{Name: "media", Path: media}}),
	}).(*shareService)
	shares.readMounts = rootOnlyMountTable
	return shares, media
}

func assertDiskFileExists(t *testing.T, path string, want bool) {
	t.Helper()
	_, err := os.Lstat(path)
	if exists := err == nil; exists != want {
		t.Fatalf("%s exists = %v (err %v), want %v", path, exists, err, want)
	}
}

func TestShareExpiryDeletesFileOnDisk(t *testing.T) {
	shares, media := newDiskExpiryTestService(t)
	createExpiringShare(t, shares, "media/file.txt", true)

	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertDiskFileExists(t, filepath.Join(media, "file.txt"), false)
	assertDiskFileExists(t, media, true)
}

func TestShareExpiryKeepsSameSizeSameTimeReplacement(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Windows has no persisted file ID; size and modification time identify the file")
	}
	shares, media := newDiskExpiryTestService(t)
	target := filepath.Join(media, "file.txt")
	createExpiringShare(t, shares, "media/file.txt", true)
	info, err := os.Stat(target)
	if err != nil {
		t.Fatal(err)
	}

	// A restored copy with the same size and time is still a different file.
	replacement := filepath.Join(media, "replacement.tmp")
	if err := os.WriteFile(replacement, []byte("shared content"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(replacement, info.ModTime(), info.ModTime()); err != nil {
		t.Fatal(err)
	}
	if err := os.Rename(replacement, target); err != nil {
		t.Fatal(err)
	}

	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertDiskFileExists(t, target, true)
}

func TestShareExpiryRejectsSelectedSymlink(t *testing.T) {
	shares, media := newDiskExpiryTestService(t)
	if err := os.Symlink(filepath.Join(media, "file.txt"), filepath.Join(media, "link.txt")); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}

	_, err := shares.Create(context.Background(), "owner", "media/link.txt", ShareSettings{DeleteOnExpiry: true}, time.Now().Add(time.Hour))
	if !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("Create() on a symlink error = %v, want %v", err, ErrInvalidOperation)
	}
}

func TestShareExpiryKeepsFileSwappedForSymlink(t *testing.T) {
	shares, media := newDiskExpiryTestService(t)
	target := filepath.Join(media, "file.txt")
	createExpiringShare(t, shares, "media/file.txt", true)

	other := filepath.Join(media, "other.txt")
	if err := os.Rename(target, other); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(other, target); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}

	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertDiskFileExists(t, target, true)
	assertDiskFileExists(t, other, true)
}

func TestShareExpiryKeepsFileReachedThroughHardLink(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Windows has no persisted file ID")
	}
	shares, media := newDiskExpiryTestService(t)
	if err := os.Link(filepath.Join(media, "file.txt"), filepath.Join(media, "alias.txt")); err != nil {
		t.Skipf("hard links unavailable: %v", err)
	}
	createExpiringShare(t, shares, "media/file.txt", true)
	if _, err := shares.Create(context.Background(), "owner", "media/alias.txt", ShareSettings{}, time.Time{}); err != nil {
		t.Fatal(err)
	}

	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertDiskFileExists(t, filepath.Join(media, "file.txt"), true)
}

func TestShareReachCoversFolderByIdentity(t *testing.T) {
	// A folder bind-mounted at two paths: neither the mount path nor the real
	// path matches, but the folder identity does.
	folder := fileKey{device: 1, inode: 10}
	target := expiringTarget{
		virtualPath: "copy/photos/a.jpg",
		realPath:    "/mnt/copy/photos/a.jpg",
		file:        fileKey{device: 1, inode: 11},
		folders:     []fileKey{folder, {device: 1, inode: 2}},
	}
	tests := []struct {
		name  string
		reach shareReach
		want  bool
	}{
		{"same folder", shareReach{virtualPath: "media/photos", realPath: "/srv/media/photos", id: folder, folder: true}, true},
		{"same file", shareReach{virtualPath: "media/photos/a.jpg", realPath: "/srv/media/photos/a.jpg", id: target.file}, true},
		{"file identity is not a folder", shareReach{virtualPath: "media/photos", realPath: "/srv/media/photos", id: target.file, folder: true}, false},
		{"other folder", shareReach{virtualPath: "media/music", realPath: "/srv/media/music", id: fileKey{device: 1, inode: 12}, folder: true}, false},
		{"unknown identity", shareReach{virtualPath: "media/photos", realPath: "/srv/media/photos", folder: true}, false},
	}
	for _, test := range tests {
		if got := test.reach.covers(target); got != test.want {
			t.Errorf("%s: covers() = %v, want %v", test.name, got, test.want)
		}
	}
}

// rereadFailingFS fails every read of the share store after the first once armed,
// so cleanup reads its snapshot and then cannot read the store again.
type rereadFailingFS struct {
	filesystem.FS
	armed bool
	reads int
}

func (f *rereadFailingFS) ReadFile(name string) ([]byte, error) {
	if f.armed && strings.HasSuffix(name, "shares.json") {
		f.reads++
		if f.reads > 1 {
			return nil, errors.New("I/O error")
		}
	}
	return f.FS.ReadFile(name)
}

func TestShareExpiryKeepsFileWhenTheStoreCannotBeReread(t *testing.T) {
	backing := filesystem.NewMemMapFS()
	setupShareTestFS(backing)
	fsys := &rereadFailingFS{FS: backing}
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	createExpiringShare(t, shares, "media/file.txt", true)

	fsys.armed = true
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, backing, "/data/media/file.txt", true)
}

func TestShareExpiryKeepsFileForLinkSavedWhileCleanupWaited(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	createExpiringShare(t, shares, "media/file.txt", true)
	later, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	// The pass read its time, then waited for the lock while Create saved.
	now := afterExpiry()
	shares.mu.Lock()
	data := shares.loadLocked()
	for i := range data.Shares {
		if data.Shares[i].ID == later.ID {
			data.Shares[i].CreatedAt = now.Add(time.Second)
		}
	}
	if err := shares.saveLocked(data); err != nil {
		t.Fatal(err)
	}
	shares.mu.Unlock()

	shares.deleteExpiredFiles(context.Background(), now)
	assertFileExists(t, fsys, "/data/media/file.txt", true)
}

// statFailFS fails Stat for one path once armed, as a transient I/O error would.
type statFailFS struct {
	filesystem.FS
	path  string
	armed bool
}

func (f *statFailFS) Stat(name string) (os.FileInfo, error) {
	if f.armed && filepath.Clean(name) == f.path {
		return nil, errors.New("input/output error")
	}
	return f.FS.Stat(name)
}

func TestShareExpiryKeepsFileWhenAnIdentityCheckFails(t *testing.T) {
	tests := []struct {
		name     string
		failPath string
	}{
		// The other link resolves, but its target cannot be identified.
		{name: "other link target", failPath: "/data/archive"},
		// A folder above the file cannot be identified.
		{name: "folder above the file", failPath: "/data"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			mem := filesystem.NewMemMapFS()
			setupShareTestFS(mem)
			fsys := &statFailFS{FS: mem, path: test.failPath}
			shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
			createExpiringShare(t, shares, "media/file.txt", true)
			if _, err := shares.Create(context.Background(), "owner", "archive", ShareSettings{}, time.Time{}); err != nil {
				t.Fatal(err)
			}

			fsys.armed = true
			shares.deleteExpiredFiles(context.Background(), afterExpiry())
			assertFileExists(t, mem, "/data/media/file.txt", true)
		})
	}
}

func TestShareExpiryStopWaitsForARunningPass(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))

	// Hold the lock so the first pass blocks inside the cleanup goroutine.
	shares.mu.Lock()
	ctx, cancel := context.WithCancel(context.Background())
	shares.StartExpiryCleanup(ctx)
	cancel()
	stopped := make(chan struct{})
	go func() {
		_ = shares.StopExpiryCleanup(context.Background())
		close(stopped)
	}()
	select {
	case <-stopped:
		t.Fatal("StopExpiryCleanup returned while a pass was running")
	case <-time.After(50 * time.Millisecond):
	}
	shares.mu.Unlock()
	select {
	case <-stopped:
	case <-time.After(5 * time.Second):
		t.Fatal("StopExpiryCleanup did not return after the pass finished")
	}
}

func TestShareReachCoversFolderMountedBelowSharedFolder(t *testing.T) {
	mounts := []hostMount{
		{id: "1", parent: "0", path: "/", device: "8:1", root: "/"},
		{id: "2", parent: "1", path: "/srv", device: "8:2", root: "/"},
		// /srv/files/photos is also mounted at /srv/shared/photos.
		{id: "3", parent: "2", path: "/srv/shared/photos", device: "8:2", root: "/files/photos"},
		{id: "4", parent: "2", path: "/srv/shared/music", device: "8:2", root: "/files/music"},
		// A single file bind-mounted inside the shared folder.
		{id: "5", parent: "2", path: "/srv/shared/a.jpg", device: "8:2", root: "/files/photos/a.jpg"},
	}
	target := expiringTarget{
		virtualPath: "files/photos/a.jpg",
		realPath:    "/srv/files/photos/a.jpg",
		shownAs:     mountedFoldersOf(mounts, "/srv/files/photos/a.jpg"),
	}
	tests := []struct {
		name   string
		folder string
		want   bool
	}{
		{"folder mount of the file's folder", "/srv/shared", true},
		{"unrelated folder", "/srv/other", false},
		{"mount at the link's own folder is not inside it", "/srv/shared/music", false},
	}
	for _, test := range tests {
		reach := shareReach{virtualPath: "x", realPath: test.folder, folder: true, nested: mountsInside(mounts[:4], test.folder)}
		if got := reach.covers(target); got != test.want {
			t.Errorf("%s: covers() = %v, want %v", test.name, got, test.want)
		}
	}
	fileMount := shareReach{virtualPath: "x", realPath: "/srv/shared", folder: true, nested: mountsInside(append(mounts[:2:2], mounts[4]), "/srv/shared")}
	if !fileMount.covers(target) {
		t.Error("covers() with the file bind-mounted inside the folder = false")
	}
}

func TestShareExpiryKeepsFileReachedThroughMountBelowSharedFolder(t *testing.T) {
	shares, media := newDiskExpiryTestService(t)
	media, err := filepath.EvalSymlinks(media)
	if err != nil {
		t.Fatal(err)
	}
	photos := filepath.Join(media, "files", "photos")
	if err := os.MkdirAll(photos, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(photos, "a.jpg"), []byte("photo"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(media, "shared", "photos"), 0o755); err != nil {
		t.Fatal(err)
	}
	expiring := createExpiringShare(t, shares, "media/files/photos/a.jpg", true)
	if _, err := shares.Create(context.Background(), "owner", "media/shared", ShareSettings{}, time.Time{}); err != nil {
		t.Fatal(err)
	}
	var record shareRecord
	for _, stored := range shares.loadLocked().Shares {
		if stored.ID == expiring.ID {
			record = stored
		}
	}
	now := afterExpiry()
	// The mount table says files/photos is also mounted at shared/photos.
	mounts := []hostMount{
		{id: "1", parent: "0", path: "/", device: "0:1", root: "/"},
		{id: "2", parent: "1", path: media, device: "0:99", root: "/"},
		{id: "3", parent: "2", path: filepath.Join(media, "shared", "photos"), device: "0:99", root: "/files/photos"},
	}

	links := newShareLinks(shares, shares.loadLocked().Shares, mounts)
	if err := shares.removePlanned(shares.planExpiredFile(record, links.protecting(record.ExpiresAt, now), mounts)); !errors.Is(err, errExpiryFileShared) {
		t.Fatalf("removal with the mount = %v, want %v", err, errExpiryFileShared)
	}
	assertDiskFileExists(t, filepath.Join(photos, "a.jpg"), true)

	// Without the nested mount, nothing else reaches the file.
	mounts = mounts[:2]
	links = newShareLinks(shares, shares.loadLocked().Shares, mounts)
	if err := shares.removePlanned(shares.planExpiredFile(record, links.protecting(record.ExpiresAt, now), mounts)); err != nil {
		t.Fatalf("removal without the mount = %v", err)
	}
	assertDiskFileExists(t, filepath.Join(photos, "a.jpg"), false)
}

func TestVisibleMountAtSkipsHiddenMounts(t *testing.T) {
	mounts := []hostMount{
		{id: "1", parent: "0", path: "/", device: "8:1", root: "/"},
		{id: "2", parent: "1", path: "/files", device: "8:2", root: "/"},
		{id: "3", parent: "2", path: "/files/photos", device: "8:2", root: "/photos"},
		// A later mount over /files hides mount 3.
		{id: "4", parent: "2", path: "/files", device: "8:3", root: "/"},
		{id: "5", parent: "1", path: "/shared", device: "8:4", root: "/"},
		// The visible /files/photos is also mounted at /shared/photos.
		{id: "6", parent: "5", path: "/shared/photos", device: "8:3", root: "/photos"},
	}
	if got := visibleMountAt(mounts, "/files/photos/a.jpg"); got == nil || got.id != "4" {
		t.Fatalf("visibleMountAt() = %+v, want mount 4", got)
	}
	if got := visibleMountAt(mounts, "/elsewhere"); got == nil || got.id != "1" {
		t.Fatalf("visibleMountAt() = %+v, want the root mount", got)
	}
	target := expiringTarget{
		virtualPath: "files/photos/a.jpg",
		realPath:    "/files/photos/a.jpg",
		shownAs:     mountedFoldersOf(mounts, "/files/photos/a.jpg"),
	}
	shared := shareReach{virtualPath: "shared", realPath: "/shared", folder: true, nested: mountsInside(mounts, "/shared")}
	if !shared.covers(target) {
		t.Fatal("covers() = false for the visible folder mounted inside the shared folder")
	}
	// Mount 3 is hidden, so it shows nothing inside /files.
	if nested := mountsInside(mounts, "/"); slices.Contains(nested, mountedFolder{device: "8:2", path: "/photos"}) {
		t.Fatalf("mountsInside() = %+v, includes the hidden mount", nested)
	}
}

func TestVisibleMountAtStopsOnALoopingTable(t *testing.T) {
	mounts := []hostMount{
		{id: "1", parent: "0", path: "/", device: "8:1", root: "/"},
		{id: "2", parent: "3", path: "/a", device: "8:2", root: "/"},
		{id: "3", parent: "2", path: "/a", device: "8:2", root: "/"},
	}
	visibleMountAt(mounts, "/a/b")
}

func TestShareExpiryChecksRunWithoutTheLockAndRetryWhenLinksChange(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	_ = fsys.MkdirAll("/data/media/other", 0o755)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	expiring := createExpiringShare(t, shares, "media/file.txt", true)

	shares.readMounts = func() ([]hostMount, error) {
		if !shares.mu.TryLock() {
			t.Error("the share lock is held while links are checked")
		} else {
			shares.mu.Unlock()
		}
		// A link created during the checks changes the store.
		if _, err := shares.Create(context.Background(), "owner", "media/other", ShareSettings{}, time.Time{}); err != nil {
			t.Fatal(err)
		}
		return rootOnlyMountTable()
	}
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
	if !storedDeleteOnExpiry(t, shares, expiring.ID) {
		t.Fatal("a pass that saw the store change consumed the deletion request")
	}

	shares.readMounts = rootOnlyMountTable
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", false)
}

func TestShareExpiryTouchesNothingAfterCancellation(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	expiring := createExpiringShare(t, shares, "media/file.txt", true)

	ctx, cancel := context.WithCancel(context.Background())
	shares.readMounts = func() ([]hostMount, error) {
		cancel()
		return rootOnlyMountTable()
	}
	shares.deleteExpiredFiles(ctx, afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
	if !storedDeleteOnExpiry(t, shares, expiring.ID) {
		t.Fatal("a cancelled pass consumed the deletion request")
	}
}

func TestShareExpiryKeepsFileWhenTheMountTableCannotBeRead(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	expiring := createExpiringShare(t, shares, "media/file.txt", true)

	shares.readMounts = func() ([]hostMount, error) { return nil, errors.New("no /proc") }
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
	if !storedDeleteOnExpiry(t, shares, expiring.ID) {
		t.Fatal("a pass without a mount table consumed the deletion request")
	}
}

func TestShareReachCoversFileShownByABindMountOfItsFolder(t *testing.T) {
	mounts := []hostMount{
		{id: "1", parent: "0", path: "/", device: "8:1", root: "/"},
		// /vault/photos is also mounted at /copies/photos.
		{id: "2", parent: "1", path: "/copies/photos", device: "8:1", root: "/vault/photos"},
	}
	target := expiringTarget{
		virtualPath: "copies/photos/a.jpg",
		realPath:    "/copies/photos/a.jpg",
		shownAs:     mountedFoldersOf(mounts, "/copies/photos/a.jpg"),
	}
	reach := func(folder string) shareReach {
		return shareReach{virtualPath: "x", realPath: folder, folder: true, self: mountedFoldersOf(mounts, folder)[0]}
	}
	if !reach("/vault").covers(target) {
		t.Fatal("covers() = false for a link to the folder the bind mount shows")
	}
	if reach("/elsewhere").covers(target) {
		t.Fatal("covers() = true for an unrelated folder")
	}
}

// replacingFS replaces the target file after the first Lstat of it, as a save
// that lands while Create waits for the lock.
type replacingFS struct {
	filesystem.FS
	target string
	lstats int
}

func (f *replacingFS) Lstat(name string) (fs.FileInfo, error) {
	info, err := f.FS.Lstat(name)
	if name == f.target {
		f.lstats++
		if f.lstats == 1 {
			// Remove first: an in-memory FileInfo reads the file's live data.
			if removeErr := f.FS.Remove(name); removeErr != nil {
				return nil, removeErr
			}
			if writeErr := f.FS.WriteFile(name, []byte("a different, longer file"), 0o644); writeErr != nil {
				return nil, writeErr
			}
		}
	}
	return info, err
}

func TestShareExpiryCreateRejectsAFileReplacedBeforeSaving(t *testing.T) {
	backing := filesystem.NewMemMapFS()
	setupShareTestFS(backing)
	fsys := &replacingFS{FS: backing, target: "/data/media/file.txt"}
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))

	_, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{DeleteOnExpiry: true}, time.Now().Add(time.Hour))
	if !errors.Is(err, ErrShareTargetChanged) {
		t.Fatalf("Create() error = %v, want %v", err, ErrShareTargetChanged)
	}
}

func TestShareExpiryStopGivesUpWhenItsContextEnds(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))

	// Hold the lock so the pass stays blocked, as on an unresponsive mount.
	shares.mu.Lock()
	defer shares.mu.Unlock()
	ctx, cancel := context.WithCancel(context.Background())
	shares.StartExpiryCleanup(ctx)
	cancel()
	wait, stop := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer stop()
	if err := shares.StopExpiryCleanup(wait); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("StopExpiryCleanup() = %v, want %v", err, context.DeadlineExceeded)
	}
}

func TestShareExpiryKeepsFileWhenTheMountTableHasNoRoot(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	expiring := createExpiringShare(t, shares, "media/file.txt", true)

	// A chroot that is not a mount point lists child mounts only.
	shares.readMounts = func() ([]hostMount, error) {
		return []hostMount{{id: "30", parent: "20", path: "/shared/photos", device: "8:1", root: "/files/photos"}}, nil
	}
	shares.deleteExpiredFiles(context.Background(), afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
	if !storedDeleteOnExpiry(t, shares, expiring.ID) {
		t.Fatal("a pass without a root mount consumed the deletion request")
	}
}

func TestShareExpiryTouchesNothingWhenCancelledWhileWaitingForTheLock(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	expiring := createExpiringShare(t, shares, "media/file.txt", true)

	ctx, cancel := context.WithCancel(context.Background())
	shares.readMounts = func() ([]hostMount, error) {
		// Hold the lock past the pass's last check before it locks, then
		// cancel while it waits.
		shares.mu.Lock()
		go func() {
			time.Sleep(30 * time.Millisecond)
			cancel()
			shares.mu.Unlock()
		}()
		return rootOnlyMountTable()
	}
	shares.deleteExpiredFiles(ctx, afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", true)
	if !storedDeleteOnExpiry(t, shares, expiring.ID) {
		t.Fatal("a pass cancelled while waiting for the lock consumed the deletion request")
	}
}

// blockingRemoveFS blocks a removal until released, as a stalled drive.
type blockingRemoveFS struct {
	filesystem.FS
	entered chan struct{}
	release chan struct{}
}

func (f *blockingRemoveFS) RemoveFileWithin(root, path string, verify func(fs.FileInfo) error) error {
	close(f.entered)
	<-f.release
	return f.FS.RemoveFileWithin(root, path, verify)
}

func TestShareExpiryStopWaitsForARemovalInProgress(t *testing.T) {
	backing := filesystem.NewMemMapFS()
	setupShareTestFS(backing)
	fsys := &blockingRemoveFS{FS: backing, entered: make(chan struct{}), release: make(chan struct{})}
	shares := newExpiryTestService(fsys, fixedMounts(shareTestMounts()))
	createExpiringShare(t, shares, "media/file.txt", true)

	go shares.deleteExpiredFiles(context.Background(), afterExpiry())
	<-fsys.entered
	shares.cleanup.Add(1) // stands in for the running cleanup goroutine
	wait, stop := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer stop()
	stopped := make(chan error, 1)
	go func() { stopped <- shares.StopExpiryCleanup(wait) }()
	select {
	case err := <-stopped:
		t.Fatalf("StopExpiryCleanup() = %v while a removal was in progress", err)
	case <-time.After(100 * time.Millisecond):
	}
	close(fsys.release)
	select {
	case err := <-stopped:
		if !errors.Is(err, context.DeadlineExceeded) {
			t.Fatalf("StopExpiryCleanup() = %v, want %v", err, context.DeadlineExceeded)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("StopExpiryCleanup did not return after the removal finished")
	}
	shares.cleanup.Done()
	assertFileExists(t, backing, "/data/media/file.txt", false)
}
