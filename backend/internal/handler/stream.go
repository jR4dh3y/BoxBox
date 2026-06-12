// Package handler provides HTTP handlers for the file manager API.
package handler

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/homelab/filemanager/internal/config"
	"github.com/homelab/filemanager/internal/model"
	"github.com/homelab/filemanager/internal/pkg/fileutil"
	"github.com/homelab/filemanager/internal/service"
)

var errUploadTooLarge = errors.New("upload too large")

// StreamHandler handles streaming upload and download operations
type StreamHandler struct {
	fileService    service.FileService
	uploadManager  *UploadManager
	chunkSizeMB    int
	maxUploadBytes int64
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
		fileService:    fileService,
		uploadManager:  NewUploadManager(config.DefaultUploadTempDir),
		chunkSizeMB:    chunkSizeMB,
		maxUploadBytes: int64(maxUploadMB) * 1024 * 1024,
	}
}

// RegisterRoutes registers stream routes on the given router
func (h *StreamHandler) RegisterRoutes(r chi.Router) {
	r.Get("/download/*", h.Download)
	r.Get("/preview/*", h.Preview)
	r.Post("/upload/*", h.Upload)
	r.Get("/upload/status/*", h.UploadStatus)
}

// StartCleanup starts the periodic cleanup of expired upload sessions
func (h *StreamHandler) StartCleanup(ctx context.Context) {
	h.uploadManager.StartCleanup(ctx)
}

// StopCleanup stops the cleanup goroutine for upload sessions
func (h *StreamHandler) StopCleanup() {
	h.uploadManager.StopCleanup()
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

// UploadSession tracks the state of a chunked upload
type UploadSession struct {
	ID             string       `json:"id"`
	Path           string       `json:"path"`
	TotalChunks    int          `json:"totalChunks"`
	ChunkSize      int64        `json:"chunkSize"`
	TotalSize      int64        `json:"totalSize"`
	ReceivedChunks map[int]bool `json:"-"`
	TempDir        string       `json:"-"`
	CreatedAt      time.Time    `json:"createdAt"`
	LastActivity   time.Time    `json:"lastActivity"`
	mu             sync.RWMutex
}

// UploadManager manages active upload sessions
type UploadManager struct {
	sessions map[string]*UploadSession
	tempRoot string
	mu       sync.RWMutex
	stopCh   chan struct{}
	wg       sync.WaitGroup
}

// NewUploadManager creates a new upload manager
func NewUploadManager(tempRoot string) *UploadManager {
	if tempRoot == "" {
		tempRoot = os.TempDir()
	}
	return &UploadManager{
		sessions: make(map[string]*UploadSession),
		tempRoot: tempRoot,
		stopCh:   make(chan struct{}),
	}
}

// StartCleanup starts the periodic cleanup of expired sessions
func (m *UploadManager) StartCleanup(ctx context.Context) {
	m.wg.Add(1)
	go func() {
		defer m.wg.Done()
		ticker := time.NewTicker(config.SessionCleanupInterval)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				return
			case <-m.stopCh:
				return
			case <-ticker.C:
				m.cleanupExpiredSessions()
			}
		}
	}()
}

// StopCleanup stops the cleanup goroutine
func (m *UploadManager) StopCleanup() {
	close(m.stopCh)
	m.wg.Wait()
}

// cleanupExpiredSessions removes sessions that have been inactive for too long
func (m *UploadManager) cleanupExpiredSessions() {
	m.mu.Lock()
	defer m.mu.Unlock()

	now := time.Now()
	for id, session := range m.sessions {
		if now.Sub(session.LastActivity) > config.SessionTimeout {
			_ = os.RemoveAll(session.TempDir)
			delete(m.sessions, id)
		}
	}
}

// CreateSession creates a new upload session
func (m *UploadManager) CreateSession(id, path string, totalChunks int, chunkSize, totalSize int64) (*UploadSession, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Ensure upload temp root exists and create a per-upload temp directory.
	if err := os.MkdirAll(m.tempRoot, 0o755); err != nil {
		return nil, fmt.Errorf("failed to ensure upload temp root: %w", err)
	}
	tempDir, err := os.MkdirTemp(m.tempRoot, "upload-"+id+"-")
	if err != nil {
		return nil, fmt.Errorf("failed to create temp directory: %w", err)
	}

	session := &UploadSession{
		ID:             id,
		Path:           path,
		TotalChunks:    totalChunks,
		ChunkSize:      chunkSize,
		TotalSize:      totalSize,
		ReceivedChunks: make(map[int]bool),
		TempDir:        tempDir,
		CreatedAt:      time.Now(),
		LastActivity:   time.Now(),
	}

	m.sessions[id] = session
	return session, nil
}

// GetSession retrieves an upload session by ID
func (m *UploadManager) GetSession(id string) (*UploadSession, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	session, ok := m.sessions[id]
	return session, ok
}

// DeleteSession removes an upload session and cleans up temp files
func (m *UploadManager) DeleteSession(id string) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if session, ok := m.sessions[id]; ok {
		_ = os.RemoveAll(session.TempDir)
		delete(m.sessions, id)
	}
}

// MarkChunkReceived marks a chunk as received
func (s *UploadSession) MarkChunkReceived(index int) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.ReceivedChunks[index] = true
	s.LastActivity = time.Now()
}

// IsChunkReceived checks if a chunk has been received
func (s *UploadSession) IsChunkReceived(index int) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.ReceivedChunks[index]
}

// IsComplete checks if all chunks have been received
func (s *UploadSession) IsComplete() bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.ReceivedChunks) == s.TotalChunks
}

// GetReceivedCount returns the number of received chunks
func (s *UploadSession) GetReceivedCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.ReceivedChunks)
}

// GetMissingChunks returns a list of missing chunk indices
func (s *UploadSession) GetMissingChunks() []int {
	s.mu.RLock()
	defer s.mu.RUnlock()

	missing := make([]int, 0)
	for i := 0; i < s.TotalChunks; i++ {
		if !s.ReceivedChunks[i] {
			missing = append(missing, i)
		}
	}
	return missing
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

// UploadResponse represents the response for a chunk upload
type UploadResponse struct {
	UploadID       string `json:"uploadId"`
	ChunkIndex     int    `json:"chunkIndex"`
	ReceivedChunks int    `json:"receivedChunks"`
	TotalChunks    int    `json:"totalChunks"`
	Complete       bool   `json:"complete"`
	Path           string `json:"path,omitempty"`
}

// UploadStatusResponse represents the status of an upload session
type UploadStatusResponse struct {
	UploadID       string    `json:"uploadId"`
	Path           string    `json:"path"`
	TotalChunks    int       `json:"totalChunks"`
	ReceivedChunks int       `json:"receivedChunks"`
	MissingChunks  []int     `json:"missingChunks"`
	Complete       bool      `json:"complete"`
	CreatedAt      time.Time `json:"createdAt"`
	LastActivity   time.Time `json:"lastActivity"`
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

	// Validate path and check write permissions
	mount, fsPath, err := h.fileService.ResolvePath(path)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	if mount.ReadOnly {
		writeError(w, "Mount point is read-only", model.ErrCodeReadOnly, http.StatusForbidden)
		return
	}

	// Get or create upload session
	session, exists := h.uploadManager.GetSession(uploadReq.UploadID)
	if !exists {
		session, err = h.uploadManager.CreateSession(
			uploadReq.UploadID,
			path,
			uploadReq.TotalChunks,
			uploadReq.ChunkSize,
			uploadReq.TotalSize,
		)
		if err != nil {
			writeError(w, "Failed to create upload session", model.ErrCodeInternalError, http.StatusInternalServerError)
			return
		}
	} else if !session.matches(path, uploadReq) {
		writeError(w, "Upload session metadata does not match this request", model.ErrCodeConflict, http.StatusConflict)
		return
	}

	// Check if chunk was already received (for resumable uploads)
	if session.IsChunkReceived(uploadReq.ChunkIndex) {
		writeJSON(w, UploadResponse{
			UploadID:       session.ID,
			ChunkIndex:     uploadReq.ChunkIndex,
			ReceivedChunks: session.GetReceivedCount(),
			TotalChunks:    session.TotalChunks,
			Complete:       session.IsComplete(),
		}, http.StatusOK)
		return
	}

	// Save chunk to temp file
	chunkPath := filepath.Join(session.TempDir, fmt.Sprintf("chunk_%d", uploadReq.ChunkIndex))
	chunkFile, err := os.OpenFile(chunkPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600)
	if err != nil {
		writeError(w, "Failed to create chunk file", model.ErrCodeInternalError, http.StatusInternalServerError)
		return
	}

	limitedBody := http.MaxBytesReader(w, r.Body, uploadReq.ExpectedChunkSize())
	writtenBytes, err := io.Copy(chunkFile, limitedBody)
	_ = limitedBody.Close()
	chunkFile.Close()
	if err != nil {
		_ = os.Remove(chunkPath)
		var maxBytesErr *http.MaxBytesError
		if errors.As(err, &maxBytesErr) {
			writeError(w, "Chunk body exceeds declared chunk size", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge)
			return
		}
		writeError(w, "Failed to write chunk", model.ErrCodeInternalError, http.StatusInternalServerError)
		return
	}
	if writtenBytes != uploadReq.ExpectedChunkSize() {
		_ = os.Remove(chunkPath)
		writeError(w, "Chunk body size does not match upload metadata", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Mark chunk as received
	session.MarkChunkReceived(uploadReq.ChunkIndex)

	// Check if upload is complete
	if session.IsComplete() {
		// Assemble chunks into final file
		err = h.assembleChunks(session, fsPath, uploadReq.Checksum)
		if err != nil {
			h.uploadManager.DeleteSession(session.ID)
			if strings.Contains(err.Error(), "checksum") {
				writeError(w, err.Error(), model.ErrCodeChecksumMismatch, http.StatusUnprocessableEntity)
			} else {
				writeError(w, "Failed to assemble file: "+err.Error(), model.ErrCodeInternalError, http.StatusInternalServerError)
			}
			return
		}

		// Clean up session
		h.uploadManager.DeleteSession(session.ID)

		writeJSON(w, UploadResponse{
			UploadID:       session.ID,
			ChunkIndex:     uploadReq.ChunkIndex,
			ReceivedChunks: session.TotalChunks,
			TotalChunks:    session.TotalChunks,
			Complete:       true,
			Path:           path,
		}, http.StatusCreated)
		return
	}

	writeJSON(w, UploadResponse{
		UploadID:       session.ID,
		ChunkIndex:     uploadReq.ChunkIndex,
		ReceivedChunks: session.GetReceivedCount(),
		TotalChunks:    session.TotalChunks,
		Complete:       false,
	}, http.StatusOK)
}

// parseUploadHeaders parses and validates upload request headers
func (h *StreamHandler) parseUploadHeaders(r *http.Request) (*UploadRequest, error) {
	uploadID := r.Header.Get("X-Upload-ID")
	if uploadID == "" {
		return nil, errors.New("X-Upload-ID header is required")
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

	// Checksum is optional but required on final chunk for verification
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

func (r *UploadRequest) ExpectedChunkSize() int64 {
	if r.ChunkIndex == r.TotalChunks-1 {
		return r.TotalSize - int64(r.ChunkIndex)*r.ChunkSize
	}

	return r.ChunkSize
}

func (s *UploadSession) matches(path string, uploadReq *UploadRequest) bool {
	return s.Path == path &&
		s.TotalChunks == uploadReq.TotalChunks &&
		s.ChunkSize == uploadReq.ChunkSize &&
		s.TotalSize == uploadReq.TotalSize
}

// assembleChunks combines all chunks into the final file
func (h *StreamHandler) assembleChunks(session *UploadSession, destPath string, expectedChecksum string) error {
	// Get the filesystem from the file service
	fs := h.fileService.GetFilesystem()

	// Ensure parent directory exists
	parentDir := filepath.Dir(destPath)
	if err := fs.MkdirAll(parentDir, 0755); err != nil {
		return fmt.Errorf("failed to create parent directory: %w", err)
	}

	// Write into a temporary file in the destination directory, then rename atomically.
	tempDestPath := destPath + ".uploading." + session.ID
	destFile, err := fs.OpenFile(tempDestPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o644)
	if err != nil {
		return fmt.Errorf("failed to create destination file: %w", err)
	}
	cleanupTemp := true
	defer func() {
		_ = destFile.Close()
		if cleanupTemp {
			_ = fs.Remove(tempDestPath)
		}
	}()

	// Create hasher for checksum verification
	hasher := sha256.New()
	writer := io.MultiWriter(destFile, hasher)
	copyBuf := make([]byte, config.FileCopyBufferSize)

	// Assemble chunks in order
	for i := 0; i < session.TotalChunks; i++ {
		chunkPath := filepath.Join(session.TempDir, fmt.Sprintf("chunk_%d", i))
		chunkFile, err := os.Open(chunkPath)
		if err != nil {
			return fmt.Errorf("failed to open chunk %d: %w", i, err)
		}

		_, err = io.CopyBuffer(writer, chunkFile, copyBuf)
		chunkFile.Close()
		if err != nil {
			return fmt.Errorf("failed to copy chunk %d: %w", i, err)
		}
	}

	// Verify checksum if provided
	if expectedChecksum != "" {
		actualChecksum := hex.EncodeToString(hasher.Sum(nil))
		// Handle both with and without "sha256:" prefix
		expected := strings.TrimPrefix(expectedChecksum, "sha256:")
		if actualChecksum != expected {
			return fmt.Errorf("checksum mismatch: expected %s, got %s", expected, actualChecksum)
		}
	}

	if err := destFile.Close(); err != nil {
		return fmt.Errorf("failed to finalize destination file: %w", err)
	}
	if err := fs.Rename(tempDestPath, destPath); err != nil {
		return fmt.Errorf("failed to finalize uploaded file: %w", err)
	}
	cleanupTemp = false

	return nil
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

	session, exists := h.uploadManager.GetSession(uploadID)
	if !exists {
		writeError(w, "Upload session not found", model.ErrCodeNotFound, http.StatusNotFound)
		return
	}

	writeJSON(w, UploadStatusResponse{
		UploadID:       session.ID,
		Path:           session.Path,
		TotalChunks:    session.TotalChunks,
		ReceivedChunks: session.GetReceivedCount(),
		MissingChunks:  session.GetMissingChunks(),
		Complete:       session.IsComplete(),
		CreatedAt:      session.CreatedAt,
		LastActivity:   session.LastActivity,
	}, http.StatusOK)
}
