package handler

import (
	"bytes"
	"context"
	"errors"
	"image"
	"image/color"
	_ "image/jpeg"
	"image/png"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/jR4dh3y/BoxBox/backend/internal/static"
)

var testDefaultImage = []byte("default-png")

type capturedDocument struct{ meta static.PageMeta }

func (d *capturedDocument) ServeIndexWithMeta(w http.ResponseWriter, _ *http.Request, meta static.PageMeta) {
	d.meta = meta
	w.WriteHeader(http.StatusOK)
}

func (d *capturedDocument) Asset(name string) ([]byte, bool) {
	return testDefaultImage, name == defaultPreviewImage
}

func testPNG(t *testing.T, width, height int) []byte {
	t.Helper()
	img := image.NewRGBA(image.Rect(0, 0, width, height))
	for x := range width {
		img.Set(x, 0, color.RGBA{R: 200, A: 255})
	}
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		t.Fatal(err)
	}
	return buf.Bytes()
}

type previewFixture struct {
	shares   service.ShareService
	document *capturedDocument
	router   *chi.Mux
}

func newPreviewFixture(t *testing.T, shares service.ShareService, publicURL string) previewFixture {
	t.Helper()
	document := &capturedDocument{}
	preview := NewSharePreviewHandler(shares, document, publicURL)
	r := chi.NewRouter()
	r.Get("/s/{token}", preview.Page)
	r.Get("/api/v1/share/{token}/thumbnail", preview.Image)
	return previewFixture{shares: shares, document: document, router: r}
}

func setupPreview(t *testing.T) (previewFixture, func(path string) string) {
	t.Helper()
	_, fs, shares := setupTestShareHandler()
	if err := fs.WriteFile("/data/media/photo.PNG", testPNG(t, 2000, 1000), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := fs.MkdirAll("/data/media/cover.jpg", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/broken.jpg", []byte("not an image"), 0o644); err != nil {
		t.Fatal(err)
	}
	share := func(path string) string {
		created, err := shares.Create(context.Background(), "owner", path,
			service.ShareSettings{Permissions: model.SharePermissions{View: true, Download: true}}, time.Time{})
		if err != nil {
			t.Fatal(err)
		}
		return created.Token
	}
	return newPreviewFixture(t, shares, ""), share
}

func (f previewFixture) page(t *testing.T, token string, headers map[string]string) static.PageMeta {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, "/s/"+token, nil)
	req.Host = "files.example.com"
	for key, value := range headers {
		if key == "Host" {
			req.Host = value
			continue
		}
		req.Header.Set(key, value)
	}
	rec := httptest.NewRecorder()
	f.document.meta = static.PageMeta{}
	f.router.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("page status = %d", rec.Code)
	}
	return f.document.meta
}

func TestSharePagePreviewByKind(t *testing.T) {
	f, share := setupPreview(t)
	file, photo, folder, coverDir := share("media/file.txt"), share("media/photo.PNG"), share("media/shared"), share("media/cover.jpg")

	meta := f.page(t, file, nil)
	if meta.OGTitle != "file.txt" || meta.Title != "\u2068file.txt\u2069 - BoxBox" ||
		meta.Description != "View this shared file on BoxBox" || meta.URL != "http://files.example.com/s/"+file ||
		meta.Card != static.TwitterCardSummary || meta.Image == nil || meta.Image.URL != "http://files.example.com/og-default.png" ||
		meta.Image.Width != 1200 || meta.Image.Height != 630 {
		t.Errorf("file meta = %+v %+v", meta, meta.Image)
	}

	meta = f.page(t, photo, nil)
	if meta.Card != static.TwitterCardLargeImage || meta.Image == nil ||
		!strings.HasPrefix(meta.Image.URL, "http://files.example.com/api/v1/share/"+photo+"/thumbnail?v=") || meta.Image.Width != 0 {
		t.Errorf("image meta = %+v %+v", meta, meta.Image)
	}

	for _, token := range []string{folder, coverDir} {
		meta = f.page(t, token, nil)
		if meta.Description != "View this shared folder on BoxBox" || meta.Card != static.TwitterCardSummary ||
			meta.Image == nil || !strings.HasSuffix(meta.Image.URL, "/og-default.png") {
			t.Errorf("folder meta = %+v %+v", meta, meta.Image)
		}
	}
}

func TestSharePageIsGenericAndUniformForMissingShares(t *testing.T) {
	f, share := setupPreview(t)
	expired, err := f.shares.Create(context.Background(), "owner", "media/file.txt",
		service.ShareSettings{Permissions: model.SharePermissions{View: true}}, time.Now().Add(-time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	revokedToken := share("media/file.txt")
	revokedShare, err := f.shares.ResolveForRecipient(revokedToken)
	if err != nil {
		t.Fatal(err)
	}
	if err := f.shares.Revoke("owner", revokedShare.ID); err != nil {
		t.Fatal(err)
	}

	want := f.page(t, "unknown-token", nil)
	if want.OGTitle != "BoxBox" || want.Title != "Shared - BoxBox" || want.URL != "" || want.Card != static.TwitterCardSummary {
		t.Fatalf("generic meta = %+v", want)
	}
	for name, token := range map[string]string{"expired": expired.Token, "revoked": revokedToken, "too long": strings.Repeat("a", 200)} {
		got := f.page(t, token, nil)
		if got.Title != want.Title || got.OGTitle != want.OGTitle || got.Description != want.Description ||
			got.URL != want.URL || got.Card != want.Card || *got.Image != *want.Image {
			t.Errorf("%s meta = %+v, want %+v", name, got, want)
		}
		if strings.Contains(got.Title+got.OGTitle+got.Description, "file.txt") {
			t.Errorf("%s meta leaks the file name", name)
		}
	}
}

type faultyShares struct {
	service.ShareService
	fail func()
}

func (s faultyShares) InfoForRecipient(context.Context, string) (*model.FileInfo, string, error) {
	s.fail()
	return nil, "", errors.New("disk on fire")
}

func TestSharePageIsGenericOnServiceErrorAndPanic(t *testing.T) {
	_, _, shares := setupTestShareHandler()
	for name, fail := range map[string]func(){"error": func() {}, "panic": func() { panic("boom") }} {
		f := newPreviewFixture(t, faultyShares{ShareService: shares, fail: fail}, "")
		if meta := f.page(t, "some-token", nil); meta.OGTitle != "BoxBox" || meta.URL != "" {
			t.Errorf("%s meta = %+v", name, meta)
		}
	}
}

func TestSharePageOrigin(t *testing.T) {
	f, share := setupPreview(t)
	token := share("media/file.txt")

	if meta := f.page(t, token, map[string]string{"X-Forwarded-Proto": "https"}); meta.URL != "https://files.example.com/s/"+token {
		t.Errorf("forwarded https URL = %q", meta.URL)
	}
	if meta := f.page(t, token, map[string]string{"Host": "[::1]:8080"}); meta.URL != "http://[::1]:8080/s/"+token {
		t.Errorf("IPv6 URL = %q", meta.URL)
	}
	for _, host := range []string{`evil"><x`, "a b", "host:99999", "::1", "", "user@host"} {
		meta := f.page(t, token, map[string]string{"Host": host})
		if meta.URL != "" || meta.Image != nil {
			t.Errorf("host %q produced URL %q image %+v", host, meta.URL, meta.Image)
		}
	}

	configured := newPreviewFixture(t, f.shares, "https://share.example.org")
	if meta := configured.page(t, token, map[string]string{"Host": "evil.test"}); meta.URL != "https://share.example.org/s/"+token {
		t.Errorf("public_url URL = %q", meta.URL)
	}
}

func TestPreviewText(t *testing.T) {
	tests := map[string]string{
		"line\nbreak\x00.txt":    "line break .txt",
		"bad\xff\xfe.jpg":        "bad\uFFFD.jpg",
		"evil\u202egnp.exe":      "evilgnp.exe",
		"שלום.txt":               "שלום.txt",
		strings.Repeat("a", 300): strings.Repeat("a", maxPreviewNameRunes-1) + "\u2026",
	}
	for input, want := range tests {
		if got := previewText(input); got != want {
			t.Errorf("previewText(%q) = %q, want %q", input, got, want)
		}
	}
}

func thumbnailRequest(f previewFixture, token, ifNoneMatch string) *httptest.ResponseRecorder {
	req := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+token+"/thumbnail", nil)
	if ifNoneMatch != "" {
		req.Header.Set("If-None-Match", ifNoneMatch)
	}
	rec := httptest.NewRecorder()
	f.router.ServeHTTP(rec, req)
	return rec
}

func TestSharePreviewImage(t *testing.T) {
	f, share := setupPreview(t)
	photo := share("media/photo.PNG")

	rec := thumbnailRequest(f, photo, "")
	if rec.Code != http.StatusOK || rec.Header().Get("Content-Type") != "image/jpeg" || rec.Header().Get("ETag") == "" ||
		rec.Header().Get("Cache-Control") != "no-cache" {
		t.Fatalf("thumbnail = %d %v", rec.Code, rec.Header())
	}
	config, format, err := image.DecodeConfig(rec.Body)
	if err != nil || format != "jpeg" || config.Width != sharePreviewImageSize || config.Height != 600 {
		t.Fatalf("thumbnail decode = %+v %q %v, want 1200x600 jpeg", config, format, err)
	}

	etag := rec.Header().Get("ETag")
	rec = thumbnailRequest(f, photo, etag)
	if rec.Code != http.StatusNotModified || rec.Header().Get("ETag") != etag || rec.Header().Get("Cache-Control") != "no-cache" {
		t.Fatalf("conditional thumbnail = %d %v", rec.Code, rec.Header())
	}

	for _, path := range []string{"media/file.txt", "media/broken.jpg", "media/shared"} {
		rec = thumbnailRequest(f, share(path), "")
		if rec.Code != http.StatusOK || rec.Header().Get("Content-Type") != "image/png" || !bytes.Equal(rec.Body.Bytes(), testDefaultImage) {
			t.Errorf("%s fallback = %d %q", path, rec.Code, rec.Header().Get("Content-Type"))
		}
	}

	revoked, err := f.shares.ResolveForRecipient(photo)
	if err != nil {
		t.Fatal(err)
	}
	if err := f.shares.Revoke("owner", revoked.ID); err != nil {
		t.Fatal(err)
	}
	for _, ifNoneMatch := range []string{"", etag} {
		if rec = thumbnailRequest(f, photo, ifNoneMatch); rec.Code != http.StatusNotFound {
			t.Errorf("revoked thumbnail (If-None-Match %q) = %d, want 404", ifNoneMatch, rec.Code)
		}
	}
	if rec = thumbnailRequest(f, "unknown-token", ""); rec.Code != http.StatusNotFound {
		t.Errorf("unknown thumbnail = %d, want 404", rec.Code)
	}
}
