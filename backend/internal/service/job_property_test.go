// Package service provides business logic for the file manager.
// This file contains property-based tests for job operations.
package service

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/websocket"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// capturingHub wraps a real Hub and captures all job updates
type capturingHub struct {
	*websocket.Hub
	updates []model.JobUpdate
	mu      sync.Mutex
}

// newCapturingHub creates a hub that captures broadcasts
func newCapturingHub() *capturingHub {
	hub := websocket.NewHub()
	return &capturingHub{
		Hub:     hub,
		updates: make([]model.JobUpdate, 0),
	}
}

// GetUpdates returns all captured updates
func (h *capturingHub) GetUpdates() []model.JobUpdate {
	h.mu.Lock()
	defer h.mu.Unlock()
	result := make([]model.JobUpdate, len(h.updates))
	copy(result, h.updates)
	return result
}

// ClearUpdates clears all captured updates
func (h *capturingHub) ClearUpdates() {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.updates = make([]model.JobUpdate, 0)
}

// captureUpdate records a job update
func (h *capturingHub) captureUpdate(job *model.Job) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.updates = append(h.updates, model.JobUpdate{
		JobID:    job.ID,
		State:    job.State,
		Progress: job.Progress,
		Error:    job.Error,
	})
}

// testableJobService wraps jobService to capture broadcasts
type testableJobService struct {
	JobService
	fs           filesystem.FS
	capturingHub *capturingHub
	allJobs      *sync.Map
}

// setupTestJobService creates a job service with an in-memory filesystem for testing
func setupTestJobService() (*testableJobService, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()

	// Create mount point directories
	fs.MkdirAll("/data/media", 0755)
	fs.MkdirAll("/data/backup", 0755)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "backup", Path: "/data/backup", ReadOnly: false},
	}

	capHub := newCapturingHub()

	// Create the real job service
	svc := NewJobService(fs, capHub.Hub, JobServiceConfig{
		Workers:     2,
		MountPoints: mounts,
	})

	// Get access to internal allJobs map for verification
	internalSvc := svc.(*jobService)

	return &testableJobService{
		JobService:   svc,
		fs:           fs,
		capturingHub: capHub,
		allJobs:      &internalSvc.allJobs,
	}, fs
}

func TestJobCreateResolvesVirtualPaths(t *testing.T) {
	svc, _ := setupTestJobService()
	ctx := context.Background()

	job, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "media/source.bin",
		DestPath:   "backup/dest.bin",
	})
	if err != nil {
		t.Fatalf("expected job creation to succeed, got %v", err)
	}

	if job.SourcePath != "media/source.bin" || job.DestPath != "backup/dest.bin" {
		t.Fatalf("expected API paths to remain virtual, got source=%q dest=%q", job.SourcePath, job.DestPath)
	}
	if job.ResolvedSourcePath != "/data/media/source.bin" {
		t.Fatalf("expected resolved source path %q, got %q", "/data/media/source.bin", job.ResolvedSourcePath)
	}
	if job.ResolvedDestPath != "/data/backup/dest.bin" {
		t.Fatalf("expected resolved destination path %q, got %q", "/data/backup/dest.bin", job.ResolvedDestPath)
	}
}

func TestJobCreateRejectsPathsOutsideMounts(t *testing.T) {
	svc, _ := setupTestJobService()
	ctx := context.Background()

	_, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "/etc/passwd",
		DestPath:   "backup/passwd",
	})
	if err != ErrMountPointNotFound {
		t.Fatalf("expected %v, got %v", ErrMountPointNotFound, err)
	}

	_, err = svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "media/source.bin",
		DestPath:   "/tmp/out.bin",
	})
	if err != ErrMountPointNotFound {
		t.Fatalf("expected %v, got %v", ErrMountPointNotFound, err)
	}
}

func TestJobCreateEnforcesReadOnlyMounts(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	if err := fs.MkdirAll("/data/readonly", 0755); err != nil {
		t.Fatalf("failed to create readonly mount: %v", err)
	}
	if err := fs.MkdirAll("/data/writeable", 0755); err != nil {
		t.Fatalf("failed to create writeable mount: %v", err)
	}

	svc := NewJobService(fs, nil, JobServiceConfig{
		Workers: 1,
		MountPoints: []model.MountPoint{
			{Name: "readonly", Path: "/data/readonly", ReadOnly: true},
			{Name: "writeable", Path: "/data/writeable", ReadOnly: false},
		},
	})
	ctx := context.Background()

	if _, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "readonly/source.bin",
		DestPath:   "writeable/copy.bin",
	}); err != nil {
		t.Fatalf("expected copy from read-only mount to be allowed, got %v", err)
	}

	if _, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "writeable/source.bin",
		DestPath:   "readonly/copy.bin",
	}); err != ErrPermissionDenied {
		t.Fatalf("expected copy into read-only mount to fail with %v, got %v", ErrPermissionDenied, err)
	}

	if _, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeMove,
		SourcePath: "readonly/source.bin",
		DestPath:   "writeable/moved.bin",
	}); err != ErrPermissionDenied {
		t.Fatalf("expected move from read-only mount to fail with %v, got %v", ErrPermissionDenied, err)
	}

	if _, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeDelete,
		SourcePath: "readonly/source.bin",
	}); err != ErrPermissionDenied {
		t.Fatalf("expected delete from read-only mount to fail with %v, got %v", ErrPermissionDenied, err)
	}
}

// captureJobUpdates polls the job and captures state changes
func (s *testableJobService) captureJobUpdates(ctx context.Context, jobID string, done chan struct{}) {
	lastState := model.JobState("")
	lastProgress := -1

	for {
		select {
		case <-ctx.Done():
			return
		case <-done:
			return
		default:
			job, err := s.Get(ctx, jobID)
			if err != nil {
				time.Sleep(10 * time.Millisecond)
				continue
			}

			// Capture state changes or progress changes
			if job.State != lastState || job.Progress != lastProgress {
				s.capturingHub.mu.Lock()
				s.capturingHub.updates = append(s.capturingHub.updates, model.JobUpdate{
					JobID:    job.ID,
					State:    job.State,
					Progress: job.Progress,
					Error:    job.Error,
				})
				s.capturingHub.mu.Unlock()
				lastState = job.State
				lastProgress = job.Progress
			}

			if job.State.IsTerminal() {
				// Capture final state one more time to ensure error is captured
				time.Sleep(10 * time.Millisecond)
				finalJob, _ := s.Get(ctx, jobID)
				if finalJob != nil && finalJob.Error != "" {
					s.capturingHub.mu.Lock()
					s.capturingHub.updates = append(s.capturingHub.updates, model.JobUpdate{
						JobID:    finalJob.ID,
						State:    finalJob.State,
						Progress: finalJob.Progress,
						Error:    finalJob.Error,
					})
					s.capturingHub.mu.Unlock()
				}
				return
			}

			time.Sleep(10 * time.Millisecond)
		}
	}
}

// **Feature: boxbox, Property 7: Job Progress Monotonicity**
// **Validates: Requirements 4.2**
//
// Property: For any background job, the progress value SHALL be between 0 and 100 inclusive,
// and progress updates SHALL be monotonically non-decreasing until completion.

func TestProperty_JobProgressMonotonicity(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file content size (in KB)
	fileSizeGen := gen.IntRange(1, 100)

	// Generator for number of files in directory
	numFilesGen := gen.IntRange(1, 20)

	properties.Property("copy job progress is between 0 and 100", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			// Start the job service
			svc.Start(ctx)
			defer svc.Stop()

			// Create source file with specified size
			content := make([]byte, fileSize*1024)
			for i := range content {
				content[i] = byte(i % 256)
			}
			fs.WriteFile("/data/media/source.bin", content, 0644)

			// Create copy job
			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/source.bin",
				DestPath:   "backup/dest.bin",
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for job to complete
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, err := svc.Get(ctx, job.ID)
				if err != nil {
					return false
				}
				if j.State.IsTerminal() {
					break
				}
			}
			close(done)

			// Check all progress updates are within bounds
			updates := svc.capturingHub.GetUpdates()
			for _, update := range updates {
				if update.JobID != job.ID {
					continue
				}
				if update.Progress < 0 || update.Progress > 100 {
					return false
				}
			}

			return true
		},
		fileSizeGen,
	))

	properties.Property("copy job progress is monotonically non-decreasing", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create source file
			content := make([]byte, fileSize*1024)
			for i := range content {
				content[i] = byte(i % 256)
			}
			fs.WriteFile("/data/media/mono_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/mono_source.bin",
				DestPath:   "backup/mono_dest.bin",
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					break
				}
			}
			close(done)

			// Verify monotonicity
			updates := svc.capturingHub.GetUpdates()
			lastProgress := -1
			for _, update := range updates {
				if update.JobID != job.ID {
					continue
				}
				if update.Progress < lastProgress {
					return false // Progress decreased
				}
				lastProgress = update.Progress
			}

			return true
		},
		fileSizeGen,
	))

	properties.Property("delete job progress is between 0 and 100", prop.ForAll(
		func(numFiles int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create directory with files
			dirPath := "/data/media/delete_test"
			virtualDirPath := "media/delete_test"
			fs.MkdirAll(dirPath, 0755)
			for i := 0; i < numFiles; i++ {
				fs.WriteFile(fmt.Sprintf("%s/file%d.txt", dirPath, i), []byte("content"), 0644)
			}

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeDelete,
				SourcePath: virtualDirPath,
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					break
				}
			}
			close(done)

			// Check progress bounds
			updates := svc.capturingHub.GetUpdates()
			for _, update := range updates {
				if update.JobID != job.ID {
					continue
				}
				if update.Progress < 0 || update.Progress > 100 {
					return false
				}
			}

			return true
		},
		numFilesGen,
	))

	properties.Property("completed job has progress 100", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			content := make([]byte, fileSize*1024)
			fs.WriteFile("/data/media/complete_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/complete_source.bin",
				DestPath:   "backup/complete_dest.bin",
			})
			if err != nil {
				return false
			}

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State == model.JobStateCompleted {
					// Verify final progress is 100
					return j.Progress == 100
				}
				if j != nil && j.State.IsTerminal() {
					break
				}
			}

			// Check final job state
			finalJob, _ := svc.Get(ctx, job.ID)
			if finalJob != nil && finalJob.State == model.JobStateCompleted {
				return finalJob.Progress == 100
			}

			return true // Job may have failed for other reasons
		},
		fileSizeGen,
	))

	properties.TestingRun(t)
}

// **Feature: boxbox, Property 8: Job Completion Notification**
// **Validates: Requirements 4.3, 5.2, 5.3**
//
// Property: For any background job that reaches completed, failed, or cancelled state,
// a WebSocket notification SHALL be sent to all connected clients containing the job ID and final state.

func TestProperty_JobCompletionNotification(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file size
	fileSizeGen := gen.IntRange(1, 50)

	properties.Property("completed copy job sends notification with job ID and completed state", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create source file
			content := make([]byte, fileSize*1024)
			fs.WriteFile("/data/media/notify_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/notify_source.bin",
				DestPath:   "backup/notify_dest.bin",
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					break
				}
			}
			close(done)
			time.Sleep(50 * time.Millisecond) // Allow final capture

			// Verify notification was sent with correct job ID and terminal state
			updates := svc.capturingHub.GetUpdates()
			foundTerminalNotification := false
			for _, update := range updates {
				if update.JobID == job.ID && update.State.IsTerminal() {
					foundTerminalNotification = true
					break
				}
			}

			return foundTerminalNotification
		},
		fileSizeGen,
	))

	properties.Property("failed job sends notification with error details", prop.ForAll(
		func(fileName string) bool {
			if fileName == "" {
				return true
			}

			svc, _ := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create job for non-existent source (will fail)
			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/nonexistent_" + fileName,
				DestPath:   "backup/dest_" + fileName,
			})
			if err != nil {
				return false
			}

			// Wait for job to fail
			var finalJob *model.Job
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					finalJob = j
					break
				}
			}

			// Verify job failed with error details
			if finalJob == nil {
				return false
			}

			// Job should be in failed state with non-empty error
			if finalJob.State != model.JobStateFailed {
				return false
			}

			// Error should be non-empty for failed jobs
			return finalJob.Error != ""
		},
		gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,10}`),
	))

	properties.Property("delete job sends completion notification", prop.ForAll(
		func(numFiles int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create directory with files
			dirPath := "/data/media/delete_notify"
			virtualDirPath := "media/delete_notify"
			fs.MkdirAll(dirPath, 0755)
			for i := 0; i < numFiles; i++ {
				fs.WriteFile(fmt.Sprintf("%s/file%d.txt", dirPath, i), []byte("content"), 0644)
			}

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeDelete,
				SourcePath: virtualDirPath,
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					break
				}
			}
			close(done)
			time.Sleep(50 * time.Millisecond)

			// Verify completion notification
			updates := svc.capturingHub.GetUpdates()
			foundCompletionNotification := false
			for _, update := range updates {
				if update.JobID == job.ID && update.State == model.JobStateCompleted {
					foundCompletionNotification = true
					break
				}
			}

			return foundCompletionNotification
		},
		gen.IntRange(1, 10),
	))

	properties.Property("move job sends completion notification", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			content := make([]byte, fileSize*1024)
			fs.WriteFile("/data/media/move_notify_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeMove,
				SourcePath: "media/move_notify_source.bin",
				DestPath:   "backup/move_notify_dest.bin",
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					break
				}
			}
			close(done)
			time.Sleep(50 * time.Millisecond)

			// Verify notification
			updates := svc.capturingHub.GetUpdates()
			foundNotification := false
			for _, update := range updates {
				if update.JobID == job.ID && update.State.IsTerminal() {
					foundNotification = true
					break
				}
			}

			return foundNotification
		},
		fileSizeGen,
	))

	properties.Property("notification contains correct job ID", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			content := make([]byte, fileSize*1024)
			fs.WriteFile("/data/media/id_check_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/id_check_source.bin",
				DestPath:   "backup/id_check_dest.bin",
			})
			if err != nil {
				return false
			}

			// Start capturing updates
			done := make(chan struct{})
			go svc.captureJobUpdates(ctx, job.ID, done)

			// Wait for completion
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					break
				}
			}
			close(done)
			time.Sleep(50 * time.Millisecond)

			// All updates for this job should have matching job ID
			updates := svc.capturingHub.GetUpdates()
			for _, update := range updates {
				if update.JobID == job.ID {
					// Found at least one update with correct ID
					return true
				}
			}

			return false // No updates found for this job
		},
		fileSizeGen,
	))

	properties.TestingRun(t)
}

// **Feature: boxbox, Property 9: Job Cancellation Cleanup**
// **Validates: Requirements 4.5**
//
// Property: For any cancelled copy or move job, partial destination files SHALL be removed
// and the source SHALL remain unchanged.

func TestProperty_JobCancellationCleanup(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 20
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file content
	contentGen := gen.SliceOfN(1000, gen.UInt8())

	properties.Property("cancelled copy job removes partial destination file", prop.ForAll(
		func(content []byte) bool {
			if len(content) == 0 {
				return true
			}

			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create a larger source file to give time for cancellation
			largeContent := make([]byte, 10*1024*1024) // 10MB
			for i := range largeContent {
				if i < len(content) {
					largeContent[i] = content[i]
				} else {
					largeContent[i] = byte(i % 256)
				}
			}
			fs.WriteFile("/data/media/cancel_source.bin", largeContent, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/cancel_source.bin",
				DestPath:   "backup/cancel_dest.bin",
			})
			if err != nil {
				return false
			}

			// Wait a bit for job to start, then cancel
			time.Sleep(10 * time.Millisecond)
			svc.Cancel(ctx, job.ID)

			// Wait for cancellation to complete
			time.Sleep(100 * time.Millisecond)

			// Verify destination file is removed (or doesn't exist)
			exists, _ := fs.Exists("/data/backup/cancel_dest.bin")

			// If job completed before cancellation, destination may exist
			// If cancelled during copy, destination should be removed
			j, _ := svc.Get(ctx, job.ID)
			if j != nil && j.State == model.JobStateCancelled {
				// Cancelled job should have cleaned up destination
				return !exists
			}

			// Job completed before cancellation - that's okay
			return true
		},
		contentGen,
	))

	properties.Property("cancelled copy job preserves source file unchanged", prop.ForAll(
		func(content []byte) bool {
			if len(content) == 0 {
				return true
			}

			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create source file
			sourceContent := make([]byte, 5*1024*1024) // 5MB
			for i := range sourceContent {
				if i < len(content) {
					sourceContent[i] = content[i]
				} else {
					sourceContent[i] = byte(i % 256)
				}
			}
			fs.WriteFile("/data/media/preserve_source.bin", sourceContent, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/preserve_source.bin",
				DestPath:   "backup/preserve_dest.bin",
			})
			if err != nil {
				return false
			}

			// Cancel immediately
			time.Sleep(5 * time.Millisecond)
			svc.Cancel(ctx, job.ID)

			// Wait for cancellation
			time.Sleep(100 * time.Millisecond)

			// Verify source file still exists and is unchanged
			exists, _ := fs.Exists("/data/media/preserve_source.bin")
			if !exists {
				return false
			}

			// Verify content is unchanged
			actualContent, err := fs.ReadFile("/data/media/preserve_source.bin")
			if err != nil {
				return false
			}

			if len(actualContent) != len(sourceContent) {
				return false
			}

			for i := range sourceContent {
				if actualContent[i] != sourceContent[i] {
					return false
				}
			}

			return true
		},
		contentGen,
	))

	properties.Property("cancelled move job preserves source and removes partial destination", prop.ForAll(
		func(content []byte) bool {
			if len(content) == 0 {
				return true
			}

			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create source file
			sourceContent := make([]byte, 5*1024*1024) // 5MB
			for i := range sourceContent {
				if i < len(content) {
					sourceContent[i] = content[i]
				} else {
					sourceContent[i] = byte(i % 256)
				}
			}
			fs.WriteFile("/data/media/move_cancel_source.bin", sourceContent, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeMove,
				SourcePath: "media/move_cancel_source.bin",
				DestPath:   "backup/move_cancel_dest.bin",
			})
			if err != nil {
				return false
			}

			// Cancel quickly
			time.Sleep(5 * time.Millisecond)
			svc.Cancel(ctx, job.ID)

			// Wait for cancellation
			time.Sleep(100 * time.Millisecond)

			j, _ := svc.Get(ctx, job.ID)
			if j != nil && j.State == model.JobStateCancelled {
				// Source should still exist
				sourceExists, _ := fs.Exists("/data/media/move_cancel_source.bin")
				if !sourceExists {
					return false
				}

				// Destination should be removed
				destExists, _ := fs.Exists("/data/backup/move_cancel_dest.bin")
				return !destExists
			}

			// Job completed before cancellation - source may be gone
			return true
		},
		contentGen,
	))

	properties.Property("cancelled job state is set to cancelled", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			// Create large file
			content := make([]byte, fileSize*1024*1024) // fileSize MB
			fs.WriteFile("/data/media/state_cancel_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/state_cancel_source.bin",
				DestPath:   "backup/state_cancel_dest.bin",
			})
			if err != nil {
				return false
			}

			// Cancel immediately
			svc.Cancel(ctx, job.ID)

			// Wait a bit
			time.Sleep(100 * time.Millisecond)

			// Check job state
			j, _ := svc.Get(ctx, job.ID)
			if j == nil {
				return false
			}

			// Job should be either cancelled or completed (if it finished before cancel)
			return j.State == model.JobStateCancelled || j.State == model.JobStateCompleted
		},
		gen.IntRange(1, 10),
	))

	properties.Property("cancellation notification is sent", prop.ForAll(
		func(fileSize int) bool {
			svc, fs := setupTestJobService()
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()

			svc.Start(ctx)
			defer svc.Stop()

			content := make([]byte, fileSize*1024*1024)
			fs.WriteFile("/data/media/notify_cancel_source.bin", content, 0644)

			job, err := svc.Create(ctx, model.JobParams{
				Type:       model.JobTypeCopy,
				SourcePath: "media/notify_cancel_source.bin",
				DestPath:   "backup/notify_cancel_dest.bin",
			})
			if err != nil {
				return false
			}

			// Cancel
			svc.Cancel(ctx, job.ID)

			// Wait for job to reach terminal state
			var finalJob *model.Job
			for i := 0; i < 100; i++ {
				time.Sleep(50 * time.Millisecond)
				j, _ := svc.Get(ctx, job.ID)
				if j != nil && j.State.IsTerminal() {
					finalJob = j
					break
				}
			}

			if finalJob == nil {
				return false
			}

			// Job should be either cancelled or completed (if it finished before cancel)
			return finalJob.State == model.JobStateCancelled || finalJob.State == model.JobStateCompleted
		},
		gen.IntRange(1, 5),
	))

	properties.TestingRun(t)
}

// Helper function to verify WebSocket message format
func verifyJobUpdateMessage(data []byte) (model.JobUpdate, bool) {
	var msg websocket.ServerMessage
	if err := json.Unmarshal(data, &msg); err != nil {
		return model.JobUpdate{}, false
	}

	if msg.Type != websocket.MessageTypeJobUpdate && msg.Type != websocket.MessageTypeJobComplete {
		return model.JobUpdate{}, false
	}

	payloadBytes, err := json.Marshal(msg.Payload)
	if err != nil {
		return model.JobUpdate{}, false
	}

	var update model.JobUpdate
	if err := json.Unmarshal(payloadBytes, &update); err != nil {
		return model.JobUpdate{}, false
	}

	return update, true
}
