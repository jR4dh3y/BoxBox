package handler

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestPeekPathReturnsAFewEntriesPicturesFirst(t *testing.T) {
	handler, fs := setupTestFileHandler()
	if err := fs.MkdirAll("/data/media/album", 0755); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"notes.txt", "cover.jpg", ".hidden.png"} {
		if err := fs.WriteFile("/data/media/album/"+name, []byte("x"), 0644); err != nil {
			t.Fatal(err)
		}
	}
	router := createFileTestRouter(handler)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/files/media/album?peek", nil))
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d: %s", rec.Code, rec.Body.String())
	}
	var body PeekResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if len(body.Items) != 2 || body.Items[0].Name != "cover.jpg" || body.Items[0].Path != "media/album/cover.jpg" {
		t.Fatalf("unexpected peek: %+v", body.Items)
	}

	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/files/media/album/notes.txt?peek", nil))
	if rec.Code == http.StatusOK {
		t.Fatal("a file was peeked into")
	}
}

func TestPeekPathAtStatsMount(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	if err := fs.MkdirAll("/data/stats", 0755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/stats/cover.jpg", []byte("x"), 0644); err != nil {
		t.Fatal(err)
	}
	files := service.NewFileService(fs, service.FileServiceConfig{MountPoints: []model.MountPoint{{Name: "stats", Path: "/data/stats"}}})
	router := createFileTestRouter(NewFileHandler(files))
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/files/stats?peek", nil))
	var body PeekResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &body); rec.Code != http.StatusOK || err != nil || len(body.Items) != 1 || body.Items[0].Path != "stats/cover.jpg" {
		t.Fatalf("unexpected peek: %d %s", rec.Code, rec.Body.String())
	}
}

func TestPropertiesTotalsASelection(t *testing.T) {
	handler, fs := setupTestFileHandler()
	if err := fs.MkdirAll("/data/media/album", 0755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/album/a.jpg", []byte("abc"), 0644); err != nil {
		t.Fatal(err)
	}
	router := createFileTestRouter(handler)
	rec := httptest.NewRecorder()
	body := strings.NewReader(`{"paths":["media/album"]}`)
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/files", body))
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d: %s", rec.Code, rec.Body.String())
	}
	var props model.PropertiesResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &props); err != nil {
		t.Fatal(err)
	}
	if props.Files != 1 || props.Folders != 1 || props.Bytes != 3 || !props.Complete {
		t.Fatalf("unexpected totals: %+v", props)
	}

	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/files", strings.NewReader(`{"paths":[]}`)))
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("an empty selection: status %d", rec.Code)
	}
}
