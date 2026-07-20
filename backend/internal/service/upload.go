package service

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
)

var (
	ErrUploadConflict      = errors.New("upload session metadata conflict")
	ErrUploadNotFound      = errors.New("upload session not found")
	ErrChunkSizeMismatch   = errors.New("chunk body size mismatch")
	ErrChecksumMismatch    = errors.New("checksum mismatch")
	ErrFinalChecksumNeeded = errors.New("final chunk checksum required")
)

type UploadChunk struct {
	UploadID    string
	ChunkIndex  int
	TotalChunks int
	ChunkSize   int64
	TotalSize   int64
	Checksum    string
}

func (chunk UploadChunk) ExpectedSize() int64 {
	if chunk.ChunkIndex == chunk.TotalChunks-1 {
		return chunk.TotalSize - int64(chunk.ChunkIndex)*chunk.ChunkSize
	}
	return chunk.ChunkSize
}

type UploadResult struct {
	UploadID       string `json:"uploadId"`
	ChunkIndex     int    `json:"chunkIndex"`
	ReceivedChunks int    `json:"receivedChunks"`
	TotalChunks    int    `json:"totalChunks"`
	Complete       bool   `json:"complete"`
	Path           string `json:"path,omitempty"`
}

type UploadStatus struct {
	UploadID       string    `json:"uploadId"`
	Path           string    `json:"path"`
	TotalChunks    int       `json:"totalChunks"`
	ReceivedChunks int       `json:"receivedChunks"`
	MissingChunks  []int     `json:"missingChunks"`
	Complete       bool      `json:"complete"`
	CreatedAt      time.Time `json:"createdAt"`
	LastActivity   time.Time `json:"lastActivity"`
}

type UploadService interface {
	StartCleanup(context.Context)
	StopCleanup()
	AcceptChunk(context.Context, string, UploadChunk, io.Reader) (*UploadResult, error)
	Status(string) (*UploadStatus, error)
}

type uploadSession struct {
	id             string
	path           string
	totalChunks    int
	chunkSize      int64
	totalSize      int64
	receivedChunks map[int]bool
	expectedHash   string
	tempDir        string
	createdAt      time.Time
	lastActivity   time.Time
	processMu      sync.Mutex
	mu             sync.RWMutex
}

type uploadService struct {
	files    FileStreamer
	tempRoot string
	sessions map[string]*uploadSession
	mu       sync.RWMutex
	cancel   context.CancelFunc
	wg       sync.WaitGroup
}

func NewUploadService(files FileStreamer, tempRoot string) UploadService {
	if tempRoot == "" {
		tempRoot = os.TempDir()
	}
	return &uploadService{
		files:    files,
		tempRoot: tempRoot,
		sessions: make(map[string]*uploadSession),
	}
}

func (s *uploadService) StartCleanup(parent context.Context) {
	s.mu.Lock()
	if s.cancel != nil {
		s.mu.Unlock()
		return
	}
	ctx, cancel := context.WithCancel(parent)
	s.cancel = cancel
	s.wg.Add(1)
	s.mu.Unlock()

	go func() {
		defer s.wg.Done()
		ticker := time.NewTicker(config.SessionCleanupInterval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				s.cleanupExpired()
			}
		}
	}()
}

func (s *uploadService) StopCleanup() {
	s.mu.Lock()
	cancel := s.cancel
	s.cancel = nil
	s.mu.Unlock()
	if cancel != nil {
		cancel()
		s.wg.Wait()
	}
}

func (s *uploadService) AcceptChunk(
	ctx context.Context,
	path string,
	chunk UploadChunk,
	body io.Reader,
) (*UploadResult, error) {
	mount, destination, err := s.files.ResolvePath(path)
	if err != nil {
		return nil, err
	}
	if mount.ReadOnly {
		return nil, ErrPermissionDenied
	}
	session, err := s.getOrCreate(path, chunk)
	if err != nil {
		return nil, err
	}
	session.processMu.Lock()
	defer session.processMu.Unlock()

	if chunk.ChunkIndex == chunk.TotalChunks-1 && chunk.Checksum == "" {
		return nil, ErrFinalChecksumNeeded
	}
	if chunk.ChunkIndex == chunk.TotalChunks-1 {
		if err := session.setExpectedHash(strings.TrimPrefix(chunk.Checksum, "sha256:")); err != nil {
			return nil, err
		}
	}
	if session.hasChunk(chunk.ChunkIndex) {
		return session.result(chunk.ChunkIndex, ""), nil
	}

	chunkPath := filepath.Join(session.tempDir, fmt.Sprintf("chunk_%d", chunk.ChunkIndex))
	chunkFile, err := os.OpenFile(chunkPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600)
	if err != nil {
		return nil, fmt.Errorf("create chunk: %w", err)
	}
	expectedSize := chunk.ExpectedSize()
	written, copyErr := io.Copy(chunkFile, io.LimitReader(body, expectedSize+1))
	closeErr := chunkFile.Close()
	if copyErr != nil || closeErr != nil {
		_ = os.Remove(chunkPath)
		return nil, fmt.Errorf("write chunk: %w", errors.Join(copyErr, closeErr))
	}
	if written != expectedSize {
		_ = os.Remove(chunkPath)
		return nil, ErrChunkSizeMismatch
	}
	if err := ctx.Err(); err != nil {
		_ = os.Remove(chunkPath)
		return nil, err
	}

	session.markChunk(chunk.ChunkIndex)
	if !session.complete() {
		return session.result(chunk.ChunkIndex, ""), nil
	}

	if err := s.assemble(ctx, session, destination, session.getExpectedHash()); err != nil {
		s.deleteSession(session.id)
		return nil, err
	}
	s.deleteSession(session.id)
	return session.result(chunk.ChunkIndex, path), nil
}

func (s *uploadService) Status(id string) (*UploadStatus, error) {
	s.mu.RLock()
	session := s.sessions[id]
	s.mu.RUnlock()
	if session == nil {
		return nil, ErrUploadNotFound
	}
	return session.status(), nil
}

func (s *uploadService) getOrCreate(path string, chunk UploadChunk) (*uploadSession, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if existing := s.sessions[chunk.UploadID]; existing != nil {
		if existing.path != path || existing.totalChunks != chunk.TotalChunks ||
			existing.chunkSize != chunk.ChunkSize || existing.totalSize != chunk.TotalSize {
			return nil, ErrUploadConflict
		}
		return existing, nil
	}

	if err := os.MkdirAll(s.tempRoot, 0o755); err != nil {
		return nil, err
	}
	tempDir, err := os.MkdirTemp(s.tempRoot, "upload-"+chunk.UploadID+"-")
	if err != nil {
		return nil, err
	}
	now := time.Now()
	session := &uploadSession{
		id:             chunk.UploadID,
		path:           path,
		totalChunks:    chunk.TotalChunks,
		chunkSize:      chunk.ChunkSize,
		totalSize:      chunk.TotalSize,
		receivedChunks: make(map[int]bool),
		tempDir:        tempDir,
		createdAt:      now,
		lastActivity:   now,
	}
	s.sessions[chunk.UploadID] = session
	return session, nil
}

func (s *uploadService) assemble(
	ctx context.Context,
	session *uploadSession,
	destination string,
	expectedChecksum string,
) error {
	fsys := s.files.GetFilesystem()
	if err := fsys.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		return err
	}
	temporary := destination + ".uploading." + session.id
	destinationFile, err := fsys.OpenFile(temporary, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o644)
	if err != nil {
		return err
	}
	complete := false
	defer func() {
		_ = destinationFile.Close()
		if !complete {
			_ = fsys.Remove(temporary)
		}
	}()

	hasher := sha256.New()
	writer := io.MultiWriter(destinationFile, hasher)
	buffer := make([]byte, config.FileCopyBufferSize)
	for index := 0; index < session.totalChunks; index++ {
		if err := ctx.Err(); err != nil {
			return err
		}
		chunkFile, err := os.Open(filepath.Join(session.tempDir, fmt.Sprintf("chunk_%d", index)))
		if err != nil {
			return err
		}
		_, copyErr := io.CopyBuffer(writer, chunkFile, buffer)
		closeErr := chunkFile.Close()
		if copyErr != nil || closeErr != nil {
			return errors.Join(copyErr, closeErr)
		}
	}

	actualChecksum := hex.EncodeToString(hasher.Sum(nil))
	if actualChecksum != expectedChecksum {
		return fmt.Errorf("%w: expected %s, got %s", ErrChecksumMismatch, expectedChecksum, actualChecksum)
	}
	if err := destinationFile.Close(); err != nil {
		return err
	}
	if err := fsys.Rename(temporary, destination); err != nil {
		return err
	}
	complete = true
	return nil
}

func (s *uploadService) cleanupExpired() {
	s.mu.Lock()
	defer s.mu.Unlock()
	now := time.Now()
	for id, session := range s.sessions {
		session.mu.RLock()
		expired := now.Sub(session.lastActivity) > config.SessionTimeout
		session.mu.RUnlock()
		if expired {
			_ = os.RemoveAll(session.tempDir)
			delete(s.sessions, id)
		}
	}
}

func (s *uploadService) deleteSession(id string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if session := s.sessions[id]; session != nil {
		_ = os.RemoveAll(session.tempDir)
		delete(s.sessions, id)
	}
}

func (s *uploadSession) hasChunk(index int) bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.receivedChunks[index]
}

func (s *uploadSession) setExpectedHash(hash string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.expectedHash != "" && s.expectedHash != hash {
		return ErrUploadConflict
	}
	s.expectedHash = hash
	s.lastActivity = time.Now()
	return nil
}

func (s *uploadSession) getExpectedHash() string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.expectedHash
}

func (s *uploadSession) markChunk(index int) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.receivedChunks[index] = true
	s.lastActivity = time.Now()
}

func (s *uploadSession) complete() bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.receivedChunks) == s.totalChunks
}

func (s *uploadSession) result(chunkIndex int, path string) *UploadResult {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return &UploadResult{
		UploadID:       s.id,
		ChunkIndex:     chunkIndex,
		ReceivedChunks: len(s.receivedChunks),
		TotalChunks:    s.totalChunks,
		Complete:       len(s.receivedChunks) == s.totalChunks,
		Path:           path,
	}
}

func (s *uploadSession) status() *UploadStatus {
	s.mu.RLock()
	defer s.mu.RUnlock()
	missing := make([]int, 0, s.totalChunks-len(s.receivedChunks))
	for index := 0; index < s.totalChunks; index++ {
		if !s.receivedChunks[index] {
			missing = append(missing, index)
		}
	}
	return &UploadStatus{
		UploadID:       s.id,
		Path:           s.path,
		TotalChunks:    s.totalChunks,
		ReceivedChunks: len(s.receivedChunks),
		MissingChunks:  missing,
		Complete:       len(s.receivedChunks) == s.totalChunks,
		CreatedAt:      s.createdAt,
		LastActivity:   s.lastActivity,
	}
}
