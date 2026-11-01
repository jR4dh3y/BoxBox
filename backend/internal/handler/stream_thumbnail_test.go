package handler

import (
	"bytes"
	"context"
	"image"
	"image/jpeg"
	"image/png"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/middleware"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"golang.org/x/crypto/bcrypt"
)

func TestThumbnailRoutePreservesAuthMountAndCacheBehavior(t *testing.T) {
	streamHandler, fsys, _ := setupTestStreamHandler()
	var source bytes.Buffer
	img := image.NewRGBA(image.Rect(0, 0, 800, 400))
	for index := 0; index < len(img.Pix); index += 4 {
		img.Pix[index] = 210
		img.Pix[index+1] = 80
		img.Pix[index+2] = 30
		img.Pix[index+3] = 255
	}
	if err := png.Encode(&source, img); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/media/photo.png", source.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}

	passwordHash, err := bcrypt.GenerateFromPassword([]byte("thumbnail-test-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	auth, err := service.NewAuthService(filesystem.NewOsFS(), service.AuthServiceConfig{
		DataDir:   t.TempDir(),
		JWTSecret: "thumbnail-test-jwt-secret-is-long-enough",
		Users:     map[string]string{"viewer": string(passwordHash)},
	})
	if err != nil {
		t.Fatal(err)
	}
	tokens, err := auth.Login(context.Background(), "viewer", "thumbnail-test-password")
	if err != nil {
		t.Fatal(err)
	}

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "documents", Path: "/data/documents"},
	}
	router := chi.NewRouter()
	router.Route("/api/v1", func(r chi.Router) {
		r.Group(func(r chi.Router) {
			r.Use(middleware.JWTAuth(auth))
			r.Route("/stream", func(r chi.Router) {
				r.Use(middleware.MountPointGuard(mounts))
				streamHandler.RegisterRoutes(r)
			})
		})
	})

	unauthenticated := httptest.NewRecorder()
	router.ServeHTTP(unauthenticated, httptest.NewRequest(http.MethodGet, "/api/v1/stream/thumbnail/media/photo.png", nil))
	if unauthenticated.Code != http.StatusUnauthorized {
		t.Fatalf("unauthenticated thumbnail status = %d, want 401", unauthenticated.Code)
	}

	response := httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest(
		http.MethodGet,
		"/api/v1/stream/thumbnail/media/photo.png?token="+tokens.AccessToken+"&width=224&height=144",
		nil,
	))
	if response.Code != http.StatusOK {
		t.Fatalf("thumbnail status = %d, want 200: %s", response.Code, response.Body.String())
	}
	if got := response.Header().Get("Content-Type"); got != "image/jpeg" {
		t.Fatalf("thumbnail Content-Type = %q, want image/jpeg", got)
	}
	if got := response.Header().Get("Cache-Control"); got != "private, max-age=3600" {
		t.Fatalf("thumbnail Cache-Control = %q", got)
	}
	etag := response.Header().Get("ETag")
	if etag == "" {
		t.Fatal("thumbnail response is missing an etag")
	}
	decoded, err := jpeg.Decode(bytes.NewReader(response.Body.Bytes()))
	if err != nil {
		t.Fatalf("thumbnail response is not JPEG: %v", err)
	}
	if decoded.Bounds().Dx() != 224 || decoded.Bounds().Dy() != 112 {
		t.Fatalf("thumbnail bounds = %v, want 224x112", decoded.Bounds())
	}

	conditional := httptest.NewRequest(
		http.MethodGet,
		"/api/v1/stream/thumbnail/media/photo.png?token="+tokens.AccessToken+"&width=224&height=144",
		nil,
	)
	conditional.Header.Set("If-None-Match", etag)
	notModified := httptest.NewRecorder()
	router.ServeHTTP(notModified, conditional)
	if notModified.Code != http.StatusNotModified || notModified.Body.Len() != 0 {
		t.Fatalf("conditional thumbnail response = %d with %d body bytes, want 304 with no body", notModified.Code, notModified.Body.Len())
	}

	outsideMount := httptest.NewRecorder()
	router.ServeHTTP(outsideMount, httptest.NewRequest(
		http.MethodGet,
		"/api/v1/stream/thumbnail/private/photo.png?token="+tokens.AccessToken,
		nil,
	))
	if outsideMount.Code != http.StatusForbidden {
		t.Fatalf("outside-mount thumbnail status = %d, want 403", outsideMount.Code)
	}
}
