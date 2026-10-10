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

func TestSmallResponsesAreNotGzipped(t *testing.T) {
	handler := newCompressionTestServer(t)
	rec := get(handler, "/api/v1/files/", "gzip")
	if encoding := rec.Header().Get("Content-Encoding"); encoding != "" {
		t.Fatalf("roots is tiny and gzip would grow it, got encoding %q", encoding)
	}
}
