package handler

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestDesktopWallpaperStagingPreservesCommittedImages(t *testing.T) {
	directory := t.TempDir()
	fs := filesystem.NewOsFS()
	settings := service.NewSettingsService(fs, service.SettingsServiceConfig{DataDir: directory})
	old := append(bytes.Clone(shareWallpaperPNG), []byte("old")...)
	if _, err := settings.SetWallpaper("dev", old, model.WallpaperDisplay{Mode: "cover"}, "desktop"); err != nil {
		t.Fatal(err)
	}
	router := func() http.Handler {
		r := chi.NewRouter()
		RegisterDesktopPreferences(r, service.NewDesktopPreferences(fs, directory), service.NewSettingsService(fs, service.SettingsServiceConfig{DataDir: directory}))
		return r
	}
	request := func(method, path string, body []byte) *httptest.ResponseRecorder {
		t.Helper()
		response := httptest.NewRecorder()
		router().ServeHTTP(response, httptest.NewRequest(method, path, bytes.NewReader(body)))
		if response.Code < 200 || response.Code >= 300 {
			t.Fatalf("%s %s: %d %s", method, path, response.Code, response.Body.String())
		}
		return response
	}
	stage := func(image []byte) string {
		t.Helper()
		response := request(http.MethodPut, "/wallpaper", image)
		var meta model.WallpaperMeta
		if err := json.Unmarshal(response.Body.Bytes(), &meta); err != nil {
			t.Fatal(err)
		}
		return "/wallpaper/" + meta.SHA256
	}
	first := append(bytes.Clone(shareWallpaperPNG), []byte("first")...)
	firstURL := stage(first)
	committed := []byte(`{"backgroundImage":"/api/v1/desktop` + firstURL + `"}`)
	request(http.MethodPut, "/preferences", committed)
	second := append(bytes.Clone(shareWallpaperPNG), []byte("abandoned")...)
	secondURL := stage(second)
	if firstURL == secondURL {
		t.Fatal("different images share a reference")
	}
	// Recreate the handlers/services on every request to include the disk persistence boundary.
	for path, want := range map[string][]byte{"/wallpaper": old, firstURL: first, secondURL: second, "/preferences": committed} {
		got := request(http.MethodGet, path, nil)
		if !bytes.Equal(bytes.TrimSpace(got.Body.Bytes()), want) {
			t.Fatalf("staging changed %s: %q", path, got.Body.Bytes())
		}
	}
	response := httptest.NewRecorder()
	router().ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/wallpaper/not-a-hash", nil))
	if response.Code != http.StatusBadRequest {
		t.Fatalf("invalid reference status: %d", response.Code)
	}
}
