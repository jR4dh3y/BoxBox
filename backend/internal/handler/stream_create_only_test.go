package handler

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestCreateOnlyUploadHTTPBoundary(t *testing.T) {
	root := t.TempDir()
	files := service.NewFileService(filesystem.NewOsFS(), service.FileServiceConfig{MountPoints: []model.MountPoint{{Name: "media", Path: root}}})
	h := NewStreamHandler(files, 10)
	h.uploadService = service.NewUploadService(files, t.TempDir())
	router := chi.NewRouter()
	router.Route("/api/v1/stream", h.RegisterRoutes)
	router.Get("/api/v1/system/upload", NewSystemHandler(service.NewSystemService(), 1).GetUploadConfig)
	config := httptest.NewRecorder()
	router.ServeHTTP(config, httptest.NewRequest("GET", "/api/v1/system/upload", nil))
	var capability UploadConfigResponse
	if err := json.Unmarshal(config.Body.Bytes(), &capability); err != nil || !capability.CreateOnly {
		t.Fatal("missing create-only capability")
	}
	send := func(req *http.Request, want int) {
		t.Helper()
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, req)
		if rec.Code != want {
			t.Fatalf("status %d, want %d: %s", rec.Code, want, rec.Body.String())
		}
	}
	upload := func(path, id string, body []byte, want int) {
		t.Helper()
		req := newChunkUploadRequest(path, body, id, 0, 1, max(1, len(body)), len(body))
		req.URL.Path = "/api/v1/stream/upload/" + path
		req.Header.Set("X-Create-Only", "true")
		setUploadChecksum(req, body)
		send(req, want)
	}
	upload("media/file.bin", "first", []byte("original"), 201)
	upload("media/file.bin", "collision", []byte("replacement"), 409)
	got, err := os.ReadFile(filepath.Join(root, "file.bin"))
	if err != nil || !bytes.Equal(got, []byte("original")) {
		t.Fatal("collision changed original")
	}
	upload("media/empty.bin", "empty", nil, 201)
	info, err := os.Stat(filepath.Join(root, "empty.bin"))
	if err != nil || info.Size() != 0 {
		t.Fatal("empty file not published")
	}
	req := httptest.NewRequest("POST", "/api/v1/stream/upload/media/folder", nil)
	req.Header.Set("X-Create-Only", "true")
	req.Header.Set("X-Upload-Type", "directory")
	send(req, 201)
	req = httptest.NewRequest("POST", "/api/v1/stream/upload/media/folder", nil)
	req.Header.Set("X-Create-Only", "true")
	req.Header.Set("X-Upload-Type", "directory")
	send(req, 409)
	upload("media/folder/child.bin", "nested", []byte("nested"), 201)
	req = newChunkUploadRequest("media/bad.bin", []byte("bad"), "checksum", 0, 1, 3, 3)
	req.URL.Path = "/api/v1/stream/upload/media/bad.bin"
	req.Header.Set("X-Create-Only", "true")
	setUploadChecksum(req, []byte("different"))
	send(req, 422)
	if _, err := os.Stat(filepath.Join(root, "bad.bin")); !os.IsNotExist(err) {
		t.Fatal("checksum failure published destination")
	}
}
