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
	"github.com/homelab/filemanager/internal/config"
	"github.com/homelab/filemanager/internal/model"
	"github.com/homelab/filemanager/internal/pkg/filesystem"
	"github.com/homelab/filemanager/internal/pkg/validator"
	"github.com/homelab/filemanager/internal/websocket"
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
	workQueue   chan *model.Job
	workers     int
	wg          sync.WaitGroup
	stopCh      chan struct{}
	mountPoints []model.MountPoint
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
			if job.State == model.JobStateCancelled {
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
		if (job.State == model.JobStateCompleted || job.State == model.JobStateFailed || job.State == model.JobStateCancelled) && job.CompletedAt.Before(cutoff) {
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
		job.State = model.JobStateFailed
		job.Error = "job queue is full"
		job.CompletedAt = time.Now()
		s.broadcastUpdate(job)
		return job, nil
	}

	return job, nil
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
		return value.(*model.Job), nil
	}
	return nil, ErrJobNotFound
}

// List returns all jobs
func (s *jobService) List(ctx context.Context) ([]*model.Job, error) {
	var jobs []*model.Job
	s.allJobs.Range(func(key, value interface{}) bool {
		jobs = append(jobs, value.(*model.Job))
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

	// Check if job is in a cancellable state
	if job.State != model.JobStatePending && job.State != model.JobStateRunning {
		return ErrJobNotCancellable
	}

	// If job is running, cancel it
	if rj, ok := s.jobs.Load(jobID); ok {
		runningJob := rj.(*runningJob)
		runningJob.cancel()
	}

	// Update job state
	job.State = model.JobStateCancelled
	job.CompletedAt = time.Now()
	s.broadcastUpdate(job)

	return nil
}

// execute runs a job
func (s *jobService) execute(ctx context.Context, job *model.Job) {
	if job.State == model.JobStateCancelled {
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

	if job.State == model.JobStateCancelled {
		return
	}

	// Update job state to running
	job.State = model.JobStateRunning
	job.StartedAt = time.Now()
	s.broadcastUpdate(job)

	var err error
	switch job.Type {
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

	// Update final state
	if err != nil {
		job.State = model.JobStateFailed
		job.Error = err.Error()
	} else {
		job.State = model.JobStateCompleted
		job.Progress = 100
	}
	job.CompletedAt = time.Now()
	s.broadcastUpdate(job)
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
				if progress != job.Progress {
					job.Progress = progress
					s.broadcastUpdate(job)
				}
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

// copyDir copies a directory recursively with progress tracking
func (s *jobService) copyDir(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)
	destPath := jobDestPath(job)

	// First, count total files for progress tracking
	totalFiles, err := s.countFiles(sourcePath)
	if err != nil {
		return err
	}

	var copiedFiles int
	return s.copyDirRecursive(ctx, job, sourcePath, destPath, totalFiles, &copiedFiles)
}

// copyDirRecursive recursively copies a directory
func (s *jobService) copyDirRecursive(ctx context.Context, job *model.Job, srcDir, dstDir string, totalFiles int, copiedFiles *int) error {
	select {
	case <-ctx.Done():
		return ctx.Err()
	default:
	}

	// Create destination directory
	if err := s.fs.MkdirAll(dstDir, 0755); err != nil {
		return err
	}

	entries, err := s.fs.ReadDir(srcDir)
	if err != nil {
		return err
	}

	for _, entry := range entries {
		srcPath := filepath.Join(srcDir, entry.Name())
		dstPath := filepath.Join(dstDir, entry.Name())

		if entry.IsDir() {
			if err := s.copyDirRecursive(ctx, job, srcPath, dstPath, totalFiles, copiedFiles); err != nil {
				return err
			}
		} else {
			info, err := entry.Info()
			if err != nil {
				return err
			}

			// Create a temporary job for file copy (to track individual file progress)
			tempJob := &model.Job{
				SourcePath:         srcPath,
				DestPath:           dstPath,
				ResolvedSourcePath: srcPath,
				ResolvedDestPath:   dstPath,
			}
			if err := s.copyFile(ctx, tempJob, info.Size()); err != nil {
				return err
			}

			*copiedFiles++
			if totalFiles > 0 {
				progress := int(float64(*copiedFiles) / float64(totalFiles) * 100)
				if progress != job.Progress {
					job.Progress = progress
					s.broadcastUpdate(job)
				}
			}
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
		job.Progress = 100
		s.broadcastUpdate(job)
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
	job.Progress = 100
	s.broadcastUpdate(job)
	return nil
}

// deleteDir deletes a directory recursively with progress tracking
func (s *jobService) deleteDir(ctx context.Context, job *model.Job) error {
	sourcePath := jobSourcePath(job)

	// Count total files for progress tracking
	totalFiles, err := s.countFiles(sourcePath)
	if err != nil {
		return err
	}

	var deletedFiles int
	return s.deleteDirRecursive(ctx, job, sourcePath, totalFiles, &deletedFiles)
}

// deleteDirRecursive recursively deletes a directory
func (s *jobService) deleteDirRecursive(ctx context.Context, job *model.Job, dirPath string, totalFiles int, deletedFiles *int) error {
	select {
	case <-ctx.Done():
		return ctx.Err()
	default:
	}

	entries, err := s.fs.ReadDir(dirPath)
	if err != nil {
		return err
	}

	for _, entry := range entries {
		entryPath := filepath.Join(dirPath, entry.Name())

		if entry.IsDir() {
			if err := s.deleteDirRecursive(ctx, job, entryPath, totalFiles, deletedFiles); err != nil {
				return err
			}
		} else {
			if err := s.fs.Remove(entryPath); err != nil {
				return err
			}

			*deletedFiles++
			if totalFiles > 0 {
				progress := int(float64(*deletedFiles) / float64(totalFiles) * 100)
				if progress != job.Progress {
					job.Progress = progress
					s.broadcastUpdate(job)
				}
			}
		}
	}

	// Remove the directory itself
	return s.fs.Remove(dirPath)
}

// countFiles counts the total number of files in a directory recursively
func (s *jobService) countFiles(path string) (int, error) {
	info, err := s.fs.Stat(path)
	if err != nil {
		return 0, err
	}

	if !info.IsDir() {
		return 1, nil
	}

	var count int
	entries, err := s.fs.ReadDir(path)
	if err != nil {
		return 0, err
	}

	for _, entry := range entries {
		if entry.IsDir() {
			subCount, err := s.countFiles(filepath.Join(path, entry.Name()))
			if err != nil {
				return 0, err
			}
			count += subCount
		} else {
			count++
		}
	}

	return count, nil
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

// broadcastUpdate sends a job update via WebSocket
func (s *jobService) broadcastUpdate(job *model.Job) {
	if s.hub != nil {
		s.hub.BroadcastJobUpdate(job)
	}
}
