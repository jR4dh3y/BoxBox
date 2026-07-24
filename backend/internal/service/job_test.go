package service

import (
	"context"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
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
