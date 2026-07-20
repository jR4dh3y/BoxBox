// Package handler provides HTTP handlers for the BoxBox API.
package handler

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

var errUploadTooLarge = errors.New("upload too large")

// StreamHandler handles streaming upload and download operations
type StreamHandler struct {
	fileService      service.FileService
	uploadService    service.UploadService
	thumbnailService service.ThumbnailService
	chunkSizeMB      int
	maxUploadBytes   int64
}

// NewStreamHandler creates a new stream handler
func NewStreamHandler(fileService service.FileService, chunkSizeMB int, maxUploadMB int) *StreamHandler {
	if chunkSizeMB <= 0 {
		chunkSizeMB = config.DefaultChunkSizeMB
	}
	if maxUploadMB <= 0 {
		maxUploadMB = config.DefaultMaxUploadMB
	}
	return &StreamHandler{
		fileService:      fileService,
		uploadService:    service.NewUploadService(fileService, config.DefaultUploadTempDir),
		thumbnailService: service.NewThumbnailService(fileService),
		chunkSizeMB:      chunkSizeMB,
		maxUploadBytes:   int64(maxUploadMB) * 1024 * 1024,
	}
}

// RegisterRoutes registers stream routes on the given router
func (h *StreamHandler) RegisterRoutes(r chi.Router) {
	r.Get("/download/*", h.Download)
	r.Get("/preview/*", h.Preview)
	r.Get("/thumbnail/*", h.Thumbnail)
	r.Post("/upload/*", h.Upload)
	r.Get("/upload/status/*", h.UploadStatus)
}

// Thumbnail returns a bounded JPEG rendition for file-grid previews.
func (h *StreamHandler) Thumbnail(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	width := parseThumbnailDimension(r.URL.Query().Get("width"), 224)
	height := parseThumbnailDimension(r.URL.Query().Get("height"), 144)
	thumbnail, err := h.thumbnailService.Render(
		r.Context(),
		path,
		width,
		height,
		r.Header.Get("If-None-Match"),
	)
	if errors.Is(err, service.ErrThumbnailNotModified) {
		w.WriteHeader(http.StatusNotModified)
		return
	}
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	w.Header().Set("Content-Type", "image/jpeg")
	w.Header().Set("Cache-Control", "private, max-age=3600")
	w.Header().Set("ETag", thumbnail.ETag)
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(thumbnail.Content)
}

func parseThumbnailDimension(value string, fallback int) int {
	dimension, err := strconv.Atoi(value)
	if err != nil {
		return fallback
	}
	return min(512, max(16, dimension))
}

// StartCleanup starts the periodic cleanup of expired upload sessions
func (h *StreamHandler) StartCleanup(ctx context.Context) {
	h.uploadService.StartCleanup(ctx)
}

// StopCleanup stops the cleanup goroutine for upload sessions
func (h *StreamHandler) StopCleanup() {
	h.uploadService.StopCleanup()
}

// Download handles file download requests with Range header support
// GET /api/v1/stream/download/*path
func (h *StreamHandler) Download(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Open the file using the file service (uses filesystem abstraction)
	file, info, err := h.fileService.OpenFile(r.Context(), path)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	defer file.Close()

	// Detect MIME type using centralized utility
	mimeType := detectStreamMimeType(file, info.Name)

	// Set response headers
	w.Header().Set("Content-Type", mimeType)
	w.Header().Set("Content-Disposition", fmt.Sprintf(`attachment; filename="%s"`, info.Name))
	w.Header().Set("Accept-Ranges", "bytes")

	// Use http.ServeContent for efficient range-based streaming
	http.ServeContent(w, r, info.Name, info.ModTime, file)
}

// Preview handles file preview requests (inline viewing) with Range header support
// GET /api/v1/stream/preview/*path
func (h *StreamHandler) Preview(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Open the file using the file service (uses filesystem abstraction)
	file, info, err := h.fileService.OpenFile(r.Context(), path)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	defer file.Close()

	// Detect MIME type using extension + content sniffing fallback
	mimeType := detectStreamMimeType(file, info.Name)

	// Set response headers for inline viewing
	w.Header().Set("Content-Type", mimeType)
	w.Header().Set("Content-Disposition", fmt.Sprintf(`inline; filename="%s"`, info.Name))
	w.Header().Set("Accept-Ranges", "bytes")
	// Allow cross-origin requests for media playback
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Range")
	w.Header().Set("Access-Control-Expose-Headers", "Content-Length, Content-Range, Accept-Ranges")

	// Avoid response transformation by intermediate proxies/CDNs.
	w.Header().Set("Cache-Control", "no-transform")

	// Use http.ServeContent for efficient range-based streaming
	http.ServeContent(w, r, info.Name, info.ModTime, file)
}

// UploadRequest represents the headers for a chunk upload
type UploadRequest struct {
	UploadID    string
	ChunkIndex  int
	TotalChunks int
	ChunkSize   int64
	TotalSize   int64
	Checksum    string // SHA256 checksum for final verification
}

// Upload handles chunked file uploads
// POST /api/v1/stream/upload/*path
// Headers:
//
//	X-Upload-ID: unique upload identifier
//	X-Chunk-Index: current chunk index (0-based)
//	X-Total-Chunks: total number of chunks
//	X-Chunk-Size: size of each chunk in bytes
//	X-Total-Size: total file size in bytes
//	X-Checksum: SHA256 checksum (only on final chunk)
func (h *StreamHandler) Upload(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Parse upload headers
	uploadReq, err := h.parseUploadHeaders(r)
	if err != nil {
		status := http.StatusBadRequest
		if errors.Is(err, errUploadTooLarge) {
			status = http.StatusRequestEntityTooLarge
		}
		writeError(w, err.Error(), model.ErrCodeValidationError, status)
		return
	}

	result, err := h.uploadService.AcceptChunk(r.Context(), path, service.UploadChunk{
		UploadID:    uploadReq.UploadID,
		ChunkIndex:  uploadReq.ChunkIndex,
		TotalChunks: uploadReq.TotalChunks,
		ChunkSize:   uploadReq.ChunkSize,
		TotalSize:   uploadReq.TotalSize,
		Checksum:    uploadReq.Checksum,
	}, r.Body)
	if err != nil {
		switch {
		case errors.Is(err, service.ErrPermissionDenied):
			writeError(w, "Mount point is read-only", model.ErrCodeReadOnly, http.StatusForbidden)
		case errors.Is(err, service.ErrUploadConflict):
			writeError(w, err.Error(), model.ErrCodeConflict, http.StatusConflict)
		case errors.Is(err, service.ErrFinalChecksumNeeded), errors.Is(err, service.ErrChunkSizeMismatch):
			writeError(w, err.Error(), model.ErrCodeValidationError, http.StatusBadRequest)
		case errors.Is(err, service.ErrChecksumMismatch):
			writeError(w, err.Error(), model.ErrCodeChecksumMismatch, http.StatusUnprocessableEntity)
		default:
			HandleServiceError(w, err)
		}
		return
	}

	status := http.StatusOK
	if result.Complete {
		status = http.StatusCreated
	}
	writeJSON(w, result, status)
}

// parseUploadHeaders parses and validates upload request headers
func (h *StreamHandler) parseUploadHeaders(r *http.Request) (*UploadRequest, error) {
	uploadID := r.Header.Get("X-Upload-ID")
	if uploadID == "" {
		return nil, errors.New("X-Upload-ID header is required")
	}
	if !isValidUploadID(uploadID) {
		return nil, errors.New("X-Upload-ID contains invalid characters")
	}

	chunkIndexStr := r.Header.Get("X-Chunk-Index")
	if chunkIndexStr == "" {
		return nil, errors.New("X-Chunk-Index header is required")
	}
	chunkIndex, err := strconv.Atoi(chunkIndexStr)
	if err != nil || chunkIndex < 0 {
		return nil, errors.New("X-Chunk-Index must be a non-negative integer")
	}

	totalChunksStr := r.Header.Get("X-Total-Chunks")
	if totalChunksStr == "" {
		return nil, errors.New("X-Total-Chunks header is required")
	}
	totalChunks, err := strconv.Atoi(totalChunksStr)
	if err != nil || totalChunks < 1 {
		return nil, errors.New("X-Total-Chunks must be a positive integer")
	}

	if chunkIndex >= totalChunks {
		return nil, errors.New("X-Chunk-Index must be less than X-Total-Chunks")
	}

	chunkSizeStr := r.Header.Get("X-Chunk-Size")
	if chunkSizeStr == "" {
		return nil, errors.New("X-Chunk-Size header is required")
	}
	chunkSize, err := strconv.ParseInt(chunkSizeStr, 10, 64)
	if err != nil || chunkSize < 1 {
		return nil, errors.New("X-Chunk-Size must be a positive integer")
	}

	totalSizeStr := r.Header.Get("X-Total-Size")
	if totalSizeStr == "" {
		return nil, errors.New("X-Total-Size header is required")
	}
	totalSize, err := strconv.ParseInt(totalSizeStr, 10, 64)
	if err != nil || totalSize < 1 {
		return nil, errors.New("X-Total-Size must be a positive integer")
	}
	if h.maxUploadBytes > 0 && totalSize > h.maxUploadBytes {
		return nil, fmt.Errorf("%w: maximum size is %d bytes", errUploadTooLarge, h.maxUploadBytes)
	}

	expectedTotalChunks := (totalSize + chunkSize - 1) / chunkSize
	if int64(totalChunks) != expectedTotalChunks {
		return nil, errors.New("X-Total-Chunks does not match X-Total-Size and X-Chunk-Size")
	}

	expectedChunkSize := chunkSize
	if chunkIndex == totalChunks-1 {
		expectedChunkSize = totalSize - int64(chunkIndex)*chunkSize
	}
	if expectedChunkSize < 1 || expectedChunkSize > chunkSize {
		return nil, errors.New("invalid upload chunk metadata")
	}

	if r.ContentLength > expectedChunkSize {
		return nil, fmt.Errorf("%w: request body exceeds expected chunk size", errUploadTooLarge)
	}
	if r.ContentLength >= 0 && r.ContentLength != expectedChunkSize {
		return nil, errors.New("request body size does not match upload metadata")
	}

	// Final chunks are rejected without this after session metadata is validated.
	checksum := r.Header.Get("X-Checksum")

	return &UploadRequest{
		UploadID:    uploadID,
		ChunkIndex:  chunkIndex,
		TotalChunks: totalChunks,
		ChunkSize:   chunkSize,
		TotalSize:   totalSize,
		Checksum:    checksum,
	}, nil
}

func isValidUploadID(uploadID string) bool {
	if len(uploadID) > 128 {
		return false
	}

	for _, r := range uploadID {
		switch {
		case r >= 'a' && r <= 'z':
		case r >= 'A' && r <= 'Z':
		case r >= '0' && r <= '9':
		case r == '.', r == '_', r == '-':
		default:
			return false
		}
	}

	return true
}

func detectStreamMimeType(file io.ReadSeeker, filename string) string {
	if mimeType := fileutil.DetectMimeType(filename); mimeType != "application/octet-stream" {
		return mimeType
	}

	buf := make([]byte, 512)
	n, readErr := file.Read(buf)
	if _, seekErr := file.Seek(0, io.SeekStart); seekErr != nil {
		return "application/octet-stream"
	}
	if readErr != nil && !errors.Is(readErr, io.EOF) {
		return "application/octet-stream"
	}
	if n > 0 {
		if detected := http.DetectContentType(buf[:n]); detected != "" {
			return detected
		}
	}

	return "application/octet-stream"
}

// UploadStatus returns the status of an upload session
// GET /api/v1/stream/upload/status/*path
func (h *StreamHandler) UploadStatus(w http.ResponseWriter, r *http.Request) {
	uploadID := r.URL.Query().Get("uploadId")
	if uploadID == "" {
		writeError(w, "uploadId query parameter is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	status, err := h.uploadService.Status(uploadID)
	if errors.Is(err, service.ErrUploadNotFound) {
		writeError(w, "Upload session not found", model.ErrCodeNotFound, http.StatusNotFound)
		return
	}
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, status, http.StatusOK)
}
