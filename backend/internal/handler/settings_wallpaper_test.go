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

func TestSettingsWallpaperImage(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	settings := service.NewSettingsService(fs, service.SettingsServiceConfig{DataDir: "/data"})
	for _, username := range []string{"alice", "bob"} {
		image := append(bytes.Clone(shareWallpaperPNG), []byte(username)...)
		if _, err := settings.SetWallpaper(username, image, model.WallpaperDisplay{Mode: "contain", FrostedGlass: true}, "source"); err != nil {
			t.Fatal(err)
		}
	}
	router := chi.NewRouter()
	router.Route("/api/v1/settings", NewSettingsHandler(settings).RegisterRoutes)

	for _, tc := range []struct {
		name     string
		username string
		status   int
	}{
		{"unauthenticated", "", http.StatusUnauthorized},
		{"alice", "alice", http.StatusOK},
		{"bob", "bob", http.StatusOK},
		{"no wallpaper", "charlie", http.StatusNotFound},
	} {
		t.Run(tc.name, func(t *testing.T) {
			request := newShareManagementRequestForUser(tc.username, http.MethodGet, "/api/v1/settings/wallpaper/image?username=bob", nil)
			recorder := httptest.NewRecorder()
			router.ServeHTTP(recorder, request)
			if recorder.Code != tc.status {
				t.Fatalf("status = %d, want %d: %s", recorder.Code, tc.status, recorder.Body.String())
			}
			if tc.status != http.StatusOK {
				return
			}
			want := append(bytes.Clone(shareWallpaperPNG), []byte(tc.username)...)
			if !bytes.Equal(recorder.Body.Bytes(), want) {
				t.Fatalf("response did not contain the caller's image: %q", recorder.Body.Bytes())
			}
			for key, want := range map[string]string{
				"Content-Type":            "image/png",
				"Cache-Control":           "private, no-store",
				"Content-Security-Policy": streamSandboxCSP,
			} {
				if got := recorder.Header().Get(key); got != want {
					t.Errorf("%s = %q, want %q", key, got, want)
				}
			}
			if recorder.Header().Get("Last-Modified") == "" {
				t.Error("missing image modification time")
			}
		})
	}
	if err := settings.DeleteWallpaper("alice"); err != nil {
		t.Fatal(err)
	}
	recorder := httptest.NewRecorder()
	router.ServeHTTP(recorder, newShareManagementRequestForUser("alice", http.MethodGet, "/api/v1/settings/wallpaper/image", nil))
	if recorder.Code != http.StatusNotFound {
		t.Fatalf("deleted wallpaper status = %d, want 404", recorder.Code)
	}
}
