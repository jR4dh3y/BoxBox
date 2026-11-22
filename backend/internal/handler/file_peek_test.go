package handler

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
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
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/files/peek/media/album", nil))
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
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/files/peek/media/album/notes.txt", nil))
	if rec.Code == http.StatusOK {
		t.Fatal("a file was peeked into")
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
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/files/properties", body))
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
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/files/properties", strings.NewReader(`{"paths":[]}`)))
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("an empty selection: status %d", rec.Code)
	}
}
