// Package handler provides HTTP handlers for the BoxBox API.
package handler

// Focused HTTP tests for streaming and upload boundaries.

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"mime"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// setupTestStreamHandler creates a stream handler with an in-memory filesystem for testing
func setupTestStreamHandler() (*StreamHandler, *filesystem.AferoFS, service.FileService) {
	fs := filesystem.NewMemMapFS()

	// Create mount point directories
	fs.MkdirAll("/data/media", 0755)
	fs.MkdirAll("/data/documents", 0755)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "documents", Path: "/data/documents", ReadOnly: false},
	}

	fileSvc := service.NewFileService(fs, service.FileServiceConfig{MountPoints: mounts})
	streamHandler := NewStreamHandler(fileSvc, 100) // 100MB max for testing

	return streamHandler, fs, fileSvc
}

// createTestRouter creates a chi router with the stream handler for testing
func createStreamTestRouter(handler *StreamHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Route("/api/v1", func(r chi.Router) {
		handler.RegisterRoutes(r)
	})
	return r
}

func newChunkUploadRequest(
	filePath string,
	body []byte,
	uploadID string,
	chunkIndex int,
	totalChunks int,
	chunkSize int,
	totalSize int,
) *http.Request {
	req := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(body))
	req.Header.Set("X-Upload-ID", uploadID)
	req.Header.Set("X-Chunk-Index", strconv.Itoa(chunkIndex))
	req.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
	req.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
	req.Header.Set("X-Total-Size", strconv.Itoa(totalSize))
	return req
}

func setUploadChecksum(req *http.Request, content []byte) {
	sum := sha256.Sum256(content)
	req.Header.Set("X-Checksum", hex.EncodeToString(sum[:]))
}

func TestUploadRejectsInvalidUploadID(t *testing.T) {
	handler, _, _ := setupTestStreamHandler()
	router := createStreamTestRouter(handler)

	req := newChunkUploadRequest("media/invalid-id.bin", []byte("ab"), "../bad", 0, 1, 2, 2)
	setUploadChecksum(req, []byte("ab"))
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected status %d, got %d: %s", http.StatusBadRequest, rec.Code, rec.Body.String())
	}
}

func TestPreviewSandboxesContentAndBlocksActiveDocuments(t *testing.T) {
	tests := []struct {
		name            string
		filename        string
		content         []byte
		wantDisposition string
	}{
		{
			name:            "HTML is downloaded",
			filename:        "attack.html",
			content:         []byte(`<script>window.top.location = "https://attacker.example"</script>`),
			wantDisposition: "attachment",
		},
		{
			name:            "SVG is downloaded",
			filename:        "attack.svg",
			content:         []byte(`<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>`),
			wantDisposition: "attachment",
		},
		{
			name:            "PDF remains inline",
			filename:        "document.pdf",
			content:         []byte("%PDF-1.7\n"),
			wantDisposition: "inline",
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)
			if err := fs.WriteFile("/data/media/"+test.filename, test.content, 0o644); err != nil {
				t.Fatalf("write fixture: %v", err)
			}

			req := httptest.NewRequest(http.MethodGet, "/api/v1/preview/media/"+test.filename, nil)
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)

			if rec.Code != http.StatusOK {
				t.Fatalf("expected status %d, got %d: %s", http.StatusOK, rec.Code, rec.Body.String())
			}
			if got := rec.Header().Get("Content-Security-Policy"); got != streamSandboxCSP {
				t.Fatalf("Content-Security-Policy = %q, want %q", got, streamSandboxCSP)
			}
			disposition, params, err := mime.ParseMediaType(rec.Header().Get("Content-Disposition"))
			if err != nil {
				t.Fatalf("parse Content-Disposition: %v", err)
			}
			if disposition != test.wantDisposition {
				t.Fatalf("disposition = %q, want %q", disposition, test.wantDisposition)
			}
			if params["filename"] != test.filename {
				t.Fatalf("filename = %q, want %q", params["filename"], test.filename)
			}
		})
	}
}

func TestStreamResponsesEncodeQuotedFilenames(t *testing.T) {
	handler, fs, _ := setupTestStreamHandler()
	router := createStreamTestRouter(handler)
	filename := `evil"name.txt`
	if err := fs.WriteFile("/data/media/"+filename, []byte("safe text"), 0o644); err != nil {
		t.Fatalf("write fixture: %v", err)
	}

	for _, route := range []string{"preview", "download"} {
		t.Run(route, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/api/v1/"+route+"/media/"+filename, nil)
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)

			if rec.Code != http.StatusOK {
				t.Fatalf("expected status %d, got %d: %s", http.StatusOK, rec.Code, rec.Body.String())
			}
			_, params, err := mime.ParseMediaType(rec.Header().Get("Content-Disposition"))
			if err != nil {
				t.Fatalf("parse Content-Disposition: %v", err)
			}
			if params["filename"] != filename {
				t.Fatalf("filename = %q, want %q", params["filename"], filename)
			}
			if got := rec.Header().Get("Content-Security-Policy"); got != streamSandboxCSP {
				t.Fatalf("Content-Security-Policy = %q, want %q", got, streamSandboxCSP)
			}
		})
	}
}

func TestUploadRejectsFinalChunkWithoutChecksum(t *testing.T) {
	handler, fs, _ := setupTestStreamHandler()
	router := createStreamTestRouter(handler)

	firstReq := newChunkUploadRequest("media/checksum-required.bin", []byte("ab"), "checksum-required", 0, 2, 2, 4)
	firstRec := httptest.NewRecorder()
	router.ServeHTTP(firstRec, firstReq)
	if firstRec.Code != http.StatusOK {
		t.Fatalf("expected initial status %d, got %d: %s", http.StatusOK, firstRec.Code, firstRec.Body.String())
	}

	missingChecksumReq := newChunkUploadRequest("media/checksum-required.bin", []byte("cd"), "checksum-required", 1, 2, 2, 4)
	missingChecksumRec := httptest.NewRecorder()
	router.ServeHTTP(missingChecksumRec, missingChecksumReq)
	if missingChecksumRec.Code != http.StatusBadRequest {
		t.Fatalf("expected missing checksum status %d, got %d: %s", http.StatusBadRequest, missingChecksumRec.Code, missingChecksumRec.Body.String())
	}

	exists, _ := fs.Exists("/data/media/checksum-required.bin")
	if exists {
		t.Fatal("file should not be assembled without a final checksum")
	}

	finalReq := newChunkUploadRequest("media/checksum-required.bin", []byte("cd"), "checksum-required", 1, 2, 2, 4)
	setUploadChecksum(finalReq, []byte("abcd"))
	finalRec := httptest.NewRecorder()
	router.ServeHTTP(finalRec, finalReq)
	if finalRec.Code != http.StatusCreated {
		t.Fatalf("expected retry status %d, got %d: %s", http.StatusCreated, finalRec.Code, finalRec.Body.String())
	}
}

func TestUploadRejectsTotalSizeAboveConfiguredMax(t *testing.T) {
	handler, _, _ := setupTestStreamHandler()
	handler.maxUploadBytes = 2
	router := createStreamTestRouter(handler)

	req := newChunkUploadRequest("media/too-large.bin", []byte("ab"), "too-large", 0, 2, 2, 3)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("expected status %d, got %d: %s", http.StatusRequestEntityTooLarge, rec.Code, rec.Body.String())
	}
}

func TestUploadRejectsChunkBodyLargerThanMetadata(t *testing.T) {
	handler, _, _ := setupTestStreamHandler()
	router := createStreamTestRouter(handler)

	req := newChunkUploadRequest("media/chunk-large.bin", []byte("abc"), "chunk-large", 0, 2, 2, 3)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("expected status %d, got %d: %s", http.StatusRequestEntityTooLarge, rec.Code, rec.Body.String())
	}
}

func TestUploadRejectsChunkBodyShorterThanMetadata(t *testing.T) {
	handler, _, _ := setupTestStreamHandler()
	router := createStreamTestRouter(handler)

	req := newChunkUploadRequest("media/chunk-short.bin", []byte("a"), "chunk-short", 0, 2, 2, 3)
	rec := httptest.NewRecorder()

	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected status %d, got %d: %s", http.StatusBadRequest, rec.Code, rec.Body.String())
	}
}

func TestUploadRejectsSessionMetadataMismatch(t *testing.T) {
	handler, _, _ := setupTestStreamHandler()
	router := createStreamTestRouter(handler)

	firstReq := newChunkUploadRequest("media/original.bin", []byte("ab"), "same-id", 0, 2, 2, 4)
	firstRec := httptest.NewRecorder()
	router.ServeHTTP(firstRec, firstReq)
	if firstRec.Code != http.StatusOK {
		t.Fatalf("expected initial status %d, got %d: %s", http.StatusOK, firstRec.Code, firstRec.Body.String())
	}

	secondReq := newChunkUploadRequest("media/other.bin", []byte("cd"), "same-id", 1, 2, 2, 4)
	secondRec := httptest.NewRecorder()
	router.ServeHTTP(secondRec, secondReq)

	if secondRec.Code != http.StatusConflict {
		t.Fatalf("expected status %d, got %d: %s", http.StatusConflict, secondRec.Code, secondRec.Body.String())
	}
}
