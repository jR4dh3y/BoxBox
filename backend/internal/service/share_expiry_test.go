package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// afterExpiry is a cleanup time past the one-hour expiry createExpiringShare uses.
func afterExpiry() time.Time { return time.Now().Add(2 * time.Hour) }

func newExpiryTestService(fsys filesystem.FS, mounts func() []model.MountPoint) *shareService {
	return newShareTestService(fsys, mounts).(*shareService)
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

	shares.deleteExpiredFiles(time.Now())
	assertFileExists(t, fsys, "/data/media/file.txt", true)

	shares.deleteExpiredFiles(afterExpiry())
	assertFileExists(t, fsys, "/data/media/file.txt", false)
	if storedDeleteOnExpiry(t, shares, share.ID) {
		t.Fatal("deletion request was not consumed")
	}

	// A later file at the same path is not the shared file.
	if err := fsys.WriteFile("/data/media/file.txt", []byte("shared content"), 0o644); err != nil {
		t.Fatal(err)
	}
	shares.deleteExpiredFiles(afterExpiry())
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

			shares.deleteExpiredFiles(afterExpiry())
			assertFileExists(t, fsys, "/data/media/file.txt", true)
			if storedDeleteOnExpiry(t, shares, expiring.ID) {
				t.Fatal("deletion request was not consumed")
			}

			// The one attempt was made at expiry; revoking the other link later
			// must not delete a file whose link expired long ago.
			if err := shares.Revoke("someone-else", other.ID); err != nil {
				t.Fatal(err)
			}
			shares.deleteExpiredFiles(afterExpiry())
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

	shares.deleteExpiredFiles(afterExpiry())
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

			shares.deleteExpiredFiles(afterExpiry())
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
	shares.deleteExpiredFiles(afterExpiry())
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
			shares.deleteExpiredFiles(afterExpiry())
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

	shares.deleteExpiredFiles(time.Now())
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
	shares.deleteExpiredFiles(afterExpiry())
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
	})
	return shares.(*shareService), media
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

	shares.deleteExpiredFiles(afterExpiry())
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

	shares.deleteExpiredFiles(afterExpiry())
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

	shares.deleteExpiredFiles(afterExpiry())
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

	shares.deleteExpiredFiles(afterExpiry())
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
