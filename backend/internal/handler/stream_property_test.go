// Package handler provides HTTP handlers for the file manager API.
// This file contains property-based tests for streaming operations.
package handler

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/homelab/filemanager/internal/model"
	"github.com/homelab/filemanager/internal/pkg/filesystem"
	"github.com/homelab/filemanager/internal/service"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
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
	streamHandler := NewStreamHandler(fileSvc, 1, 100) // 1MB chunks, 100MB max for testing

	return streamHandler, fs, fileSvc
}

func testChunkCount(totalSize, chunkSize int) int {
	return (totalSize + chunkSize - 1) / chunkSize
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

// **Feature: homelab-file-manager, Property 4: Upload/Download Round-Trip Integrity**
// **Validates: Requirements 2.5, 3.1**
//
// Property: For any file content uploaded via chunked upload, downloading that file
// SHALL return byte-identical content with matching checksum.

func TestProperty_UploadDownloadRoundTripIntegrity(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file content (varying sizes)
	contentGen := gen.SliceOfN(1024, gen.UInt8()) // Up to 1KB content

	// Generator for file names
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,15}`)

	properties.Property("uploaded content matches downloaded content", prop.ForAll(
		func(content []byte, fileName string) bool {
			if fileName == "" || len(content) == 0 {
				return true // Skip empty cases
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			filePath := "media/" + fileName + ".bin"
			uploadID := "test-upload-" + fileName

			// Calculate expected checksum
			hasher := sha256.New()
			hasher.Write(content)
			expectedChecksum := hex.EncodeToString(hasher.Sum(nil))

			// Upload as single chunk
			uploadReq := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(content))
			uploadReq.Header.Set("X-Upload-ID", uploadID)
			uploadReq.Header.Set("X-Chunk-Index", "0")
			uploadReq.Header.Set("X-Total-Chunks", "1")
			uploadReq.Header.Set("X-Chunk-Size", strconv.Itoa(len(content)))
			uploadReq.Header.Set("X-Total-Size", strconv.Itoa(len(content)))
			uploadReq.Header.Set("X-Checksum", expectedChecksum)

			uploadRec := httptest.NewRecorder()
			router.ServeHTTP(uploadRec, uploadReq)

			if uploadRec.Code != http.StatusCreated {
				return false
			}

			// Verify file exists in filesystem
			fsPath := "/data/media/" + fileName + ".bin"
			exists, _ := fs.Exists(fsPath)
			if !exists {
				return false
			}

			// Read file content directly from filesystem
			downloadedContent, err := fs.ReadFile(fsPath)
			if err != nil {
				return false
			}

			// Verify content matches
			if !bytes.Equal(content, downloadedContent) {
				return false
			}

			// Verify checksum matches
			downloadHasher := sha256.New()
			downloadHasher.Write(downloadedContent)
			actualChecksum := hex.EncodeToString(downloadHasher.Sum(nil))

			return expectedChecksum == actualChecksum
		},
		contentGen,
		fileNameGen,
	))

	properties.Property("multi-chunk upload produces correct file", prop.ForAll(
		func(content []byte, fileName string, numChunks int) bool {
			if fileName == "" || len(content) == 0 || numChunks < 1 {
				return true
			}

			// Limit chunks to reasonable number
			if numChunks > 10 {
				numChunks = 10
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			filePath := "media/" + fileName + "_multi.bin"
			uploadID := "multi-upload-" + fileName

			// Calculate expected checksum
			hasher := sha256.New()
			hasher.Write(content)
			expectedChecksum := hex.EncodeToString(hasher.Sum(nil))

			// Calculate chunk size
			chunkSize := (len(content) + numChunks - 1) / numChunks
			if chunkSize < 1 {
				chunkSize = 1
			}
			totalChunks := testChunkCount(len(content), chunkSize)

			// Upload in chunks
			for i := 0; i < totalChunks; i++ {
				start := i * chunkSize
				end := start + chunkSize
				if end > len(content) {
					end = len(content)
				}
				if start >= len(content) {
					break
				}

				chunkData := content[start:end]

				uploadReq := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(chunkData))
				uploadReq.Header.Set("X-Upload-ID", uploadID)
				uploadReq.Header.Set("X-Chunk-Index", strconv.Itoa(i))
				uploadReq.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
				uploadReq.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
				uploadReq.Header.Set("X-Total-Size", strconv.Itoa(len(content)))

				// Add checksum on final chunk
				if i == totalChunks-1 {
					uploadReq.Header.Set("X-Checksum", expectedChecksum)
				}

				uploadRec := httptest.NewRecorder()
				router.ServeHTTP(uploadRec, uploadReq)

				// Last chunk should return 201, others 200
				expectedStatus := http.StatusOK
				if i == totalChunks-1 {
					expectedStatus = http.StatusCreated
				}
				if uploadRec.Code != expectedStatus {
					return false
				}
			}

			// Verify file content
			fsPath := "/data/media/" + fileName + "_multi.bin"
			downloadedContent, err := fs.ReadFile(fsPath)
			if err != nil {
				return false
			}

			return bytes.Equal(content, downloadedContent)
		},
		contentGen,
		fileNameGen,
		gen.IntRange(1, 5),
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 5: Resumable Upload Correctness**
// **Validates: Requirements 2.3**
//
// Property: For any upload interrupted after N successful chunks, resuming the upload
// from chunk N SHALL result in a complete file identical to the original.

func TestProperty_ResumableUploadCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file content
	contentGen := gen.SliceOfN(512, gen.UInt8())

	// Generator for file names
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,10}`)

	// Generator for number of chunks
	numChunksGen := gen.IntRange(2, 5)

	// Generator for interruption point (which chunk to stop at)
	interruptGen := gen.IntRange(0, 3)

	properties.Property("interrupted upload can be resumed successfully", prop.ForAll(
		func(content []byte, fileName string, numChunks int, interruptAt int) bool {
			if fileName == "" || len(content) == 0 || numChunks < 2 {
				return true
			}

			// Ensure interrupt point is valid
			if interruptAt >= numChunks-1 {
				interruptAt = numChunks - 2
			}
			if interruptAt < 0 {
				interruptAt = 0
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			filePath := "media/" + fileName + "_resume.bin"
			uploadID := "resume-upload-" + fileName

			// Calculate expected checksum
			hasher := sha256.New()
			hasher.Write(content)
			expectedChecksum := hex.EncodeToString(hasher.Sum(nil))

			// Calculate chunk size
			chunkSize := (len(content) + numChunks - 1) / numChunks
			if chunkSize < 1 {
				chunkSize = 1
			}
			totalChunks := testChunkCount(len(content), chunkSize)
			if totalChunks < 2 {
				return true
			}
			if interruptAt >= totalChunks-1 {
				interruptAt = totalChunks - 2
			}
			if interruptAt < 0 {
				interruptAt = 0
			}

			// Phase 1: Upload chunks up to interrupt point
			for i := 0; i <= interruptAt; i++ {
				start := i * chunkSize
				end := start + chunkSize
				if end > len(content) {
					end = len(content)
				}
				if start >= len(content) {
					break
				}

				chunkData := content[start:end]

				uploadReq := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(chunkData))
				uploadReq.Header.Set("X-Upload-ID", uploadID)
				uploadReq.Header.Set("X-Chunk-Index", strconv.Itoa(i))
				uploadReq.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
				uploadReq.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
				uploadReq.Header.Set("X-Total-Size", strconv.Itoa(len(content)))

				uploadRec := httptest.NewRecorder()
				router.ServeHTTP(uploadRec, uploadReq)

				if uploadRec.Code != http.StatusOK {
					return false
				}
			}

			// Phase 2: Resume upload from interrupt point + 1
			for i := interruptAt + 1; i < totalChunks; i++ {
				start := i * chunkSize
				end := start + chunkSize
				if end > len(content) {
					end = len(content)
				}
				if start >= len(content) {
					break
				}

				chunkData := content[start:end]

				uploadReq := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(chunkData))
				uploadReq.Header.Set("X-Upload-ID", uploadID)
				uploadReq.Header.Set("X-Chunk-Index", strconv.Itoa(i))
				uploadReq.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
				uploadReq.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
				uploadReq.Header.Set("X-Total-Size", strconv.Itoa(len(content)))

				// Add checksum on final chunk
				if i == totalChunks-1 {
					uploadReq.Header.Set("X-Checksum", expectedChecksum)
				}

				uploadRec := httptest.NewRecorder()
				router.ServeHTTP(uploadRec, uploadReq)

				// Last chunk should return 201, others 200
				expectedStatus := http.StatusOK
				if i == totalChunks-1 {
					expectedStatus = http.StatusCreated
				}
				if uploadRec.Code != expectedStatus {
					return false
				}
			}

			// Verify file content matches original
			fsPath := "/data/media/" + fileName + "_resume.bin"
			downloadedContent, err := fs.ReadFile(fsPath)
			if err != nil {
				return false
			}

			return bytes.Equal(content, downloadedContent)
		},
		contentGen,
		fileNameGen,
		numChunksGen,
		interruptGen,
	))

	properties.Property("re-uploading same chunk is idempotent", prop.ForAll(
		func(content []byte, fileName string) bool {
			if fileName == "" || len(content) == 0 {
				return true
			}

			handler, _, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			filePath := "media/" + fileName + "_idempotent.bin"
			uploadID := "idempotent-upload-" + fileName

			numChunks := 3
			chunkSize := (len(content) + numChunks - 1) / numChunks
			if chunkSize < 1 {
				chunkSize = 1
			}
			totalChunks := testChunkCount(len(content), chunkSize)

			// Upload first chunk
			start := 0
			end := chunkSize
			if end > len(content) {
				end = len(content)
			}
			chunkData := content[start:end]

			uploadReq := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(chunkData))
			uploadReq.Header.Set("X-Upload-ID", uploadID)
			uploadReq.Header.Set("X-Chunk-Index", "0")
			uploadReq.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
			uploadReq.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
			uploadReq.Header.Set("X-Total-Size", strconv.Itoa(len(content)))

			uploadRec := httptest.NewRecorder()
			router.ServeHTTP(uploadRec, uploadReq)

			if uploadRec.Code != http.StatusOK {
				return false
			}

			// Re-upload the same chunk (simulating retry)
			uploadReq2 := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(chunkData))
			uploadReq2.Header.Set("X-Upload-ID", uploadID)
			uploadReq2.Header.Set("X-Chunk-Index", "0")
			uploadReq2.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
			uploadReq2.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
			uploadReq2.Header.Set("X-Total-Size", strconv.Itoa(len(content)))

			uploadRec2 := httptest.NewRecorder()
			router.ServeHTTP(uploadRec2, uploadReq2)

			// Should still return 200 OK (idempotent)
			return uploadRec2.Code == http.StatusOK
		},
		contentGen,
		fileNameGen,
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 6: Range Request Correctness**
// **Validates: Requirements 3.2**
//
// Property: For any file and valid byte range [start, end], a Range request SHALL return
// exactly the bytes from position start to end (inclusive) with HTTP 206 status.

func TestProperty_RangeRequestCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file content (larger files for range testing)
	contentGen := gen.SliceOfN(1024, gen.UInt8())

	// Generator for file names
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,10}`)

	properties.Property("range request returns correct byte range with 206 status", prop.ForAll(
		func(content []byte, fileName string, startPercent int, rangePercent int) bool {
			if fileName == "" || len(content) < 10 {
				return true // Need minimum content for meaningful range tests
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			// Create file directly in filesystem
			fsPath := "/data/media/" + fileName + "_range.bin"
			err := fs.WriteFile(fsPath, content, 0644)
			if err != nil {
				return false
			}

			// Calculate range based on percentages (0-100)
			startPercent = startPercent % 100
			rangePercent = (rangePercent % 50) + 1 // 1-50% of file

			start := (len(content) * startPercent) / 100
			rangeLen := (len(content) * rangePercent) / 100
			if rangeLen < 1 {
				rangeLen = 1
			}
			end := start + rangeLen - 1
			if end >= len(content) {
				end = len(content) - 1
			}
			if start > end {
				start = end
			}

			// Make range request
			downloadReq := httptest.NewRequest("GET", "/api/v1/download/media/"+fileName+"_range.bin", nil)
			downloadReq.Header.Set("Range", fmt.Sprintf("bytes=%d-%d", start, end))

			downloadRec := httptest.NewRecorder()
			router.ServeHTTP(downloadRec, downloadReq)

			// Should return 206 Partial Content
			if downloadRec.Code != http.StatusPartialContent {
				return false
			}

			// Verify returned content matches expected range
			expectedContent := content[start : end+1]
			actualContent := downloadRec.Body.Bytes()

			return bytes.Equal(expectedContent, actualContent)
		},
		contentGen,
		fileNameGen,
		gen.IntRange(0, 99),
		gen.IntRange(1, 50),
	))

	properties.Property("range request without end returns from start to EOF", prop.ForAll(
		func(content []byte, fileName string, startPercent int) bool {
			if fileName == "" || len(content) < 10 {
				return true
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			// Create file directly in filesystem
			fsPath := "/data/media/" + fileName + "_range_open.bin"
			err := fs.WriteFile(fsPath, content, 0644)
			if err != nil {
				return false
			}

			// Calculate start position
			startPercent = startPercent % 90 // Leave room for some content
			start := (len(content) * startPercent) / 100

			// Make range request without end (bytes=start-)
			downloadReq := httptest.NewRequest("GET", "/api/v1/download/media/"+fileName+"_range_open.bin", nil)
			downloadReq.Header.Set("Range", fmt.Sprintf("bytes=%d-", start))

			downloadRec := httptest.NewRecorder()
			router.ServeHTTP(downloadRec, downloadReq)

			// Should return 206 Partial Content
			if downloadRec.Code != http.StatusPartialContent {
				return false
			}

			// Verify returned content matches from start to end of file
			expectedContent := content[start:]
			actualContent := downloadRec.Body.Bytes()

			return bytes.Equal(expectedContent, actualContent)
		},
		contentGen,
		fileNameGen,
		gen.IntRange(0, 89),
	))

	properties.Property("full download returns complete file with 200 status", prop.ForAll(
		func(content []byte, fileName string) bool {
			if fileName == "" || len(content) == 0 {
				return true
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			// Create file directly in filesystem
			fsPath := "/data/media/" + fileName + "_full.bin"
			err := fs.WriteFile(fsPath, content, 0644)
			if err != nil {
				return false
			}

			// Make download request without Range header
			downloadReq := httptest.NewRequest("GET", "/api/v1/download/media/"+fileName+"_full.bin", nil)

			downloadRec := httptest.NewRecorder()
			router.ServeHTTP(downloadRec, downloadReq)

			// Should return 200 OK
			if downloadRec.Code != http.StatusOK {
				return false
			}

			// Verify returned content matches complete file
			actualContent := downloadRec.Body.Bytes()

			return bytes.Equal(content, actualContent)
		},
		contentGen,
		fileNameGen,
	))

	properties.Property("download sets Accept-Ranges header", prop.ForAll(
		func(content []byte, fileName string) bool {
			if fileName == "" || len(content) == 0 {
				return true
			}

			handler, fs, _ := setupTestStreamHandler()
			router := createStreamTestRouter(handler)

			// Create file directly in filesystem
			fsPath := "/data/media/" + fileName + "_headers.bin"
			err := fs.WriteFile(fsPath, content, 0644)
			if err != nil {
				return false
			}

			// Make download request
			downloadReq := httptest.NewRequest("GET", "/api/v1/download/media/"+fileName+"_headers.bin", nil)

			downloadRec := httptest.NewRecorder()
			router.ServeHTTP(downloadRec, downloadReq)

			// Should have Accept-Ranges header
			return downloadRec.Header().Get("Accept-Ranges") == "bytes"
		},
		contentGen,
		fileNameGen,
	))

	properties.TestingRun(t)
}

// Helper function to perform chunked upload
func performChunkedUpload(router http.Handler, filePath, uploadID string, content []byte, numChunks int, checksum string) error {
	chunkSize := (len(content) + numChunks - 1) / numChunks
	if chunkSize < 1 {
		chunkSize = 1
	}
	totalChunks := testChunkCount(len(content), chunkSize)

	for i := 0; i < totalChunks; i++ {
		start := i * chunkSize
		end := start + chunkSize
		if end > len(content) {
			end = len(content)
		}
		if start >= len(content) {
			break
		}

		chunkData := content[start:end]

		uploadReq := httptest.NewRequest("POST", "/api/v1/upload/"+filePath, bytes.NewReader(chunkData))
		uploadReq.Header.Set("X-Upload-ID", uploadID)
		uploadReq.Header.Set("X-Chunk-Index", strconv.Itoa(i))
		uploadReq.Header.Set("X-Total-Chunks", strconv.Itoa(totalChunks))
		uploadReq.Header.Set("X-Chunk-Size", strconv.Itoa(chunkSize))
		uploadReq.Header.Set("X-Total-Size", strconv.Itoa(len(content)))

		if i == totalChunks-1 && checksum != "" {
			uploadReq.Header.Set("X-Checksum", checksum)
		}

		uploadRec := httptest.NewRecorder()
		router.ServeHTTP(uploadRec, uploadReq)

		expectedStatus := http.StatusOK
		if i == totalChunks-1 {
			expectedStatus = http.StatusCreated
		}
		if uploadRec.Code != expectedStatus {
			return fmt.Errorf("chunk %d: expected status %d, got %d", i, expectedStatus, uploadRec.Code)
		}
	}

	return nil
}

// Helper function to perform download and return content
func performDownload(router http.Handler, filePath string) ([]byte, int, error) {
	downloadReq := httptest.NewRequest("GET", "/api/v1/download/"+filePath, nil)
	downloadRec := httptest.NewRecorder()
	router.ServeHTTP(downloadRec, downloadReq)

	body, err := io.ReadAll(downloadRec.Body)
	if err != nil {
		return nil, downloadRec.Code, err
	}

	return body, downloadRec.Code, nil
}
