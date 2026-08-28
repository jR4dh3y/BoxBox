package service

import (
	"context"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestWalkerWalkSeq(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/dir/sub", 0755); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/dir/file1.txt", []byte("hello"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/dir/sub/file2.txt", []byte("world"), 0644); err != nil {
		t.Fatal(err)
	}

	walker := NewWalker(fsys)
	ctx := context.Background()

	var visited []string
	for entry, err := range walker.WalkSeq(ctx, "/data/dir", WalkOptions{IncludeHidden: true}) {
		if err != nil {
			t.Fatalf("unexpected error during WalkSeq: %v", err)
		}
		visited = append(visited, entry.DirEntry.Name())
	}

	if len(visited) < 3 {
		t.Fatalf("expected at least 3 visited entries, got %d: %v", len(visited), visited)
	}
}
