package handler

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func setupTestFileHandler() (*FileHandler, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()
	_ = fs.MkdirAll("/data/media", 0755)
	_ = fs.MkdirAll("/data/documents", 0755)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "documents", Path: "/data/documents", ReadOnly: false},
	}

	fileSvc := service.NewFileService(fs, service.FileServiceConfig{MountPoints: mounts})
	return NewFileHandler(fileSvc), fs
}

func createFileTestRouter(handler *FileHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Route("/api/v1/files", func(r chi.Router) {
		handler.RegisterRoutes(r)
	})
	return r
}

func TestCreateItemCreatesFileViaAPI(t *testing.T) {
	handler, fs := setupTestFileHandler()
	router := createFileTestRouter(handler)

	req := httptest.NewRequest(
		http.MethodPost,
		"/api/v1/files/media",
		bytes.NewBufferString(`{"name":"note.txt","type":"file"}`),
	)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusCreated {
		t.Fatalf("expected status %d, got %d: %s", http.StatusCreated, rec.Code, rec.Body.String())
	}

	content, err := fs.ReadFile("/data/media/note.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "" {
		t.Fatalf("expected empty file, got %q", string(content))
	}
}

func TestCreateItemRejectsExistingFileViaAPI(t *testing.T) {
	handler, fs := setupTestFileHandler()
	router := createFileTestRouter(handler)

	if err := fs.WriteFile("/data/media/note.txt", []byte("existing"), 0644); err != nil {
		t.Fatal(err)
	}

	req := httptest.NewRequest(
		http.MethodPost,
		"/api/v1/files/media",
		bytes.NewBufferString(`{"name":"note.txt","type":"file"}`),
	)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusConflict {
		t.Fatalf("expected status %d, got %d: %s", http.StatusConflict, rec.Code, rec.Body.String())
	}
}

func TestSaveFileContentViaAPI(t *testing.T) {
	handler, fs := setupTestFileHandler()
	router := createFileTestRouter(handler)

	if err := fs.WriteFile("/data/media/editable.txt", []byte("before"), 0644); err != nil {
		t.Fatal(err)
	}

	req := httptest.NewRequest(
		http.MethodPatch,
		"/api/v1/files/media/editable.txt",
		bytes.NewBufferString(`{"content":"after"}`),
	)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d: %s", http.StatusOK, rec.Code, rec.Body.String())
	}

	content, err := fs.ReadFile("/data/media/editable.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "after" {
		t.Fatalf("expected overwritten content, got %q", string(content))
	}
}
