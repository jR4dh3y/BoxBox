// Package service provides business logic for the file manager.
package service

import (
	"context"
	"errors"
	"io"
	"path/filepath"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
	"github.com/jR4dh3y/BoxBox/backend/internal/websocket"
)

// Job service errors
var (
	ErrJobNotFound       = errors.New("job not found")
	ErrJobNotCancellable = errors.New("job cannot be cancelled")
	ErrInvalidJobType    = errors.New("invalid job type")
	ErrInvalidJobParams  = errors.New("invalid job parameters")
)

// JobService defines the job operations service interface
type JobService interface {
	// Create creates a new background job
	Create(ctx context.Context, params model.JobParams) (*model.Job, error)
	// Get returns a job by ID
	Get(ctx context.Context, jobID string) (*model.Job, error)
	// List returns all jobs
	List(ctx context.Context) ([]*model.Job, error)
	// Cancel cancels a running job
	Cancel(ctx context.Context, jobID string) error
	// Start starts the job executor
	Start(ctx context.Context)
	// Stop stops the job executor
	Stop()
}

// runningJob tracks a job that is currently executing
type runningJob struct {
	cancel context.CancelFunc
}

// jobService implements JobService
type jobService struct {
	fs          filesystem.FS
	hub         *websocket.Hub
	jobs        sync.Map // map[string]*runningJob
	allJobs     sync.Map // map[string]*model.Job - stores all jobs including completed
	jobMu       sync.RWMutex
	jobEventMu  sync.Mutex
	workQueue   chan *model.Job
	workers     int
	wg          sync.WaitGroup
	stopCh      chan struct{}
	mountPoints []model.MountPoint
	walker      Walker
}

// JobServiceConfig holds configuration for the job service
type JobServiceConfig struct {
	Workers     int
	MountPoints []model.MountPoint
}

// NewJobService creates a new job service
func NewJobService(fsys filesystem.FS, hub *websocket.Hub, cfg JobServiceConfig) JobService {
	workers := cfg.Workers
	if workers <= 0 {
		workers = config.DefaultJobWorkers
	}

	return &jobService{
		fs:          fsys,
		hub:         hub,
		workQueue:   make(chan *model.Job, config.JobQueueSize),
		workers:     workers,
		stopCh:      make(chan struct{}),
		mountPoints: cfg.MountPoints,
		walker:      NewWalker(fsys),
	}
}

// Start starts the job executor workers
func (s *jobService) Start(ctx context.Context) {
	for i := 0; i < s.workers; i++ {
		s.wg.Add(1)
		go s.worker(ctx)
	}

	// Start cleanup goroutine
	s.wg.Add(1)
	go s.cleanupLoop(ctx)
}

// Stop stops the job executor
func (s *jobService) Stop() {
	close(s.stopCh)
	s.wg.Wait()
}

// worker processes jobs from the work queue
func (s *jobService) worker(ctx context.Context) {
	defer s.wg.Done()

	for {
		select {
		case <-ctx.Done():
			return
		case <-s.stopCh:
			return
		case job := <-s.workQueue:
			if s.jobState(job) == model.JobStateCancelled {
				continue
			}
			s.execute(ctx, job)
		}
	}
}

// cleanupLoop periodically creates cleanup jobs
func (s *jobService) cleanupLoop(ctx context.Context) {
	defer s.wg.Done()
	ticker := time.NewTicker(config.JobCleanupInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-s.stopCh:
			return
		case <-ticker.C:
			s.cleanupHistory()
		}
	}
}

// cleanupHistory removes old completed jobs
func (s *jobService) cleanupHistory() {
	cutoff := time.Now().Add(-config.JobRetentionPeriod)
	s.allJobs.Range(func(key, value interface{}) bool {
		job := value.(*model.Job)
		snapshot := s.snapshotJob(job)
		if snapshot.State.IsTerminal() && snapshot.CompletedAt.Before(cutoff) {
			s.allJobs.Delete(key)
		}
		return true
	})
}

// Create creates a new background job
func (s *jobService) Create(ctx context.Context, params model.JobParams) (*model.Job, error) {
	// Validate job type
	if !params.Type.IsValid() {
		return nil, ErrInvalidJobType
	}

	// Validate parameters
	if params.SourcePath == "" {
		return nil, ErrInvalidJobParams
	}

	// Copy and move require destination path
	if (params.Type == model.JobTypeCopy || params.Type == model.JobTypeMove) && params.DestPath == "" {
		return nil, ErrInvalidJobParams
	}

	sourceMount, sourceFsPath, err := s.resolveJobPath(params.SourcePath)
	if err != nil {
		return nil, err
	}

	destFsPath := ""
	if params.DestPath != "" {
		destMount, resolvedDestPath, err := s.resolveJobPath(params.DestPath)
		if err != nil {
			return nil, err
		}
		if destMount.ReadOnly {
			return nil, ErrPermissionDenied
		}
		destFsPath = resolvedDestPath
	}

	if params.Type == model.JobTypeMove || params.Type == model.JobTypeDelete {
		if sourceMount.ReadOnly {
			return nil, ErrPermissionDenied
		}
	}

	// Create job
	job := &model.Job{
		ID:                 uuid.New().String(),
		Type:               params.Type,
		State:              model.JobStatePending,
		Progress:           0,
		SourcePath:         params.SourcePath,
		DestPath:           params.DestPath,
		ResolvedSourcePath: sourceFsPath,
		ResolvedDestPath:   destFsPath,
		CreatedAt:          time.Now(),
	}

	// Store job
	s.allJobs.Store(job.ID, job)

	// Queue job for execution
	select {
	case s.workQueue <- job:
		// Job queued successfully
	default:
		// Queue is full, mark as failed
		snapshot := s.updateAndBroadcast(job, func(job *model.Job) bool {
			job.State = model.JobStateFailed
			job.Error = "job queue is full"
			job.CompletedAt = time.Now()
			return true
		})
		return snapshot, nil
	}

	return s.snapshotJob(job), nil
}

func (s *jobService) resolveJobPath(path string) (*model.MountPoint, string, error) {
	mount, fsPath, err := validator.ValidatePathAgainstMounts(path, s.mountPoints)
	if err != nil {
		if errors.Is(err, validator.ErrOutsideMountPoint) || errors.Is(err, validator.ErrMountPointNotFound) {
			return nil, "", ErrMountPointNotFound
		}
		return nil, "", ErrInvalidJobParams
	}

	return mount, fsPath, nil
}

// Get returns a job by ID
func (s *jobService) Get(ctx context.Context, jobID string) (*model.Job, error) {
	if value, ok := s.allJobs.Load(jobID); ok {
		return s.snapshotJob(value.(*model.Job)), nil
	}
	return nil, ErrJobNotFound
}

// List returns all jobs
func (s *jobService) List(ctx context.Context) ([]*model.Job, error) {
	var jobs []*model.Job
	s.allJobs.Range(func(key, value interface{}) bool {
		jobs = append(jobs, s.snapshotJob(value.(*model.Job)))
		return true
	})
	return jobs, nil
}

// Cancel cancels a running job
func (s *jobService) Cancel(ctx context.Context, jobID string) error {
	// Check if job exists
	jobValue, ok := s.allJobs.Load(jobID)
	if !ok {
		return ErrJobNotFound
	}

	job := jobValue.(*model.Job)
	cancelled := s.updateAndBroadcast(job, func(job *model.Job) bool {
		if job.State != model.JobStatePending && job.State != model.JobStateRunning {
			return false
		}
		job.State = model.JobStateCancelled
		job.CompletedAt = time.Now()
		return true
	})
	if cancelled == nil {
		return ErrJobNotCancellable
	}

	// If job is running, cancel it
	if rj, ok := s.jobs.Load(jobID); ok {
		runningJob := rj.(*runningJob)
		runningJob.cancel()
	}

	return nil
}

// execute runs a job
func (s *jobService) execute(ctx context.Context, job *model.Job) {
	if s.jobState(job) == model.JobStateCancelled {
		return
	}

	// Create cancellable context for this job
	jobCtx, cancel := context.WithCancel(ctx)
	rj := &runningJob{cancel: cancel}
	s.jobs.Store(job.ID, rj)
	defer func() {
		s.jobs.Delete(job.ID)
		cancel()
	}()

	started := s.updateAndBroadcast(job, func(job *model.Job) bool {
		if job.State == model.JobStateCancelled {
			return false
		}
		job.State = model.JobStateRunning
		job.StartedAt = time.Now()
		return true
	})
	if started == nil {
		return
	}
	var err error
	switch started.Type {
	case model.JobTypeCopy:
		err = s.executeCopy(jobCtx, job)
	case model.JobTypeMove:
		err = s.executeMove(jobCtx, job)
	case model.JobTypeDelete:
		err = s.executeDelete(jobCtx, job)
	default:
		err = ErrInvalidJobType
	}

	// Check if cancelled
	if jobCtx.Err() == context.Canceled {
		// Job was cancelled, cleanup is handled by cancel
		return
	}

	s.updateAndBroadcast(job, func(job *model.Job) bool {
		if job.State == model.JobStateCancelled {
			return false
		}
		if err != nil {
			job.State = model.JobStateFailed
			job.Error = err.Error()
		} else {
			job.State = model.JobStateCompleted
			job.Progress = 100
		}
		job.CompletedAt = time.Now()
		return true
	})
}

// executeCopy copies a file or directory
func (s *jobService) executeCopy(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)
	srcInfo, err := s.fs.Stat(sourcePath)
	if err != nil {
		return err
	}

	if srcInfo.IsDir() {
		return s.copyDir(ctx, job)
	}
	return s.copyFile(ctx, job, srcInfo.Size())
}

// copyFile copies a single file with progress tracking
func (s *jobService) copyFile(ctx context.Context, job *model.Job, totalSize int64) error {
	sourcePath := jobSourcePath(job)
	destPath := jobDestPath(job)

	src, err := s.fs.Open(sourcePath)
	if err != nil {
		return err
	}
	defer src.Close()

	// Ensure destination directory exists
	destDir := filepath.Dir(destPath)
	if err := s.fs.MkdirAll(destDir, 0755); err != nil {
		return err
	}

	dst, err := s.fs.Create(destPath)
	if err != nil {
		return err
	}
	defer dst.Close()

	buf := make([]byte, 1024*1024) // 1MB buffer
	var copied int64

	for {
		select {
		case <-ctx.Done():
			// Cleanup partial file on cancellation
			dst.Close()
			s.fs.Remove(destPath)
			return ctx.Err()
		default:
		}

		n, readErr := src.Read(buf)
		if n > 0 {
			_, writeErr := dst.Write(buf[:n])
			if writeErr != nil {
				return writeErr
			}
			copied += int64(n)

			// Update progress
			if totalSize > 0 {
				progress := int(float64(copied) / float64(totalSize) * 100)
				s.updateProgress(job, progress)
			}
		}
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			return readErr
		}
	}

	return nil
}

// copyDir builds one traversal manifest, then copies without walking the
// directory tree a second time merely to calculate progress.
func (s *jobService) copyDir(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)
	destPath := jobDestPath(job)
	manifest, totalFiles, err := s.collectManifest(ctx, sourcePath)
	if err != nil {
		return err
	}
	if err := s.fs.MkdirAll(destPath, 0o755); err != nil {
		return err
	}

	copiedFiles := 0
	for _, entry := range manifest {
		if err := ctx.Err(); err != nil {
			return err
		}
		targetPath := filepath.Join(destPath, entry.RelativePath)
		if entry.Metadata.IsDir() {
			if err := s.fs.MkdirAll(targetPath, entry.Metadata.Mode().Perm()); err != nil {
				return err
			}
			continue
		}

		tempJob := &model.Job{
			SourcePath:         entry.Path,
			DestPath:           targetPath,
			ResolvedSourcePath: entry.Path,
			ResolvedDestPath:   targetPath,
		}
		if err := s.copyFile(ctx, tempJob, 0); err != nil {
			return err
		}
		copiedFiles++
		if totalFiles > 0 {
			progress := int(float64(copiedFiles) / float64(totalFiles) * 100)
			s.updateProgress(job, progress)
		}
	}
	return nil
}

// executeMove moves a file or directory
func (s *jobService) executeMove(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)
	destPath := jobDestPath(job)

	// Try simple rename first (works if on same filesystem)
	err := s.fs.Rename(sourcePath, destPath)
	if err == nil {
		s.updateProgress(job, 100)
		return nil
	}

	// If rename fails, fall back to copy + delete
	srcInfo, err := s.fs.Stat(sourcePath)
	if err != nil {
		return err
	}

	// Copy first
	if srcInfo.IsDir() {
		if err := s.copyDir(ctx, job); err != nil {
			return err
		}
	} else {
		if err := s.copyFile(ctx, job, srcInfo.Size()); err != nil {
			return err
		}
	}

	// Check for cancellation before deleting source
	select {
	case <-ctx.Done():
		// Cleanup destination on cancellation
		s.fs.RemoveAll(destPath)
		return ctx.Err()
	default:
	}

	// Delete source
	return s.fs.RemoveAll(sourcePath)
}

// executeDelete deletes a file or directory
func (s *jobService) executeDelete(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)
	info, err := s.fs.Stat(sourcePath)
	if err != nil {
		return err
	}

	if info.IsDir() {
		return s.deleteDir(ctx, job)
	}

	// Simple file delete
	if err := s.fs.Remove(sourcePath); err != nil {
		return err
	}
	s.updateProgress(job, 100)
	return nil
}

// deleteDir uses the same traversal manifest as copy and removes entries in
// reverse order so child paths are deleted before their parents.
func (s *jobService) deleteDir(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)
	manifest, totalFiles, err := s.collectManifest(ctx, sourcePath)
	if err != nil {
		return err
	}
	deletedFiles := 0
	for index := len(manifest) - 1; index >= 0; index-- {
		if err := ctx.Err(); err != nil {
			return err
		}
		entry := manifest[index]
		if err := s.fs.Remove(entry.Path); err != nil {
			return err
		}
		if !entry.Metadata.IsDir() {
			deletedFiles++
			if totalFiles > 0 {
				progress := int(float64(deletedFiles) / float64(totalFiles) * 100)
				s.updateProgress(job, progress)
			}
		}
	}
	return s.fs.Remove(sourcePath)
}

func (s *jobService) collectManifest(ctx context.Context, root string) ([]WalkEntry, int, error) {
	manifest := make([]WalkEntry, 0, 128)
	fileCount := 0
	err := s.walker.Walk(ctx, root, WalkOptions{IncludeHidden: true, LoadMetadata: true}, func(entry WalkEntry) error {
		manifest = append(manifest, entry)
		if !entry.Metadata.IsDir() {
			fileCount++
		}
		return nil
	})
	if err != nil {
		return nil, 0, err
	}
	return manifest, fileCount, nil
}

func jobSourcePath(job *model.Job) string {
	if job.ResolvedSourcePath != "" {
		return job.ResolvedSourcePath
	}
	return job.SourcePath
}

func jobDestPath(job *model.Job) string {
	if job.ResolvedDestPath != "" {
		return job.ResolvedDestPath
	}
	return job.DestPath
}

func (s *jobService) snapshotJob(job *model.Job) *model.Job {
	s.jobMu.RLock()
	defer s.jobMu.RUnlock()
	snapshot := *job
	return &snapshot
}

func (s *jobService) jobState(job *model.Job) model.JobState {
	s.jobMu.RLock()
	defer s.jobMu.RUnlock()
	return job.State
}

func (s *jobService) updateJobIf(job *model.Job, update func(*model.Job) bool) *model.Job {
	s.jobMu.Lock()
	defer s.jobMu.Unlock()
	if !update(job) {
		return nil
	}
	snapshot := *job
	return &snapshot
}

func (s *jobService) updateAndBroadcast(
	job *model.Job,
	update func(*model.Job) bool,
) *model.Job {
	s.jobEventMu.Lock()
	defer s.jobEventMu.Unlock()
	snapshot := s.updateJobIf(job, update)
	if snapshot != nil {
		s.broadcastUpdate(snapshot)
	}
	return snapshot
}

func (s *jobService) updateProgress(job *model.Job, progress int) {
	s.updateAndBroadcast(job, func(job *model.Job) bool {
		if job.State.IsTerminal() || job.Progress == progress {
			return false
		}
		job.Progress = progress
		return true
	})
}

// broadcastUpdate sends a job update via WebSocket
func (s *jobService) broadcastUpdate(job *model.Job) {
	if s.hub != nil {
		s.hub.BroadcastJobUpdate(job)
	}
}
