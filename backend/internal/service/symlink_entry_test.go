package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// symlinkFixture builds home/photos/a.jpg and home/shortcut -> home/photos.
func symlinkFixture(t *testing.T) (home, photos string, mountList []model.MountPoint) {
	t.Helper()
	home = t.TempDir()
	photos = filepath.Join(home, "photos")
	if err := os.MkdirAll(photos, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(photos, "a.jpg"), []byte("jpeg"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(photos, filepath.Join(home, "shortcut")); err != nil {
		t.Fatal(err)
	}
	return home, photos, []model.MountPoint{{Name: "home", Path: home}}
}

func requireTargetIntact(t *testing.T, photos string) {
	t.Helper()
	if _, err := os.Stat(filepath.Join(photos, "a.jpg")); err != nil {
		t.Fatalf("symlink target was changed: %v", err)
	}
}

func requireMissing(t *testing.T, path string) {
	t.Helper()
	if _, err := os.Lstat(path); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("%s still exists: %v", path, err)
	}
}

func TestMountsResolveEntryKeepsFinalSymlink(t *testing.T) {
	home, _, mountList := symlinkFixture(t)
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(home, "escape")); err != nil {
		t.Fatal(err)
	}
	m := newMounts(filesystem.NewOsFS(), fixedMounts(mountList))
	root, err := filepath.EvalSymlinks(home)
	if err != nil {
		t.Fatal(err)
	}

	for _, access := range []pathAccess{readEntry, writeEntry} {
		_, resolved, err := m.resolve("home/shortcut", access)
		if err != nil || resolved != filepath.Join(root, "shortcut") {
			t.Fatalf("resolve(home/shortcut, %d) = %q, %v; want the link itself", access, resolved, err)
		}
		if _, _, err := m.resolve("home/escape", access); !errors.Is(err, ErrPermissionDenied) {
			t.Fatalf("link out of the mount: error = %v, want %v", err, ErrPermissionDenied)
		}
		if _, resolved, err := m.resolve("home", access); err != nil || resolved != root {
			t.Fatalf("mount root = %q, %v; want %q", resolved, err, root)
		}
	}

	readOnly := newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{{Name: "home", Path: home, ReadOnly: true}}))
	if _, _, err := readOnly.resolve("home/shortcut", readEntry); err != nil {
		t.Fatalf("read-only mount refused readEntry: %v", err)
	}
	if _, _, err := readOnly.resolve("home/shortcut", writeEntry); !errors.Is(err, ErrPermissionDenied) {
		t.Fatalf("read-only mount writeEntry: error = %v, want %v", err, ErrPermissionDenied)
	}
}

func TestFileDeleteRemovesSymlinkNotTarget(t *testing.T) {
	home, photos, mountList := symlinkFixture(t)
	if err := os.Symlink(filepath.Join(photos, "a.jpg"), filepath.Join(home, "a-shortcut.jpg")); err != nil {
		t.Fatal(err)
	}
	svc := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: mountList})

	for _, link := range []string{"shortcut", "a-shortcut.jpg"} {
		if err := svc.Delete(context.Background(), "home/"+link); err != nil {
			t.Fatalf("delete %s: %v", link, err)
		}
		requireMissing(t, filepath.Join(home, link))
		requireTargetIntact(t, photos)
	}
}

func TestFileRenameRenamesSymlinkNotTarget(t *testing.T) {
	home, photos, mountList := symlinkFixture(t)
	svc := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: mountList})

	if err := svc.Rename(context.Background(), "home/shortcut", "home/renamed"); err != nil {
		t.Fatal(err)
	}
	requireMissing(t, filepath.Join(home, "shortcut"))
	info, err := os.Lstat(filepath.Join(home, "renamed"))
	if err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("renamed entry is not the symlink: %v, %v", info, err)
	}
	requireTargetIntact(t, photos)
}

func TestFileEntryInfoDescribesSymlink(t *testing.T) {
	_, _, mountList := symlinkFixture(t)
	svc := NewFileService(filesystem.NewOsFS(), FileServiceConfig{MountPoints: mountList})

	entry, err := svc.EntryInfo(context.Background(), "home/shortcut")
	if err != nil || entry.IsDir {
		t.Fatalf("EntryInfo(home/shortcut) = %+v, %v; want a non-directory entry", entry, err)
	}
	followed, err := svc.GetInfo(context.Background(), "home/shortcut")
	if err != nil || !followed.IsDir {
		t.Fatalf("GetInfo(home/shortcut) = %+v, %v; want the target folder", followed, err)
	}
}

func runJob(t *testing.T, mountList []model.MountPoint, params model.JobParams) *model.Job {
	t.Helper()
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)
	svc := NewJobService(filesystem.NewOsFS(), nil, JobServiceConfig{MountPoints: mountList})
	svc.Start(ctx)
	job, err := svc.Create(ctx, params)
	if err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		current, err := svc.Get(ctx, job.ID)
		if err != nil {
			t.Fatal(err)
		}
		if current.State.IsTerminal() {
			return current
		}
		time.Sleep(5 * time.Millisecond)
	}
	t.Fatal("job did not finish")
	return nil
}

func TestDeleteJobRemovesSymlinkNotTarget(t *testing.T) {
	home, photos, mountList := symlinkFixture(t)

	job := runJob(t, mountList, model.JobParams{Type: model.JobTypeDelete, SourcePath: "home/shortcut"})
	if job.State != model.JobStateCompleted {
		t.Fatalf("job state = %s (%s), want completed", job.State, job.Error)
	}
	requireMissing(t, filepath.Join(home, "shortcut"))
	requireTargetIntact(t, photos)
}

func TestDeleteJobRemovesFolderContainingSymlinks(t *testing.T) {
	home, photos, mountList := symlinkFixture(t)
	trash := filepath.Join(home, "trash")
	if err := os.MkdirAll(trash, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(photos, filepath.Join(trash, "shortcut")); err != nil {
		t.Fatal(err)
	}

	job := runJob(t, mountList, model.JobParams{Type: model.JobTypeDelete, SourcePath: "home/trash"})
	if job.State != model.JobStateCompleted {
		t.Fatalf("job state = %s (%s), want completed", job.State, job.Error)
	}
	requireMissing(t, trash)
	requireTargetIntact(t, photos)
}

func TestMoveJobMovesSymlinkNotTarget(t *testing.T) {
	home, photos, mountList := symlinkFixture(t)
	if err := os.MkdirAll(filepath.Join(home, "elsewhere"), 0o755); err != nil {
		t.Fatal(err)
	}

	job := runJob(t, mountList, model.JobParams{
		Type: model.JobTypeMove, SourcePath: "home/shortcut", DestPath: "home/elsewhere/shortcut",
	})
	if job.State != model.JobStateCompleted {
		t.Fatalf("job state = %s (%s), want completed", job.State, job.Error)
	}
	requireMissing(t, filepath.Join(home, "shortcut"))
	info, err := os.Lstat(filepath.Join(home, "elsewhere", "shortcut"))
	if err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("moved entry is not the symlink: %v, %v", info, err)
	}
	requireTargetIntact(t, photos)
}

func TestCopyJobStillCopiesSymlinkTarget(t *testing.T) {
	home, photos, mountList := symlinkFixture(t)

	job := runJob(t, mountList, model.JobParams{
		Type: model.JobTypeCopy, SourcePath: "home/shortcut", DestPath: "home/copy",
	})
	if job.State != model.JobStateCompleted {
		t.Fatalf("job state = %s (%s), want completed", job.State, job.Error)
	}
	if _, err := os.Stat(filepath.Join(home, "copy", "a.jpg")); err != nil {
		t.Fatalf("copy did not copy the target's contents: %v", err)
	}
	requireTargetIntact(t, photos)
}

func TestJobsOnMountRootBehindSymlink(t *testing.T) {
	for _, jobType := range []model.JobType{model.JobTypeDelete, model.JobTypeMove} {
		base := t.TempDir()
		real := filepath.Join(base, "real")
		if err := os.MkdirAll(real, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(real, "a.jpg"), []byte("jpeg"), 0o644); err != nil {
			t.Fatal(err)
		}
		link := filepath.Join(base, "home")
		if err := os.Symlink(real, link); err != nil {
			t.Fatal(err)
		}
		dest := t.TempDir()
		mountList := []model.MountPoint{{Name: "home", Path: link}, {Name: "dest", Path: dest}}

		params := model.JobParams{Type: jobType, SourcePath: "home"}
		if jobType == model.JobTypeMove {
			params.DestPath = "dest/moved"
		}
		job := runJob(t, mountList, params)
		if job.State != model.JobStateCompleted {
			t.Fatalf("%s job state = %s (%s), want completed", jobType, job.State, job.Error)
		}
	}
}
