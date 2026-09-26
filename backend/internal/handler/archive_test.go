package handler

import (
	"archive/zip"
	"bytes"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/middleware"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func TestArchiveStreamsFolderZip(t *testing.T) {
	handler, fs, _ := setupTestStreamHandler()
	if err := fs.MkdirAll("/data/media/folder/nested", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/folder/nested/note.txt", []byte("archive note"), 0o644); err != nil {
		t.Fatal(err)
	}
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "documents", Path: "/data/documents"},
	}
	router := chi.NewRouter()
	router.Route("/api/v1", func(r chi.Router) {
		r.Group(func(r chi.Router) {
			r.Use(middleware.DevelopmentAuth)
			r.Route("/stream", func(r chi.Router) {
				r.Use(middleware.MountPointGuard(mounts))
				handler.RegisterRoutes(r)
			})
		})
	})

	req := httptest.NewRequest(http.MethodGet, "/api/v1/stream/archive/media/folder", nil)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("archive status = %d, want 200: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Content-Type"); got != "application/zip" {
		t.Fatalf("archive Content-Type = %q, want application/zip", got)
	}
	if got := rec.Header().Get("Content-Disposition"); !strings.Contains(got, "folder.zip") {
		t.Fatalf("archive Content-Disposition = %q", got)
	}
	reader, err := zip.NewReader(bytes.NewReader(rec.Body.Bytes()), int64(rec.Body.Len()))
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range reader.File {
		if entry.Name != "folder/nested/note.txt" {
			continue
		}
		file, err := entry.Open()
		if err != nil {
			t.Fatal(err)
		}
		content, readErr := io.ReadAll(file)
		closeErr := file.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read archive item: read=%v close=%v", readErr, closeErr)
		}
		if string(content) != "archive note" {
			t.Fatalf("archive content = %q", content)
		}
		return
	}
	t.Fatalf("ZIP is missing folder/nested/note.txt: %+v", reader.File)
}

func TestArchiveRouteEnforcesMountGuard(t *testing.T) {
	handler, _, _ := setupTestStreamHandler()
	mounts := []model.MountPoint{{Name: "media", Path: "/data/media"}}
	router := chi.NewRouter()
	router.Route("/api/v1/stream", func(r chi.Router) {
		r.Use(middleware.MountPointGuard(mounts))
		handler.RegisterRoutes(r)
	})

	req := httptest.NewRequest(http.MethodGet, "/api/v1/stream/archive/private/folder", nil)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusForbidden {
		t.Fatalf("archive outside configured mounts status = %d, want 403: %s", rec.Code, rec.Body.String())
	}
}
