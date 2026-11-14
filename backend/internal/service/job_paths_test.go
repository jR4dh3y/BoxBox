package service

import (
	"context"
	"sync"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func setupTestJobService() (JobService, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()
	if err := fs.MkdirAll("/data/media", 0755); err != nil {
		panic(err)
	}
	if err := fs.MkdirAll("/data/backup", 0755); err != nil {
		panic(err)
	}
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "backup", Path: "/data/backup"},
	}
	return NewJobService(fs, nil, JobServiceConfig{Workers: 2, MountPoints: mounts}), fs
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

func TestRevalidateJobPathsSynchronizesSnapshots(t *testing.T) {
	svc, fs := setupTestJobService()
	if err := fs.WriteFile("/data/media/source.bin", []byte("source"), 0644); err != nil {
		t.Fatalf("write source: %v", err)
	}

	job, err := svc.Create(context.Background(), model.JobParams{
		Type:       model.JobTypeCopy,
		SourcePath: "media/source.bin",
		DestPath:   "backup/destination.bin",
	})
	if err != nil {
		t.Fatalf("create job: %v", err)
	}

	internal := svc.(*jobService)
	start := make(chan struct{})
	var workers sync.WaitGroup
	workers.Add(2)
	go func() {
		defer workers.Done()
		<-start
		for range 1000 {
			if err := internal.revalidateJobPaths(job); err != nil {
				t.Errorf("revalidate job paths: %v", err)
				return
			}
		}
	}()
	go func() {
		defer workers.Done()
		<-start
		for range 1000 {
			internal.snapshotJob(job)
		}
	}()
	close(start)
	workers.Wait()
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
