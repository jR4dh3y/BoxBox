package handler

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"slices"
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

func TestDirectoryListingEndpointsReturnCompleteRefreshableResults(t *testing.T) {
	handler, fs := setupTestFileHandler()
	router := createFileTestRouter(handler)
	dir := "/data/media/listing"
	if err := fs.MkdirAll(dir, 0755); err != nil {
		t.Fatal(err)
	}
	for i := range 75 {
		name := fmt.Sprintf("file%03d.txt", i)
		if err := fs.WriteFile(dir+"/"+name, nil, 0644); err != nil {
			t.Fatal(err)
		}
	}

	endpoints := []string{
		"/api/v1/files/list/media/listing",
		"/api/v1/files/media/listing",
	}
	getListing := func(endpoint string) model.FileList {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, endpoint, nil))
		if rec.Code != http.StatusOK {
			t.Fatalf("GET %s returned %d: %s", endpoint, rec.Code, rec.Body.String())
		}

		var fields map[string]json.RawMessage
		if err := json.Unmarshal(rec.Body.Bytes(), &fields); err != nil {
			t.Fatal(err)
		}
		if len(fields) != 2 || fields["path"] == nil || fields["items"] == nil {
			t.Fatalf("listing response fields = %v, want only path and items", fields)
		}

		var listing model.FileList
		if err := json.Unmarshal(rec.Body.Bytes(), &listing); err != nil {
			t.Fatal(err)
		}
		for i := range listing.Items {
			if endpoint == "/api/v1/files/media/listing" && listing.Items[i].Path == "" {
				t.Fatal("file-info directory response lost its entry paths")
			}
			if listing.Items[i].Path == "" {
				listing.Items[i].Path = listing.Path + "/" + listing.Items[i].Name
			}
		}
		return listing
	}

	for _, endpoint := range endpoints {
		listing := getListing(endpoint)
		if listing.Path != "media/listing" || len(listing.Items) != 75 {
			t.Fatalf("initial listing = path %q, %d items; want media/listing and 75", listing.Path, len(listing.Items))
		}
		// The server sends directory order; the client sorts.
		for i, name := range sortedNames(listing) {
			if want := fmt.Sprintf("file%03d.txt", i); name != want {
				t.Fatalf("initial item %d = %q, want %q", i, name, want)
			}
		}
	}

	if err := fs.WriteFile(dir+"/file075.txt", nil, 0644); err != nil {
		t.Fatal(err)
	}
	for _, endpoint := range endpoints {
		listing := getListing(endpoint)
		if listing.Path != "media/listing" || len(listing.Items) != 76 {
			t.Fatalf("refreshed listing = path %q, %d items; want media/listing and 76", listing.Path, len(listing.Items))
		}
		names := sortedNames(listing)
		if got := names[len(names)-1]; got != "file075.txt" {
			t.Fatalf("refreshed final item = %q, want file075.txt", got)
		}
	}
}

func sortedNames(listing model.FileList) []string {
	names := make([]string, 0, len(listing.Items))
	for _, item := range listing.Items {
		names = append(names, item.Name)
	}
	slices.Sort(names)
	return names
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

func TestDeleteSymlinkToFolderNeedsNoConfirmationAndKeepsTarget(t *testing.T) {
	home := t.TempDir()
	photos := filepath.Join(home, "photos")
	if err := os.MkdirAll(photos, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(photos, "a.jpg"), []byte("jpeg"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(photos, filepath.Join(home, "shortcut")); err != nil {
		t.Fatal(err)
	}
	fileSvc := service.NewFileService(filesystem.NewOsFS(), service.FileServiceConfig{
		MountPoints: []model.MountPoint{{Name: "home", Path: home}},
	})
	router := createFileTestRouter(NewFileHandler(fileSvc))

	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodDelete, "/api/v1/files/home/photos", nil))
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("unconfirmed folder delete: status = %d, want %d", rec.Code, http.StatusBadRequest)
	}

	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodDelete, "/api/v1/files/home/shortcut", nil))
	if rec.Code != http.StatusOK {
		t.Fatalf("symlink delete: status = %d, body = %s", rec.Code, rec.Body.String())
	}
	if _, err := os.Lstat(filepath.Join(home, "shortcut")); !os.IsNotExist(err) {
		t.Fatalf("symlink still exists: %v", err)
	}
	if _, err := os.Stat(filepath.Join(photos, "a.jpg")); err != nil {
		t.Fatalf("symlink target was deleted: %v", err)
	}
}
