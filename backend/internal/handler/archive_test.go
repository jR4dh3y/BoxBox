package handler

import (
	"archive/zip"
	"bytes"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestArchiveStreamsFolderZip(t *testing.T) {
	handler, fs, _ := setupTestStreamHandler()
	if err := fs.MkdirAll("/data/media/folder/nested", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/folder/nested/note.txt", []byte("archive note"), 0o644); err != nil {
		t.Fatal(err)
	}
	router := createStreamTestRouter(handler)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/archive/media/folder", nil)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("archive status = %d, want 200: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Content-Type"); got != "application/zip" {
		t.Fatalf("archive Content-Type = %q, want application/zip", got)
	}
	if got := rec.Header().Get("Content-Disposition"); !strings.Contains(got, "folder.zip") {
		t.Fatalf("archive Content-Disposition = %q", got)
	}
	reader, err := zip.NewReader(bytes.NewReader(rec.Body.Bytes()), int64(rec.Body.Len()))
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range reader.File {
		if entry.Name != "folder/nested/note.txt" {
			continue
		}
		file, err := entry.Open()
		if err != nil {
			t.Fatal(err)
		}
		content, readErr := io.ReadAll(file)
		closeErr := file.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read archive item: read=%v close=%v", readErr, closeErr)
		}
		if string(content) != "archive note" {
			t.Fatalf("archive content = %q", content)
		}
		return
	}
	t.Fatalf("ZIP is missing folder/nested/note.txt: %+v", reader.File)
}
