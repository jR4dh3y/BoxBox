package handler

import (
	"compress/gzip"
	"context"
	"encoding/json"
	"fmt"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestDirectoryWireResponseSharesParentAndKeepsMetadata(t *testing.T) {
	handler, fs := setupTestFileHandler()
	if err := fs.MkdirAll("/data/media/My Photos/été", 0755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/My Photos/été #1.txt", []byte("hello"), 0640); err != nil {
		t.Fatal(err)
	}
	router := createFileTestRouter(handler)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/files/list/media/My%20Photos/", nil))
	if rec.Code != http.StatusOK {
		t.Fatalf("status %d: %s", rec.Code, rec.Body.String())
	}
	var wire struct {
		Path  string                       `json:"path"`
		Items []map[string]json.RawMessage `json:"items"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &wire); err != nil {
		t.Fatal(err)
	}
	if wire.Path != "media/My Photos" || len(wire.Items) != 2 {
		t.Fatalf("unexpected response: %s", rec.Body.String())
	}
	for _, item := range wire.Items {
		if _, ok := item["path"]; ok {
			t.Fatal("entry repeats the parent path")
		}
		for _, key := range []string{"name", "size", "isDir", "modTime", "permissions"} {
			if _, ok := item[key]; !ok {
				t.Fatalf("missing %s", key)
			}
		}
	}
	var listing directoryResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &listing); err != nil {
		t.Fatal(err)
	}
	for _, item := range listing.Items {
		if item.Name == "été #1.txt" && (item.Size != 5 || item.Permissions != "-rw-r-----" || item.MimeType == "") {
			t.Fatalf("lost metadata: %+v", item)
		}
	}
}

// Stable timestamps make compressed-byte ceilings independent of fixture creation speed.
type stableListingService struct{ service.FileManager }

func (s stableListingService) List(ctx context.Context, path string) (*model.FileList, error) {
	list, err := s.FileManager.List(ctx, path)
	if err != nil {
		return nil, err
	}
	for i := range list.Items {
		list.Items[i].ModTime = time.Date(2026, 10, 23, 0, 0, 0, 0, time.UTC)
	}
	return list, nil
}

func TestDirectoryResponseByteCeilings(t *testing.T) {
	handler, fs := setupTestFileHandler()
	for i := range 1000 {
		if err := fs.WriteFile(fmt.Sprintf("/data/media/File-%05d.txt", i), []byte("fixture\n"), 0644); err != nil {
			t.Fatal(err)
		}
	}
	handler.fileService = stableListingService{handler.fileService}
	router := createFileTestRouter(handler)
	request := httptest.NewRequest(http.MethodGet, "/api/v1/files/list/media", nil)
	request.Header.Set("Accept-Encoding", "gzip")
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, request)
	if rec.Code != http.StatusOK || rec.Header().Get("Content-Encoding") != "gzip" {
		t.Fatalf("status=%d headers=%v", rec.Code, rec.Header())
	}
	compressedBytes := rec.Body.Len()
	reader, err := gzip.NewReader(rec.Body)
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()
	body, err := io.ReadAll(reader)
	if err != nil {
		t.Fatal(err)
	}
	t.Logf("raw=%d compressed=%d", len(body), compressedBytes)
	if len(body) > 150000 {
		t.Fatalf("raw listing exceeds 150000 bytes: %d", len(body))
	}
	if compressedBytes > 4096 {
		t.Fatalf("compressed listing exceeds 4096 bytes: %d", compressedBytes)
	}
	var list directoryResponse
	if err := json.Unmarshal(body, &list); err != nil {
		t.Fatal(err)
	}
	if len(list.Items) != 1000 {
		t.Fatalf("got %d entries", len(list.Items))
	}
}
