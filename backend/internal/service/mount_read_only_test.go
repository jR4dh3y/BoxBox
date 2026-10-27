package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestReadOnlyMountProtectsAncestorRenameAndJobs(t *testing.T) {
	home := t.TempDir()
	protected := filepath.Join(home, "parent", "backups")
	if err := os.MkdirAll(protected, 0755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(protected, "db.bak")
	if err := os.WriteFile(target, []byte("precious"), 0644); err != nil {
		t.Fatal(err)
	}
	list := []model.MountPoint{{Name: "home", Path: home}, {Name: "backups", Path: protected, ReadOnly: true}}
	fsys := filesystem.NewOsFS()
	files := NewFileService(fsys, FileServiceConfig{MountPoints: list})
	ctx := context.Background()
	for _, path := range []string{"home/parent/backups/db.bak", "home/parent/backups", "home/parent"} {
		if err := files.Rename(ctx, path, "home/renamed"); !errors.Is(err, ErrPermissionDenied) {
			t.Errorf("rename %s: %v", path, err)
		}
		if err := files.Delete(ctx, path); !errors.Is(err, ErrPermissionDenied) {
			t.Errorf("delete %s: %v", path, err)
		}
	}
	if err := os.WriteFile(filepath.Join(home, "source"), []byte("source"), 0644); err != nil {
		t.Fatal(err)
	}
	jobs := NewJobService(fsys, nil, JobServiceConfig{MountPoints: list})
	for _, params := range []model.JobParams{
		{Type: model.JobTypeMove, SourcePath: "home/source", DestPath: "home/parent/backups/moved"},
		{Type: model.JobTypeDelete, SourcePath: "home/parent"},
		{Type: model.JobTypeDelete, SourcePath: "home/parent/backups/db.bak"},
		{Type: model.JobTypeMove, SourcePath: "home/parent", DestPath: "home/moved"},
		{Type: model.JobTypeCopy, SourcePath: "home/parent/backups/db.bak", DestPath: "home/parent/backups/copy"},
		{Type: model.JobTypeMove, SourcePath: "home/parent/backups/db.bak", DestPath: "home/parent/backups/moved"},
	} {
		if _, err := jobs.Create(ctx, params); !errors.Is(err, ErrPermissionDenied) {
			t.Errorf("job %+v: %v", params, err)
		}
	}
	if content, err := os.ReadFile(target); err != nil || string(content) != "precious" {
		t.Fatalf("protected content: %q %v", content, err)
	}
}

func TestQueuedJobsRecheckOverlappingReadOnlyMounts(t *testing.T) {
	home := t.TempDir()
	protected := filepath.Join(home, "backups")
	if err := os.Mkdir(protected, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(protected, "db.bak"), []byte("precious"), 0644); err != nil {
		t.Fatal(err)
	}
	list := []model.MountPoint{{Name: "home", Path: home}, {Name: "backups", Path: protected}}
	jobs := NewJobService(filesystem.NewOsFS(), nil, JobServiceConfig{MountPoints: list})
	for _, test := range []struct {
		params model.JobParams
		want   error
	}{
		{model.JobParams{Type: model.JobTypeDelete, SourcePath: "home/backups/db.bak"}, ErrPermissionDenied},
		{model.JobParams{Type: model.JobTypeMove, SourcePath: "home/backups", DestPath: "home/moved"}, ErrPermissionDenied},
		{model.JobParams{Type: model.JobTypeCopy, SourcePath: "home/backups/db.bak", DestPath: "home/backups/new"}, ErrPermissionDenied},
		{model.JobParams{Type: model.JobTypeCopy, SourcePath: "home/backups/db.bak", DestPath: "home/copy"}, nil},
	} {
		list[1].ReadOnly = false
		job, err := jobs.Create(context.Background(), test.params)
		if err != nil {
			t.Fatal(err)
		}
		list[1].ReadOnly = true
		if err := jobs.(*jobService).revalidateJobPaths(job); !errors.Is(err, test.want) {
			t.Errorf("revalidate %+v: %v, want %v", test.params, err, test.want)
		}
	}
}
