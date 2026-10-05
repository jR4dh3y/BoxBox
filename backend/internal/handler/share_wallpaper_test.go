package handler

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

var shareWallpaperPNG = []byte("\x89PNG\r\n\x1a\nowner-wallpaper")

func setupShareWallpaperRouter(t *testing.T) (http.Handler, service.ShareService, service.SettingsService) {
	t.Helper()
	_, fs, shareSvc := setupTestShareHandler()
	settingsSvc := service.NewSettingsService(fs, service.SettingsServiceConfig{DataDir: "/data"})
	return createShareTestRouter(NewShareHandler(shareSvc, settingsSvc, 1)), shareSvc, settingsSvc
}

func getShareInfo(t *testing.T, router http.Handler, token string) model.ShareInfoResponse {
	t.Helper()
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/share/"+token, nil))
	if rec.Code != http.StatusOK {
		t.Fatalf("share info status = %d: %s", rec.Code, rec.Body.String())
	}
	var info model.ShareInfoResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &info); err != nil {
		t.Fatal(err)
	}
	return info
}

func TestShareServesOwnerWallpaper(t *testing.T) {
	router, shareSvc, settingsSvc := setupShareWallpaperRouter(t)
	perms := service.ShareSettings{Permissions: model.SharePermissions{View: true, Download: true}}
	share, err := shareSvc.Create(context.Background(), "owner", "media/file.txt", perms, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	if info := getShareInfo(t, router, share.Token); info.Wallpaper != nil {
		t.Fatalf("wallpaper reported before the owner set one: %+v", info.Wallpaper)
	}
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/wallpaper", nil))
	if rec.Code != http.StatusNotFound {
		t.Fatalf("wallpaper without one set: status = %d, want 404", rec.Code)
	}

	if _, err := settingsSvc.SetWallpaper("owner", shareWallpaperPNG, model.WallpaperDisplay{Mode: "contain", FrostedGlass: true}, "test-source"); err != nil {
		t.Fatal(err)
	}
	if _, err := settingsSvc.SetWallpaper("someone-else", []byte("\x89PNG\r\n\x1a\nother"), model.WallpaperDisplay{Mode: "cover"}, "test-source"); err != nil {
		t.Fatal(err)
	}

	info := getShareInfo(t, router, share.Token)
	if info.Wallpaper == nil || info.Wallpaper.Mode != "contain" || !info.Wallpaper.FrostedGlass || info.Wallpaper.Version == 0 {
		t.Fatalf("share info wallpaper = %+v", info.Wallpaper)
	}

	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/wallpaper", nil))
	if rec.Code != http.StatusOK {
		t.Fatalf("wallpaper status = %d", rec.Code)
	}
	if rec.Body.String() != string(shareWallpaperPNG) {
		t.Fatalf("served another user's wallpaper: %q", rec.Body.String())
	}
	if got := rec.Header().Get("Content-Type"); got != "image/png" {
		t.Fatalf("Content-Type = %q", got)
	}
	if got := rec.Header().Get("Content-Security-Policy"); got != streamSandboxCSP {
		t.Fatalf("Content-Security-Policy = %q", got)
	}

	if err := shareSvc.Revoke("owner", share.ID); err != nil {
		t.Fatal(err)
	}
	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/wallpaper", nil))
	if rec.Code != http.StatusNotFound {
		t.Fatalf("revoked share wallpaper status = %d, want 404", rec.Code)
	}
}
