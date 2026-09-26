package service

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
)

func setupShareTestFS(fsys filesystem.FS) {
	_ = fsys.MkdirAll("/data/media/subdir", 0o755)
	_ = fsys.MkdirAll("/data/archive", 0o755)
	_ = fsys.WriteFile("/data/media/file.txt", []byte("shared content"), 0o644)
	_ = fsys.WriteFile("/data/archive/file.txt", []byte("archived content"), 0o644)
}

func shareTestMounts() []model.MountPoint {
	return []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "archive", Path: "/data/archive", ReadOnly: true},
	}
}

func newShareTestService(fsys filesystem.FS, mounts func() []model.MountPoint) ShareService {
	_ = fsys.MkdirAll("/data", 0o755)
	return NewShareService(fsys, ShareServiceConfig{
		DataDir: "/data",
		Mounts:  mounts,
	})
}

type renameGateFS struct {
	filesystem.FS
	renameStarted chan struct{}
	allowRename   <-chan struct{}
	once          sync.Once
}

func (f *renameGateFS) Rename(oldpath, newpath string) error {
	if strings.Contains(oldpath, ".share.") {
		f.once.Do(func() { close(f.renameStarted) })
		<-f.allowRename
	}
	return f.FS.Rename(oldpath, newpath)
}

func TestShareCreateRejectsInvalidTargets(t *testing.T) {
	tests := []struct {
		name    string
		path    string
		perms   model.SharePermissions
		wantErr error
	}{
		{name: "missing file", path: "media/missing.txt", wantErr: ErrPathNotFound},
		{name: "outside mounts", path: "elsewhere/file.txt", wantErr: validator.ErrOutsideMountPoint},
		{name: "path traversal", path: "media/../../etc/passwd", wantErr: validator.ErrPathTraversal},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			setupShareTestFS(fsys)
			shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

			share, err := shares.Create(context.Background(), "owner", test.path, ShareSettings{Permissions: test.perms}, time.Time{})
			if !errors.Is(err, test.wantErr) {
				t.Fatalf("Create() error = %v, want %v", err, test.wantErr)
			}
			if share != nil {
				t.Fatal("Create() returned a share despite the error")
			}
		})
	}
}

func TestShareCreateSupportsFoldersAndSimplifiesFilePermissions(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	folder, err := shares.Create(
		context.Background(),
		"owner",
		"media/subdir",
		ShareSettings{Permissions: model.SharePermissions{View: true, Upload: true}},
		time.Time{},
	)
	if err != nil {
		t.Fatal(err)
	}
	if !folder.IsFolder || !folder.Permissions.View || !folder.Permissions.Download || !folder.Permissions.Upload || folder.Permissions.Delete {
		t.Fatalf("folder share = %+v", folder)
	}

	file, err := shares.Create(
		context.Background(),
		"owner",
		"archive/file.txt",
		ShareSettings{Permissions: model.SharePermissions{Upload: true}},
		time.Time{},
	)
	if err != nil {
		t.Fatal(err)
	}
	if file.IsFolder || !file.Permissions.View || !file.Permissions.Download || file.Permissions.Upload || file.Permissions.Delete {
		t.Fatalf("file share = %+v", file)
	}
}

func TestShareFolderListsNestedItemsAndKeepsPathsRelative(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	_ = fsys.WriteFile("/data/media/subdir/notes.txt", []byte("notes"), 0o644)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	folder, err := shares.Create(context.Background(), "owner", "media/subdir", ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	root, err := shares.ListForRecipient(context.Background(), folder.Token, "")
	if err != nil {
		t.Fatal(err)
	}
	if root.Path != "" || len(root.Items) != 1 || root.Items[0].Path != "notes.txt" {
		t.Fatalf("root listing = %+v", root)
	}

	file, info, err := shares.OpenForRecipientPath(context.Background(), folder.Token, "notes.txt")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	if info.Name != "notes.txt" {
		t.Fatalf("nested file info = %+v", info)
	}
	content, err := io.ReadAll(file)
	if err != nil || string(content) != "notes" {
		t.Fatalf("nested file content = %q, err=%v", content, err)
	}
}

func TestShareFolderEditorCanCreateFileButViewerCannot(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	editor, err := shares.Create(context.Background(), "owner", "media/subdir", ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), editor.Token, "new.txt", strings.NewReader("new content")); err != nil {
		t.Fatal(err)
	}
	content, err := fsys.ReadFile("/data/media/subdir/new.txt")
	if err != nil || string(content) != "new content" {
		t.Fatalf("created file = %q, err=%v", content, err)
	}

	viewer, err := shares.Create(context.Background(), "owner", "media/subdir", ShareSettings{}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), viewer.Token, "blocked.txt", strings.NewReader("blocked")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("viewer write error = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestShareFolderRejectsInvalidRecipientPaths(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })
	folder, err := shares.Create(context.Background(), "owner", "media/subdir", ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	if _, err := shares.ListForRecipient(context.Background(), folder.Token, "../media"); !errors.Is(err, validator.ErrPathTraversal) {
		t.Fatalf("ListForRecipient() traversal error = %v, want %v", err, validator.ErrPathTraversal)
	}
	if _, err := shares.ListForRecipient(context.Background(), folder.Token, "missing"); !errors.Is(err, ErrPathNotFound) {
		t.Fatalf("ListForRecipient() missing path error = %v, want %v", err, ErrPathNotFound)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), folder.Token, "../escape.txt", strings.NewReader("escape")); !errors.Is(err, validator.ErrPathTraversal) {
		t.Fatalf("WriteForRecipientPath() traversal error = %v, want %v", err, validator.ErrPathTraversal)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), folder.Token, "missing/child.txt", strings.NewReader("child")); !errors.Is(err, ErrPathNotFound) {
		t.Fatalf("WriteForRecipientPath() missing parent error = %v, want %v", err, ErrPathNotFound)
	}
}

func TestShareLegacyFilePermissionsAreNormalized(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	const token = "legacy-token"
	data := []byte(`{"shares":[{"id":"legacy-id","token":"legacy-token","mountName":"media","relPath":"file.txt","permissions":{"write":true},"fileName":"file.txt","createdAt":"2025-01-01T00:00:00Z","createdBy":"owner"}]}`)
	if err := fsys.WriteFile(filepath.Join("/data", config.SharesFileName), data, 0o600); err != nil {
		t.Fatal(err)
	}

	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })
	share, err := shares.ResolveForRecipient(token)
	if err != nil {
		t.Fatal(err)
	}
	if !share.Permissions.View || !share.Permissions.Download || share.Permissions.Upload || share.Permissions.Delete {
		t.Fatalf("legacy file permissions = %+v", share.Permissions)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), token, "file.txt", strings.NewReader("should stay read-only")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("legacy file upload error = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestShareLegacyFolderWriteKeepsReplacementOnlyForLegacyUpdates(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	const token = "legacy-folder-token"
	data := []byte(`{"shares":[{"id":"legacy-folder","token":"legacy-folder-token","mountName":"media","relPath":"","isFolder":true,"permissions":{"write":true},"fileName":"media","createdAt":"2025-01-01T00:00:00Z","createdBy":"owner"}]}`)
	if err := fsys.WriteFile(filepath.Join("/data", config.SharesFileName), data, 0o600); err != nil {
		t.Fatal(err)
	}
	shares := NewShareService(fsys, ShareServiceConfig{
		DataDir:        "/data",
		MaxUploadBytes: 32,
		Mounts:         func() []model.MountPoint { return shareTestMounts() },
	})

	share, err := shares.ResolveForRecipient(token)
	if err != nil {
		t.Fatal(err)
	}
	if !share.Permissions.Upload || share.Permissions.Delete || !share.Permissions.LegacyReplace {
		t.Fatalf("legacy folder permissions = %+v", share.Permissions)
	}
	if share.MaxUploadBytes != 32 {
		t.Fatalf("legacy upload limit = %d, want configured default 32", share.MaxUploadBytes)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), token, "file.txt", strings.NewReader("replaced")); err != nil {
		t.Fatalf("legacy replacement failed: %v", err)
	}
	if err := shares.DeleteForRecipientPath(context.Background(), token, "file.txt"); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("legacy delete error = %v, want %v", err, ErrPermissionDenied)
	}

	updated, err := shares.Update("owner", share.ID, ShareSettings{
		Permissions:    model.SharePermissions{Upload: true, LegacyReplace: true},
		MaxUploadBytes: 24,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !updated.Permissions.LegacyReplace || updated.Permissions.Delete {
		t.Fatalf("updated legacy permissions = %+v", updated.Permissions)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), token, "file.txt", strings.NewReader("again")); err != nil {
		t.Fatalf("legacy replacement stopped after settings update: %v", err)
	}

	modernUpdate, err := shares.Update("owner", share.ID, ShareSettings{
		Permissions:    model.SharePermissions{View: true, Download: true, Upload: true},
		MaxUploadBytes: 24,
	})
	if err != nil {
		t.Fatal(err)
	}
	if modernUpdate.Permissions.LegacyReplace || modernUpdate.Permissions.Delete {
		t.Fatalf("modern upload-only update retained legacy access: %+v", modernUpdate.Permissions)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), token, "file.txt", strings.NewReader("blocked")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("modern upload-only replacement error = %v, want %v", err, ErrPermissionDenied)
	}
	contents, err := fsys.ReadFile("/data/media/file.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(contents) != "again" {
		t.Fatalf("modern update changed existing file to %q", contents)
	}
}

func TestShareCreateGeneratesHighEntropyTokens(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	first, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	second, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	for _, share := range []*model.Share{first, second} {
		if len(share.Token) != 43 {
			t.Fatalf("token %q length = %d, want 43", share.Token, len(share.Token))
		}
		raw, err := base64.RawURLEncoding.DecodeString(share.Token)
		if err != nil {
			t.Fatalf("token %q is not base64 raw URL encoding: %v", share.Token, err)
		}
		if len(raw) != 32 {
			t.Fatalf("token decodes to %d bytes, want 32", len(raw))
		}
	}
	if first.Token == second.Token {
		t.Fatal("two shares received identical tokens")
	}
}

func TestShareResolveForRecipientIsUniform(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })
	perms := ShareSettings{Permissions: model.SharePermissions{View: true, Download: true, Upload: true}}

	valid, err := shares.Create(context.Background(), "owner", "media/file.txt", perms, time.Now().Add(time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	expired, err := shares.Create(context.Background(), "owner", "media/file.txt", perms, time.Now().Add(-time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	revoked, err := shares.Create(context.Background(), "owner", "media/file.txt", perms, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if err := shares.Revoke("owner", revoked.ID); err != nil {
		t.Fatal(err)
	}

	tests := []struct {
		name  string
		token string
	}{
		{name: "unknown token", token: "does-not-exist"},
		{name: "expired share", token: expired.Token},
		{name: "revoked share", token: revoked.Token},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			share, err := shares.ResolveForRecipient(test.token)
			if !errors.Is(err, ErrShareNotFound) {
				t.Fatalf("ResolveForRecipient() error = %v, want %v", err, ErrShareNotFound)
			}
			if share != nil {
				t.Fatal("ResolveForRecipient() returned a share despite the error")
			}

			file, _, err := shares.OpenForRecipient(context.Background(), test.token)
			if file != nil {
				_ = file.Close()
			}
			if !errors.Is(err, ErrShareNotFound) {
				t.Fatalf("OpenForRecipient() error = %v, want %v", err, ErrShareNotFound)
			}

			if _, _, err := shares.WriteForRecipientPath(context.Background(), test.token, "data.txt", strings.NewReader("data")); !errors.Is(err, ErrShareNotFound) {
				t.Fatalf("WriteForRecipientPath() error = %v, want %v", err, ErrShareNotFound)
			}
		})
	}

	if _, err := shares.ResolveForRecipient(valid.Token); err != nil {
		t.Fatalf("valid share rejected: %v", err)
	}
}

func TestShareRevokeUnknownID(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	if err := shares.Revoke("owner", "missing-id"); !errors.Is(err, ErrShareNotFound) {
		t.Fatalf("Revoke() error = %v, want %v", err, ErrShareNotFound)
	}
}

func TestShareListReturnsActiveSharesOnly(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })
	perms := ShareSettings{Permissions: model.SharePermissions{View: true}}

	active, err := shares.Create(context.Background(), "owner", "media/file.txt", perms, time.Now().Add(time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	expired, err := shares.Create(context.Background(), "owner", "media/file.txt", perms, time.Now().Add(-time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	revoked, err := shares.Create(context.Background(), "owner", "media/file.txt", perms, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if err := shares.Revoke("owner", revoked.ID); err != nil {
		t.Fatal(err)
	}

	listed, err := shares.List("owner")
	if err != nil {
		t.Fatal(err)
	}
	if len(listed) != 1 {
		t.Fatalf("List() returned %d shares, want 1: %+v", len(listed), listed)
	}
	if listed[0].ID != active.ID {
		t.Fatalf("List() returned share %q, want %q", listed[0].ID, active.ID)
	}
	if listed[0].FileName != "file.txt" || listed[0].MountName != "media" || listed[0].RelPath != "file.txt" {
		t.Fatalf("List() share fields = %+v", listed[0])
	}
	if listed[0].CreatedBy != "owner" {
		t.Fatalf("CreatedBy = %q, want owner", listed[0].CreatedBy)
	}
	if expired.ID == active.ID {
		t.Fatal("distinct shares received identical IDs")
	}
}

func TestShareRecipientRevalidatesLiveMounts(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	mounts := shareTestMounts()
	shares := newShareTestService(fsys, func() []model.MountPoint { return mounts })
	perms := ShareSettings{Permissions: model.SharePermissions{Upload: true}}

	share, err := shares.Create(context.Background(), "owner", "media", perms, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	file, info, err := shares.OpenForRecipientPath(context.Background(), share.Token, "file.txt")
	if err != nil {
		t.Fatalf("OpenForRecipientPath() against live mount = %v", err)
	}
	_ = file.Close()
	if info.Name != "file.txt" {
		t.Fatalf("opened file name = %q, want file.txt", info.Name)
	}

	// The mount disappears: the share must fail like any other missing token.
	mounts = nil
	if _, _, err := shares.OpenForRecipientPath(context.Background(), share.Token, "file.txt"); !errors.Is(err, ErrShareNotFound) {
		t.Fatalf("OpenForRecipientPath() after mount removal = %v, want %v", err, ErrShareNotFound)
	}

	// The mount returns read-only: reads keep working, writes are denied.
	mounts = []model.MountPoint{{Name: "media", Path: "/data/media", ReadOnly: true}}
	file, _, err = shares.OpenForRecipientPath(context.Background(), share.Token, "file.txt")
	if err != nil {
		t.Fatalf("OpenForRecipientPath() on read-only mount = %v", err)
	}
	_ = file.Close()
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "new.txt", strings.NewReader("new")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("WriteForRecipientPath() on read-only mount = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestShareFileUploadIsAlwaysDenied(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	share, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{Permissions: model.SharePermissions{View: true, Download: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", strings.NewReader("new")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("WriteForRecipientPath() for a file share = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestShareRecipientRejectsChangedFolderTarget(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	mounts := shareTestMounts()
	shares := newShareTestService(fsys, func() []model.MountPoint { return mounts })

	share, err := shares.Create(context.Background(), "owner", "media/subdir", ShareSettings{}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if err := fsys.Remove("/data/media/subdir"); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/media/subdir", []byte("the folder was replaced"), 0o644); err != nil {
		t.Fatal(err)
	}

	if _, _, err := shares.InfoForRecipient(context.Background(), share.Token); !errors.Is(err, ErrShareNotFound) {
		t.Fatalf("InfoForRecipient() after folder replacement = %v, want %v", err, ErrShareNotFound)
	}
	if _, err := shares.ListForRecipient(context.Background(), share.Token, ""); !errors.Is(err, ErrShareNotFound) {
		t.Fatalf("ListForRecipient() after folder replacement = %v, want %v", err, ErrShareNotFound)
	}
}

func TestShareFolderUploadIsAtomicAndBounded(t *testing.T) {
	root := t.TempDir()
	media := filepath.Join(root, "media")
	if err := os.MkdirAll(media, 0o755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(media, "file.txt")
	if err := os.WriteFile(target, []byte("original"), 0o600); err != nil {
		t.Fatal(err)
	}
	fsys := filesystem.NewOsFS()
	dataDir := filepath.Join(root, "data")
	mounts := func() []model.MountPoint {
		return []model.MountPoint{{Name: "media", Path: media}}
	}
	shares := NewShareService(fsys, ShareServiceConfig{
		DataDir:        dataDir,
		MaxUploadBytes: 16,
		Mounts:         mounts,
	})
	perms := ShareSettings{Permissions: model.SharePermissions{View: true, Download: true, Upload: true, Delete: true}}

	share, err := shares.Create(context.Background(), "owner", "media", perms, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	written, fileName, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", strings.NewReader("replacement"))
	if err != nil {
		t.Fatal(err)
	}
	if fileName != "file.txt" {
		t.Fatalf("WriteForRecipientPath() file name = %q, want file.txt", fileName)
	}
	if written != int64(len("replacement")) {
		t.Fatalf("WriteForRecipientPath() wrote %d bytes, want %d", written, len("replacement"))
	}
	got, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "replacement" {
		t.Fatalf("target content = %q, want %q", got, "replacement")
	}
	info, err := os.Stat(target)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := info.Mode().Perm(), os.FileMode(0o600); got != want {
		t.Fatalf("replacement mode = %o, want %o", got, want)
	}
	assertOnlyEntries(t, media, "file.txt")

	// An oversized upload is rejected and leaves the previous content intact.
	oversized := strings.NewReader("this replacement is far longer than the configured limit")
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", oversized); !errors.Is(err, ErrShareTooLarge) {
		t.Fatalf("oversized WriteForRecipientPath() = %v, want %v", err, ErrShareTooLarge)
	}
	got, err = os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "replacement" {
		t.Fatalf("target content after failed upload = %q, want %q", got, "replacement")
	}
	assertOnlyEntries(t, media, "file.txt")

	// An empty upload is rejected without touching the target.
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", bytes.NewReader(nil)); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("empty WriteForRecipientPath() = %v, want %v", err, ErrInvalidOperation)
	}
	got, err = os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "replacement" {
		t.Fatalf("target content after empty upload = %q, want %q", got, "replacement")
	}
	assertOnlyEntries(t, media, "file.txt")
}

func TestShareUploadOnlyCannotReplaceOrDeleteAndEnforcesLinkLimit(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })
	share, err := shares.Create(context.Background(), "owner", "media", ShareSettings{
		Permissions:    model.SharePermissions{Upload: true},
		MaxUploadBytes: 4,
	}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if share.MaxUploadBytes != 4 {
		t.Fatalf("share upload limit = %d, want 4", share.MaxUploadBytes)
	}

	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", strings.NewReader("replacement")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("upload-only replacement error = %v, want %v", err, ErrPermissionDenied)
	}
	if err := shares.DeleteForRecipientPath(context.Background(), share.Token, "file.txt"); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("upload-only delete error = %v, want %v", err, ErrPermissionDenied)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "new.txt", strings.NewReader("four")); err != nil {
		t.Fatalf("new upload failed: %v", err)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "too-large.txt", strings.NewReader("five!")); !errors.Is(err, ErrShareTooLarge) {
		t.Fatalf("oversized upload error = %v, want %v", err, ErrShareTooLarge)
	}
	if exists, err := fsys.Exists("/data/media/too-large.txt"); err != nil || exists {
		t.Fatalf("oversized upload left a file (exists=%t, err=%v)", exists, err)
	}

	managed, err := shares.Update("owner", share.ID, ShareSettings{
		Permissions:    model.SharePermissions{Upload: true, Delete: true},
		MaxUploadBytes: 8,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !managed.Permissions.Upload || !managed.Permissions.Delete || managed.MaxUploadBytes != 8 {
		t.Fatalf("updated share = %+v", managed)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", strings.NewReader("replace")); err != nil {
		t.Fatalf("upload-and-delete replacement failed: %v", err)
	}
	if err := shares.DeleteForRecipientPath(context.Background(), share.Token, ""); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("shared-root deletion error = %v, want %v", err, ErrPermissionDenied)
	}
	if err := shares.DeleteForRecipientPath(context.Background(), share.Token, "../outside"); !errors.Is(err, validator.ErrPathTraversal) {
		t.Fatalf("traversal deletion error = %v, want %v", err, validator.ErrPathTraversal)
	}
	if err := shares.DeleteForRecipientPath(context.Background(), share.Token, "new.txt"); err != nil {
		t.Fatalf("managed delete failed: %v", err)
	}
	if exists, err := fsys.Exists("/data/media/new.txt"); err != nil || exists {
		t.Fatalf("managed delete left a file (exists=%t, err=%v)", exists, err)
	}

	viewer, err := shares.Update("owner", share.ID, ShareSettings{
		Permissions:    model.SharePermissions{View: true},
		MaxUploadBytes: 8,
	})
	if err != nil {
		t.Fatal(err)
	}
	if viewer.Permissions.Upload || viewer.Permissions.Delete {
		t.Fatalf("view-only update kept write permissions: %+v", viewer.Permissions)
	}
	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "blocked.txt", strings.NewReader("no")); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("view-only upload error = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestShareFolderArchiveContainsRelativeZipEntries(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	_ = fsys.MkdirAll("/data/media/subdir/nested", 0o755)
	_ = fsys.WriteFile("/data/media/subdir/nested/notes.txt", []byte("archive content"), 0o644)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })
	share, err := shares.Create(context.Background(), "owner", "media/subdir", ShareSettings{}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	archive, err := shares.PrepareDirectoryArchive(context.Background(), share.Token, "")
	if err != nil {
		t.Fatal(err)
	}
	var buffer bytes.Buffer
	if err := archive.WriteTo(context.Background(), &buffer); err != nil {
		t.Fatal(err)
	}
	reader, err := zip.NewReader(bytes.NewReader(buffer.Bytes()), int64(buffer.Len()))
	if err != nil {
		t.Fatal(err)
	}
	contents := make(map[string]string, len(reader.File))
	for _, entry := range reader.File {
		if strings.Contains(entry.Name, "media/") || strings.HasPrefix(entry.Name, "/") || strings.Contains(entry.Name, "../") {
			t.Fatalf("archive entry escaped share-relative names: %q", entry.Name)
		}
		file, err := entry.Open()
		if err != nil {
			t.Fatal(err)
		}
		content, readErr := io.ReadAll(file)
		closeErr := file.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read archive entry %q: read=%v close=%v", entry.Name, readErr, closeErr)
		}
		contents[entry.Name] = string(content)
	}
	if contents["subdir/nested/notes.txt"] != "archive content" {
		t.Fatalf("archive contents = %+v", contents)
	}
}

func assertOnlyEntries(t *testing.T, dir string, allowed ...string) {
	t.Helper()
	allowedSet := make(map[string]bool, len(allowed))
	for _, name := range allowed {
		allowedSet[name] = true
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if !allowedSet[entry.Name()] {
			t.Fatalf("unexpected leftover entry %q in %s", entry.Name(), dir)
		}
	}
}

func TestShareWriteRenamesOverResolvedTargetNotSymlink(t *testing.T) {
	root := t.TempDir()
	media := filepath.Join(root, "media")
	if err := os.MkdirAll(media, 0o755); err != nil {
		t.Fatal(err)
	}
	realFile := filepath.Join(media, "real.txt")
	if err := os.WriteFile(realFile, []byte("original"), 0o644); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(media, "link.txt")
	if err := os.Symlink(realFile, link); err != nil {
		t.Fatal(err)
	}
	fsys := filesystem.NewOsFS()
	shares := NewShareService(fsys, ShareServiceConfig{
		DataDir: filepath.Join(root, "data"),
		Mounts: func() []model.MountPoint {
			return []model.MountPoint{{Name: "media", Path: media}}
		},
	})

	// Sharing a symlink pins the resolved target inside the mount.
	share, err := shares.Create(context.Background(), "owner", "media", ShareSettings{Permissions: model.SharePermissions{Upload: true, Delete: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if !share.IsFolder || share.RelPath != "" {
		t.Fatalf("share = %+v, want a folder rooted at media", share)
	}

	if _, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "link.txt", strings.NewReader("replaced")); err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(realFile)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "replaced" {
		t.Fatalf("resolved target content = %q, want %q", got, "replaced")
	}
	linkInfo, err := os.Lstat(link)
	if err != nil {
		t.Fatal(err)
	}
	if linkInfo.Mode()&os.ModeSymlink == 0 {
		t.Fatal("symlink entry was replaced by a regular file")
	}
	assertOnlyEntries(t, media, "real.txt", "link.txt")
}

func TestShareRejectsSymlinkEscapingMount(t *testing.T) {
	root := t.TempDir()
	media := filepath.Join(root, "media")
	outside := filepath.Join(root, "outside")
	for _, dir := range []string{media, outside} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	secret := filepath.Join(outside, "secret.txt")
	if err := os.WriteFile(secret, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(secret, filepath.Join(media, "escape.txt")); err != nil {
		t.Fatal(err)
	}
	fsys := filesystem.NewOsFS()
	shares := NewShareService(fsys, ShareServiceConfig{
		DataDir: filepath.Join(root, "data"),
		Mounts: func() []model.MountPoint {
			return []model.MountPoint{{Name: "media", Path: media}}
		},
	})

	if _, err := shares.Create(context.Background(), "owner", "media/escape.txt", ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Time{}); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("Create() through escaping symlink = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestShareManagementIsScopedToOwner(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	aliceShare, err := shares.Create(context.Background(), "alice", "media/file.txt", ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	bobShare, err := shares.Create(context.Background(), "bob", "media/file.txt", ShareSettings{Permissions: model.SharePermissions{Download: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	listed, err := shares.List("alice")
	if err != nil {
		t.Fatal(err)
	}
	if len(listed) != 1 || listed[0].ID != aliceShare.ID {
		t.Fatalf("List(alice) = %+v, want only %q", listed, aliceShare.ID)
	}
	if _, err := shares.List(""); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("List(empty) error = %v, want %v", err, ErrInvalidOperation)
	}

	if err := shares.Revoke("alice", bobShare.ID); !errors.Is(err, ErrShareNotFound) {
		t.Fatalf("Revoke(alice, bob) error = %v, want %v", err, ErrShareNotFound)
	}
	if _, err := shares.ResolveForRecipient(bobShare.Token); err != nil {
		t.Fatalf("bob share was revoked by alice: %v", err)
	}
	if err := shares.Revoke("bob", bobShare.ID); err != nil {
		t.Fatalf("Revoke(bob, bob) = %v", err)
	}
}

func TestShareRevocationDuringUploadPreventsCommit(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	setupShareTestFS(fsys)
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	share, err := shares.Create(context.Background(), "owner", "media", ShareSettings{Permissions: model.SharePermissions{Upload: true, Delete: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	reader, writer := io.Pipe()
	defer writer.Close()
	uploadResult := make(chan error, 1)
	go func() {
		_, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", reader)
		uploadResult <- err
	}()

	if _, err := writer.Write([]byte("replacement")); err != nil {
		t.Fatal(err)
	}
	if err := shares.Revoke("owner", share.ID); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}

	select {
	case err := <-uploadResult:
		if !errors.Is(err, ErrShareNotFound) {
			t.Fatalf("WriteForRecipientPath() error = %v, want %v", err, ErrShareNotFound)
		}
	case <-time.After(time.Second):
		t.Fatal("WriteForRecipientPath() did not finish after the upload body closed")
	}

	content, err := fsys.ReadFile("/data/media/file.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "shared content" {
		t.Fatalf("target content after revocation = %q, want original", content)
	}
}

func TestShareRevokeWaitsForInFlightCommit(t *testing.T) {
	backingFS := filesystem.NewMemMapFS()
	setupShareTestFS(backingFS)
	allowRename := make(chan struct{})
	fsys := &renameGateFS{
		FS:            backingFS,
		renameStarted: make(chan struct{}),
		allowRename:   allowRename,
	}
	shares := newShareTestService(fsys, func() []model.MountPoint { return shareTestMounts() })

	share, err := shares.Create(context.Background(), "owner", "media", ShareSettings{Permissions: model.SharePermissions{Upload: true, Delete: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	var releaseOnce sync.Once
	releaseRename := func() { releaseOnce.Do(func() { close(allowRename) }) }
	t.Cleanup(releaseRename)

	uploadResult := make(chan error, 1)
	go func() {
		_, _, err := shares.WriteForRecipientPath(context.Background(), share.Token, "file.txt", strings.NewReader("replacement"))
		uploadResult <- err
	}()

	select {
	case <-fsys.renameStarted:
	case <-time.After(time.Second):
		t.Fatal("upload did not reach its final rename")
	}

	revokeStarted := make(chan struct{})
	revokeResult := make(chan error, 1)
	go func() {
		close(revokeStarted)
		revokeResult <- shares.Revoke("owner", share.ID)
	}()
	<-revokeStarted

	select {
	case err := <-revokeResult:
		t.Fatalf("Revoke() returned %v before the final rename finished", err)
	case <-time.After(100 * time.Millisecond):
	}

	releaseRename()
	if err := <-uploadResult; err != nil {
		t.Fatalf("WriteForRecipientPath() = %v", err)
	}
	if err := <-revokeResult; err != nil {
		t.Fatalf("Revoke() = %v", err)
	}
}

func TestShareStoreUsesPrivatePermissions(t *testing.T) {
	root := t.TempDir()
	dataDir := filepath.Join(root, "data")
	media := filepath.Join(root, "media")
	if err := os.MkdirAll(dataDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(media, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(media, "file.txt"), []byte("shared content"), 0o644); err != nil {
		t.Fatal(err)
	}
	sharesPath := filepath.Join(dataDir, config.SharesFileName)
	if err := os.WriteFile(sharesPath, []byte(`{"shares":[]}`), 0o644); err != nil {
		t.Fatal(err)
	}

	shares := NewShareService(filesystem.NewOsFS(), ShareServiceConfig{
		DataDir: dataDir,
		Mounts: func() []model.MountPoint {
			return []model.MountPoint{{Name: "media", Path: media}}
		},
	})
	assertPrivatePermissions(t, dataDir, 0o700)
	assertPrivatePermissions(t, sharesPath, 0o600)

	if _, err := shares.Create(context.Background(), "owner", "media/file.txt", ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Time{}); err != nil {
		t.Fatal(err)
	}
	assertPrivatePermissions(t, dataDir, 0o700)
	assertPrivatePermissions(t, sharesPath, 0o600)
}

func assertPrivatePermissions(t *testing.T, path string, want os.FileMode) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != want {
		t.Fatalf("%s permissions = %04o, want %04o", path, got, want)
	}
}
