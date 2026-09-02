package service

import (
	"bytes"
	"context"
	"errors"
	"os"
	"sync"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/spf13/afero"
)

func TestCancelledPendingJobDoesNotExecute(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	if err := fs.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.MkdirAll("/data/backup", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/source.txt", []byte("content"), 0o644); err != nil {
		t.Fatal(err)
	}

	svc := NewJobService(fs, nil, JobServiceConfig{
		Workers: 1,
		MountPoints: []model.MountPoint{
			{Name: "media", Path: "/data/media"},
			{Name: "backup", Path: "/data/backup"},
		},
	})

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	job, err := svc.Create(ctx, model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "media/source.txt",
		DestPath:   "backup/copied.txt",
	})
	if err != nil {
		t.Fatalf("create job: %v", err)
	}
	if err := svc.Cancel(ctx, job.ID); err != nil {
		t.Fatalf("cancel job: %v", err)
	}

	svc.Start(ctx)
	defer svc.Stop()
	time.Sleep(50 * time.Millisecond)

	current, err := svc.Get(ctx, job.ID)
	if err != nil {
		t.Fatalf("get job: %v", err)
	}
	if current.State != model.JobStateCancelled {
		t.Fatalf("job state = %s, want %s", current.State, model.JobStateCancelled)
	}
	if exists, _ := fs.Exists("/data/backup/copied.txt"); exists {
		t.Fatal("cancelled pending job created destination")
	}
}

func TestJobsAreScopedToAuthenticatedOwner(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	_ = fsys.MkdirAll("/data/media", 0o755)
	_ = fsys.MkdirAll("/data/backup", 0o755)
	_ = fsys.WriteFile("/data/media/source.txt", []byte("content"), 0o644)
	svc := NewJobService(fsys, nil, JobServiceConfig{MountPoints: []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "backup", Path: "/data/backup"},
	}})

	alice := authcontext.WithUsername(context.Background(), "alice")
	bob := authcontext.WithUsername(context.Background(), "bob")
	job, err := svc.Create(alice, model.JobParams{
		Type: model.JobTypeCopy, SourcePath: "media/source.txt", DestPath: "backup/copied.txt",
	})
	if err != nil {
		t.Fatal(err)
	}
	if jobs, err := svc.List(bob); err != nil || len(jobs) != 0 {
		t.Fatalf("bob saw alice's jobs: count=%d err=%v", len(jobs), err)
	}
	if _, err := svc.Get(bob, job.ID); !errors.Is(err, ErrJobNotFound) {
		t.Fatalf("bob read alice's job: %v", err)
	}
	if err := svc.Cancel(bob, job.ID); !errors.Is(err, ErrJobNotFound) {
		t.Fatalf("bob cancelled alice's job: %v", err)
	}
}

func TestJobServiceReturnsSnapshots(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	if err := fs.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.MkdirAll("/data/backup", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/source.txt", []byte("content"), 0o644); err != nil {
		t.Fatal(err)
	}

	svc := NewJobService(fs, nil, JobServiceConfig{
		Workers: 1,
		MountPoints: []model.MountPoint{
			{Name: "media", Path: "/data/media"},
			{Name: "backup", Path: "/data/backup"},
		},
	})
	job, err := svc.Create(context.Background(), model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "media/source.txt",
		DestPath:   "backup/copied.txt",
	})
	if err != nil {
		t.Fatal(err)
	}

	job.State = model.JobStateCompleted
	job.Progress = 100
	stored, err := svc.Get(context.Background(), job.ID)
	if err != nil {
		t.Fatal(err)
	}
	if stored.State != model.JobStatePending || stored.Progress != 0 {
		t.Fatalf("external mutation changed stored job: state=%s progress=%d", stored.State, stored.Progress)
	}

	listed, err := svc.List(context.Background())
	if err != nil || len(listed) != 1 {
		t.Fatalf("list jobs: count=%d err=%v", len(listed), err)
	}
	listed[0].State = model.JobStateFailed
	stored, err = svc.Get(context.Background(), job.ID)
	if err != nil {
		t.Fatal(err)
	}
	if stored.State != model.JobStatePending {
		t.Fatalf("list result mutation changed stored job: %s", stored.State)
	}
}

type synchronizedFS struct {
	filesystem.FS
	onWriteFile func(name string)
	failRename  bool
}

func (s *synchronizedFS) OpenFile(name string, flag int, perm os.FileMode) (afero.File, error) {
	file, err := s.FS.OpenFile(name, flag, perm)
	if err != nil {
		return nil, err
	}
	if s.onWriteFile != nil {
		return &hookedFile{File: file, onWrite: func() { s.onWriteFile(name) }}, nil
	}
	return file, nil
}

func (s *synchronizedFS) Rename(oldpath, newpath string) error {
	if s.failRename {
		return errors.New("cross-device link")
	}
	return s.FS.Rename(oldpath, newpath)
}

type hookedFile struct {
	afero.File
	onWrite func()
	once    sync.Once
}

func (h *hookedFile) Write(p []byte) (int, error) {
	n, err := h.File.Write(p)
	if n > 0 && h.onWrite != nil {
		h.once.Do(h.onWrite)
	}
	return n, err
}

func TestJobMidFlightCancellationCleansUpPartialDestination(t *testing.T) {
	tests := []struct {
		name       string
		jobType    model.JobType
		failRename bool
	}{
		{
			name:       "copy job cleans up destination and preserves source",
			jobType:    model.JobTypeCopy,
			failRename: false,
		},
		{
			name:       "move job cleans up destination and preserves source",
			jobType:    model.JobTypeMove,
			failRename: true, // forces fallback from rename to copy-then-delete
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			memFS := filesystem.NewMemMapFS()
			if err := memFS.MkdirAll("/data/media", 0o755); err != nil {
				t.Fatal(err)
			}
			if err := memFS.MkdirAll("/data/backup", 0o755); err != nil {
				t.Fatal(err)
			}

			// 2MB payload ensures multiple 1MB chunks in copyFile
			sourcePayload := bytes.Repeat([]byte("X"), 2*1024*1024)
			sourcePath := "/data/media/source.bin"
			destPath := "/data/backup/dest.bin"

			if err := memFS.WriteFile(sourcePath, sourcePayload, 0o644); err != nil {
				t.Fatal(err)
			}

			var (
				svc        JobService
				jobID      string
				cancelOnce sync.Once
				cancelled  = make(chan struct{})
			)

			syncFS := &synchronizedFS{
				FS:         memFS,
				failRename: tt.failRename,
				onWriteFile: func(name string) {
					if name == destPath && jobID != "" {
						cancelOnce.Do(func() {
							// Destination file must exist mid-transfer
							exists, err := memFS.Exists(destPath)
							if err != nil || !exists {
								t.Errorf("destination file should exist during copy: exists=%v, err=%v", exists, err)
							}

							// Cancel while transfer is in flight
							if err := svc.Cancel(context.Background(), jobID); err != nil {
								t.Errorf("cancel job failed: %v", err)
							}
							close(cancelled)
						})
					}
				},
			}

			svc = NewJobService(syncFS, nil, JobServiceConfig{
				Workers: 1,
				MountPoints: []model.MountPoint{
					{Name: "media", Path: "/data/media"},
					{Name: "backup", Path: "/data/backup"},
				},
			})

			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()

			job, err := svc.Create(ctx, model.JobParams{
				Type:       tt.jobType,
				SourcePath: "media/source.bin",
				DestPath:   "backup/dest.bin",
			})
			if err != nil {
				t.Fatalf("create job: %v", err)
			}
			jobID = job.ID

			svc.Start(ctx)

			select {
			case <-cancelled:
			case <-time.After(5 * time.Second):
				t.Fatal("timed out waiting for mid-flight cancellation trigger")
			}

			// Stop the service to wait for the worker to exit copyFile and finish cleanup
			svc.Stop()

			terminalJob, err := svc.Get(ctx, job.ID)
			if err != nil || terminalJob == nil {
				t.Fatalf("get job failed: %v", err)
			}
			if terminalJob.State != model.JobStateCancelled {
				t.Fatalf("job state = %s, want %s", terminalJob.State, model.JobStateCancelled)
			}

			// Destination must be cleaned up
			destExists, err := memFS.Exists(destPath)
			if err != nil {
				t.Fatalf("check dest exists: %v", err)
			}
			if destExists {
				t.Fatal("cancelled job left partial destination file")
			}

			// Source must remain unchanged
			sourceExists, err := memFS.Exists(sourcePath)
			if err != nil {
				t.Fatalf("check source exists: %v", err)
			}
			if !sourceExists {
				t.Fatal("source file was unexpectedly removed")
			}
			sourceContent, err := memFS.ReadFile(sourcePath)
			if err != nil {
				t.Fatalf("read source file: %v", err)
			}
			if !bytes.Equal(sourceContent, sourcePayload) {
				t.Fatal("source file content was modified")
			}
		})
	}
}
