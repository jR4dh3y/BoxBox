package handler

import (
	"context"
	"errors"

	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/middleware"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestNestedReadOnlyMountBlocksParentAccess(t *testing.T) {
	home := t.TempDir()
	backups := filepath.Join(home, "backups")
	if err := os.Mkdir(backups, 0755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(backups, "db.bak")
	if err := os.WriteFile(target, []byte("precious"), 0644); err != nil {
		t.Fatal(err)
	}
	mounts := []model.MountPoint{{Name: "home", Path: home}, {Name: "backups", Path: backups, ReadOnly: true}}
	fsys := filesystem.NewOsFS()
	files := NewFileHandler(service.NewFileService(fsys, service.FileServiceConfig{MountPoints: mounts}))
	router := chi.NewRouter()
	router.Route("/api/v1/files", func(r chi.Router) { r.Use(middleware.MountPointGuard(mounts)); files.RegisterRoutes(r) })
	for _, check := range []struct {
		path   string
		status int
	}{{"backups/db.bak", 403}, {"home/backups/db.bak", 403}} {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, newShareManagementRequest(http.MethodDelete, "/api/v1/files/"+check.path, nil))
		t.Logf("DELETE %s -> %d", check.path, rec.Code)
		if rec.Code != check.status {
			t.Fatalf("want %d, body: %s", check.status, rec.Body.String())
		}
	}
	if content, err := os.ReadFile(target); err != nil || string(content) != "precious" {
		t.Fatalf("protected file changed: %q, %v", content, err)
	}
	shares := service.NewShareService(fsys, service.ShareServiceConfig{Mounts: func() []model.MountPoint { return mounts }, DataDir: t.TempDir(), MaxUploadBytes: 1 << 20})
	if _, err := shares.Create(context.Background(), "owner", "backups", service.ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{}); !errors.Is(err, service.ErrPermissionDenied) {
		t.Fatalf("direct read-only share creation: %v", err)
	}
	share, err := shares.Create(context.Background(), "owner", "home", service.ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	shareRouter := createShareTestRouter(NewShareHandler(shares, nil, 1))
	rec := httptest.NewRecorder()
	shareRouter.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=backups/planted.txt", strings.NewReader("planted")))
	t.Logf("anonymous upload into backups -> %d", rec.Code)
	if rec.Code != http.StatusForbidden {
		t.Fatalf("upload accepted: %s", rec.Body.String())
	}
	content, err := os.ReadFile(filepath.Join(backups, "planted.txt"))
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("protected upload exists: content=%q err=%v", content, err)
	}

	for _, path := range []string{"backups", "home/backups", "home"} {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, newShareManagementRequest(http.MethodDelete, "/api/v1/files/"+path+"?confirm=true", nil))
		if rec.Code != http.StatusForbidden {
			t.Fatalf("DELETE %s: %d", path, rec.Code)
		}
	}
	full, err := shares.Create(context.Background(), "owner", "home", service.ShareSettings{Permissions: model.SharePermissions{Upload: true, Delete: true, Manage: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	for _, request := range []struct{ method, route string }{{http.MethodDelete, "/items?path=backups/db.bak"}, {http.MethodDelete, "/items?path=backups"}, {http.MethodPost, "/folders?path=backups/new"}} {
		rec := httptest.NewRecorder()
		shareRouter.ServeHTTP(rec, httptest.NewRequest(request.method, "/api/v1/share/"+full.Token+request.route, nil))
		if rec.Code != http.StatusForbidden {
			t.Fatalf("%s %s: %d %s", request.method, request.route, rec.Code, rec.Body.String())
		}
	}
	rec = httptest.NewRecorder()
	shareRouter.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=allowed.txt", strings.NewReader("allowed")))
	if rec.Code != http.StatusOK {
		t.Fatalf("writable sibling upload: %d %s", rec.Code, rec.Body.String())
	}
	if content, err := os.ReadFile(filepath.Join(home, "allowed.txt")); err != nil || string(content) != "allowed" {
		t.Fatalf("sibling upload: %q %v", content, err)
	}

}

func TestNestedReadOnlyMountRejectsWritableShareSettings(t *testing.T) {
	home := t.TempDir()
	backups := filepath.Join(home, "backups")
	if err := os.Mkdir(backups, 0755); err != nil {
		t.Fatal(err)
	}
	mounts := []model.MountPoint{{Name: "home", Path: home}, {Name: "backups", Path: backups, ReadOnly: true}}
	shares := service.NewShareService(filesystem.NewOsFS(), service.ShareServiceConfig{Mounts: func() []model.MountPoint { return mounts }, DataDir: t.TempDir()})
	ctx := context.Background()
	if _, err := shares.Create(ctx, "owner", "home/backups", service.ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{}); !errors.Is(err, service.ErrPermissionDenied) {
		t.Fatalf("create writable protected share: %v", err)
	}
	share, err := shares.Create(ctx, "owner", "home/backups", service.ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := shares.Update("owner", share.ID, service.ShareUpdateSettings{Permissions: model.SharePermissions{Upload: true}}); !errors.Is(err, service.ErrPermissionDenied) {
		t.Fatalf("enable protected share upload: %v", err)
	}
}

func TestChunkUploadRespectsOverlappingReadOnlyMounts(t *testing.T) {
	home := t.TempDir()
	backups := filepath.Join(home, "backups")
	if err := os.Mkdir(backups, 0755); err != nil {
		t.Fatal(err)
	}
	mounts := []model.MountPoint{{Name: "root", Path: filepath.Dir(home), ReadOnly: true}, {Name: "home", Path: home}, {Name: "backups", Path: backups, ReadOnly: true}}
	files := service.NewFileService(filesystem.NewOsFS(), service.FileServiceConfig{MountPoints: mounts})
	handler := NewStreamHandler(files, 1)
	handler.uploadService = service.NewUploadService(files, t.TempDir())
	router := createStreamTestRouter(handler)
	for _, test := range []struct {
		path string
		want int
	}{{"home/backups/new.txt", http.StatusForbidden}, {"home/allowed.txt", http.StatusCreated}} {
		body := []byte("chunk")
		req := newChunkUploadRequest(test.path, body, "nested-readonly-upload", 0, 1, len(body), len(body))
		setUploadChecksum(req, body)
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, req)
		if rec.Code != test.want {
			t.Fatalf("upload %s: %d %s", test.path, rec.Code, rec.Body.String())
		}
	}
	if _, err := os.Stat(filepath.Join(backups, "new.txt")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("protected chunk upload exists: %v", err)
	}
	if body, err := os.ReadFile(filepath.Join(home, "allowed.txt")); err != nil || string(body) != "chunk" {
		t.Fatalf("writable child upload: %q %v", body, err)
	}
}

func TestInaccessibleReadOnlyMountBlocksWritableAlias(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("requires an unprivileged user to enforce directory permissions")
	}
	root := t.TempDir()
	target := filepath.Join(root, "target")
	locked := filepath.Join(root, "locked")
	for _, dir := range []string{target, locked} {
		if err := os.Mkdir(dir, 0755); err != nil {
			t.Fatal(err)
		}
	}
	readonlyAlias := filepath.Join(locked, "readonly")
	writableAlias := filepath.Join(root, "writable")
	for _, link := range []string{readonlyAlias, writableAlias} {
		if err := os.Symlink(target, link); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Chmod(locked, 0); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := os.Chmod(locked, 0755); err != nil {
			t.Error(err)
		}
	})
	if _, err := filepath.EvalSymlinks(readonlyAlias); !errors.Is(err, os.ErrPermission) {
		t.Fatalf("expected inaccessible read-only root, got %v", err)
	}
	mounts := []model.MountPoint{{Name: "readonly", Path: readonlyAlias, ReadOnly: true}, {Name: "writable", Path: writableAlias}}
	files := service.NewFileService(filesystem.NewOsFS(), service.FileServiceConfig{MountPoints: mounts})
	if err := files.CreateDir(context.Background(), "writable/planted"); !errors.Is(err, service.ErrPermissionDenied) {
		t.Errorf("mkdir through alias: %v", err)
	}
	handler := NewStreamHandler(files, 1)
	handler.uploadService = service.NewUploadService(files, t.TempDir())
	router := createStreamTestRouter(handler)
	body := []byte("planted")
	req := newChunkUploadRequest("writable/planted.txt", body, "inaccessible-readonly", 0, 1, len(body), len(body))
	setUploadChecksum(req, body)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusForbidden {
		t.Errorf("upload through alias: %d %s", rec.Code, rec.Body.String())
	}
	for _, path := range []string{"planted", "planted.txt"} {
		if _, err := os.Stat(filepath.Join(target, path)); !errors.Is(err, os.ErrNotExist) {
			t.Errorf("protected target was modified at %s: %v", path, err)
		}
	}
	// Once the root is accessible, normal identity checks deny its writable alias too.
	if err := os.Chmod(locked, 0755); err != nil {
		t.Fatal(err)
	}
	if err := files.CreateDir(context.Background(), "writable/still-protected"); !errors.Is(err, service.ErrPermissionDenied) {
		t.Fatalf("accessible alias: %v", err)
	}
}
