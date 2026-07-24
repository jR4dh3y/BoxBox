package service

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"os"
	"runtime"
	"sync"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type blockingReader struct {
	data    *bytes.Reader
	started chan struct{}
	release chan struct{}
	once    sync.Once
}

func (r *blockingReader) Read(buffer []byte) (int, error) {
	r.once.Do(func() { close(r.started) })
	<-r.release
	return r.data.Read(buffer)
}

func TestUploadServiceAcceptsConcurrentOutOfOrderChunks(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	files := NewFileService(fsys, FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "media", Path: "/data/media",
	}}})
	uploads := NewUploadService(files, t.TempDir())

	content := []byte("abcdefgh")
	digest := sha256.Sum256(content)
	chunks := []struct {
		index    int
		body     []byte
		checksum string
	}{
		{index: 1, body: content[4:], checksum: "sha256:" + hex.EncodeToString(digest[:])},
		{index: 0, body: content[:4]},
	}

	start := make(chan struct{})
	errors := make(chan error, len(chunks))
	var wg sync.WaitGroup
	for _, part := range chunks {
		wg.Add(1)
		go func() {
			defer wg.Done()
			<-start
			_, err := uploads.AcceptChunk(context.Background(), "media/file.bin", UploadChunk{
				UploadID:    "concurrent-upload",
				ChunkIndex:  part.index,
				TotalChunks: 2,
				ChunkSize:   4,
				TotalSize:   int64(len(content)),
				Checksum:    part.checksum,
			}, bytes.NewReader(part.body))
			errors <- err
		}()
	}
	close(start)
	wg.Wait()
	close(errors)
	for err := range errors {
		if err != nil {
			t.Fatal(err)
		}
	}

	got, err := fsys.ReadFile("/data/media/file.bin")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, content) {
		t.Fatalf("uploaded content = %q, want %q", got, content)
	}
}

func TestUploadCleanupPreservesActiveSession(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	files := NewFileService(fsys, FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "media", Path: "/data/media",
	}}})
	uploads := NewUploadService(files, t.TempDir()).(*uploadService)

	content := []byte("abcdefgh")
	digest := sha256.Sum256(content)
	chunk := UploadChunk{
		UploadID:    "active-upload",
		ChunkIndex:  0,
		TotalChunks: 2,
		ChunkSize:   4,
		TotalSize:   int64(len(content)),
	}
	if _, err := uploads.AcceptChunk(
		context.Background(),
		"media/file.bin",
		chunk,
		bytes.NewReader(content[:4]),
	); err != nil {
		t.Fatal(err)
	}

	uploads.mu.RLock()
	session := uploads.sessions[chunk.UploadID]
	uploads.mu.RUnlock()
	session.mu.Lock()
	session.lastActivity = time.Now().Add(-config.SessionTimeout - time.Hour)
	session.mu.Unlock()

	reader := &blockingReader{
		data:    bytes.NewReader(content[4:]),
		started: make(chan struct{}),
		release: make(chan struct{}),
	}
	result := make(chan error, 1)
	go func() {
		chunk.ChunkIndex = 1
		chunk.Checksum = "sha256:" + hex.EncodeToString(digest[:])
		_, err := uploads.AcceptChunk(
			context.Background(),
			"media/file.bin",
			chunk,
			reader,
		)
		result <- err
	}()

	<-reader.started
	uploads.cleanupExpired()
	if _, err := os.Stat(session.tempDir); err != nil {
		t.Fatalf("active session directory removed during cleanup: %v", err)
	}
	close(reader.release)
	if err := <-result; err != nil {
		t.Fatal(err)
	}

	got, err := fsys.ReadFile("/data/media/file.bin")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, content) {
		t.Fatalf("uploaded content = %q, want %q", got, content)
	}
}

func TestUploadFinalizationPreservesPinnedSession(t *testing.T) {
	tests := []struct {
		name          string
		checksum      func([sha256.Size]byte) string
		wantErr       error
		wantCompleted bool
	}{
		{
			name: "success",
			checksum: func(digest [sha256.Size]byte) string {
				return "sha256:" + hex.EncodeToString(digest[:])
			},
			wantCompleted: true,
		},
		{
			name:     "assembly failure",
			checksum: func([sha256.Size]byte) string { return "sha256:invalid" },
			wantErr:  ErrChecksumMismatch,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
				t.Fatal(err)
			}
			files := NewFileService(fsys, FileServiceConfig{MountPoints: []model.MountPoint{{
				Name: "media", Path: "/data/media",
			}}})
			uploads := NewUploadService(files, t.TempDir()).(*uploadService)

			content := []byte("abcdefgh")
			digest := sha256.Sum256(content)
			chunk := UploadChunk{
				UploadID:    "pinned-upload",
				ChunkIndex:  0,
				TotalChunks: 2,
				ChunkSize:   4,
				TotalSize:   int64(len(content)),
			}
			if _, err := uploads.AcceptChunk(
				context.Background(),
				"media/file.bin",
				chunk,
				bytes.NewReader(content[:4]),
			); err != nil {
				t.Fatal(err)
			}

			reader := &blockingReader{
				data:    bytes.NewReader(content[4:]),
				started: make(chan struct{}),
				release: make(chan struct{}),
			}
			chunk.ChunkIndex = 1
			chunk.Checksum = test.checksum(digest)
			results := make(chan error, 2)
			go func() {
				_, err := uploads.AcceptChunk(
					context.Background(),
					"media/file.bin",
					chunk,
					reader,
				)
				results <- err
			}()
			<-reader.started
			go func() {
				_, err := uploads.AcceptChunk(
					context.Background(),
					"media/file.bin",
					chunk,
					bytes.NewReader(content[4:]),
				)
				results <- err
			}()

			uploads.mu.RLock()
			session := uploads.sessions[chunk.UploadID]
			uploads.mu.RUnlock()
			deadline := time.Now().Add(time.Second)
			for {
				session.mu.RLock()
				activeRequests := session.activeRequests
				session.mu.RUnlock()
				if activeRequests == 2 {
					break
				}
				if time.Now().After(deadline) {
					t.Fatal("second upload request was not pinned")
				}
				runtime.Gosched()
			}

			close(reader.release)
			for range 2 {
				err := <-results
				if !errors.Is(err, test.wantErr) {
					t.Fatalf("AcceptChunk() error = %v, want %v", err, test.wantErr)
				}
			}

			if test.wantCompleted {
				got, err := fsys.ReadFile("/data/media/file.bin")
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(got, content) {
					t.Fatalf("uploaded content = %q, want %q", got, content)
				}
			}
		})
	}
}
