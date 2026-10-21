package service

import (
	"context"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/websocket"
)

type destinationRaceFS struct{ filesystem.FS }

func (f destinationRaceFS) RenameNoReplace(source, destination string) error {
	if err := f.FS.WriteFile(destination, []byte("other application's file"), 0600); err != nil {
		return err
	}
	return f.FS.RenameNoReplace(source, destination)
}

func TestMoveDoesNotReplaceDestinationCreatedAfterPreflight(t *testing.T) {
	dir := t.TempDir()
	source, destination := filepath.Join(dir, "source"), filepath.Join(dir, "destination")
	if err := os.WriteFile(source, []byte("original source"), 0600); err != nil {
		t.Fatal(err)
	}
	f := destinationRaceFS{FS: filesystem.NewOsFS()}
	s := &jobService{fs: f, hub: websocket.NewHub()}
	err := s.executeMove(context.Background(), &model.Job{SourcePath: source, DestPath: destination})
	if !errors.Is(err, ErrPathExists) {
		t.Fatalf("move error = %v", err)
	}
	for path, want := range map[string]string{source: "original source", destination: "other application's file"} {
		data, err := os.ReadFile(path)
		if err != nil || string(data) != want {
			t.Fatalf("%s = %q (%v), want %q", path, data, err, want)
		}
	}
}

func TestDirectoryCopyDoesNotMergeIntoExistingDestination(t *testing.T) {
	dir := t.TempDir()
	source, destination := filepath.Join(dir, "source"), filepath.Join(dir, "destination")
	for _, path := range []string{source, destination} {
		if err := os.Mkdir(path, 0700); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(source, "new-file"), []byte("source"), 0600); err != nil {
		t.Fatal(err)
	}
	f := filesystem.NewOsFS()
	s := &jobService{fs: f, hub: websocket.NewHub(), walker: NewWalker(f)}
	err := s.copyDir(context.Background(), &model.Job{SourcePath: source, DestPath: destination})
	if !errors.Is(err, fs.ErrExist) {
		t.Fatalf("copy error = %v", err)
	}
	if _, err := os.Stat(filepath.Join(destination, "new-file")); !errors.Is(err, fs.ErrNotExist) {
		t.Fatal("copy merged into an existing destination")
	}
}
