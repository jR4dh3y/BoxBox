package main

import (
	"bytes"
	"compress/gzip"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func newCompressionTestServer(t *testing.T) http.Handler {
	t.Helper()
	root := t.TempDir()
	for i := 0; i < 60; i++ {
		if err := os.WriteFile(filepath.Join(root, fmt.Sprintf("holiday-photo-%03d.jpg", i)), []byte("x"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	cfg := &model.ServerConfig{
		Host:        "127.0.0.1",
		MaxUploadMB: 10,
		ChunkSizeMB: 5,
		DataDir:     t.TempDir(),
		MountPoints: []model.MountPoint{{Name: "files", Path: root}},
	}
	server, _, _, _, _, err := initializeServer(cfg, true)
	if err != nil {
		t.Fatal(err)
	}
	return server.Handler
}

func get(handler http.Handler, target, acceptEncoding string) *httptest.ResponseRecorder {
	req := httptest.NewRequest(http.MethodGet, target, nil)
	if acceptEncoding != "" {
		req.Header.Set("Accept-Encoding", acceptEncoding)
	}
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)
	return rec
}

func TestDirectoryListingIsGzippedWhenAccepted(t *testing.T) {
	handler := newCompressionTestServer(t)
	const target = "/api/v1/files/list/files?page=1&pageSize=50"

	plain := get(handler, target, "")
	if plain.Code != http.StatusOK || plain.Header().Get("Content-Encoding") != "" {
		t.Fatalf("plain: status %d, encoding %q", plain.Code, plain.Header().Get("Content-Encoding"))
	}

	zipped := get(handler, target, "gzip")
	if zipped.Header().Get("Content-Encoding") != "gzip" {
		t.Fatalf("want gzip, got %q", zipped.Header().Get("Content-Encoding"))
	}
	reader, err := gzip.NewReader(bytes.NewReader(zipped.Body.Bytes()))
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := io.ReadAll(reader)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(decoded, plain.Body.Bytes()) {
		t.Fatal("gzipped body differs from the plain body once decoded")
	}
	// Directory listings are highly repetitive; if this stops holding, compression is not doing its job.
	if zipped.Body.Len()*3 > plain.Body.Len() {
		t.Fatalf("gzip saved too little: %d -> %d bytes", plain.Body.Len(), zipped.Body.Len())
	}
}

func TestFileDownloadsAreNotRecompressed(t *testing.T) {
	handler := newCompressionTestServer(t)
	rec := get(handler, "/api/v1/stream/files/holiday-photo-000.jpg", "gzip")
	if rec.Code != http.StatusOK && rec.Code != http.StatusPartialContent {
		t.Fatalf("stream status %d: %s", rec.Code, rec.Body.String())
	}
	if encoding := rec.Header().Get("Content-Encoding"); encoding != "" {
		t.Fatalf("file stream must not be content-encoded, got %q", encoding)
	}
}

func TestSmallResponsesOnCompressedRoutesAreNotGzipped(t *testing.T) {
	handler := newCompressionTestServer(t)
	// An empty page of a listing and a search with no matches: both go through the compression middleware.
	for _, target := range []string{
		"/api/v1/files/list/files?page=9",
		"/api/v1/search?path=files&q=zzzzzz",
	} {
		plain := get(handler, target, "")
		zipped := get(handler, target, "gzip")
		if zipped.Code != http.StatusOK || plain.Code != http.StatusOK {
			t.Fatalf("%s: status plain %d, gzip %d", target, plain.Code, zipped.Code)
		}
		if encoding := zipped.Header().Get("Content-Encoding"); encoding != "" {
			t.Errorf("%s: a %d byte body must not be gzipped, got encoding %q", target, plain.Body.Len(), encoding)
		}
		if !bytes.Equal(zipped.Body.Bytes(), plain.Body.Bytes()) {
			t.Errorf("%s: body changed when gzip was accepted", target)
		}
	}
}

func TestErrorResponsesKeepTheirStatus(t *testing.T) {
	handler := newCompressionTestServer(t)
	rec := get(handler, "/api/v1/search?q=abc", "gzip")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("want 400 for a search without a path, got %d", rec.Code)
	}
}

func TestGzipIsSkippedWhenTheClientRefusesIt(t *testing.T) {
	handler := newCompressionTestServer(t)
	// Large enough to be gzipped if the refusal were ignored.
	for _, acceptEncoding := range []string{"gzip;q=0", "gzip;q=0.0", "gzip;q=0.000", "br, gzip;q=0.000"} {
		rec := get(handler, "/api/v1/files/list/files?page=1&pageSize=50", acceptEncoding)
		if rec.Code != http.StatusOK {
			t.Fatalf("%q: status %d", acceptEncoding, rec.Code)
		}
		if encoding := rec.Header().Get("Content-Encoding"); encoding != "" {
			t.Errorf("%q must not be gzipped, got %q", acceptEncoding, encoding)
		}
	}
}
