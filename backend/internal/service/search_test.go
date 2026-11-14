package service

import (
	"context"
	"errors"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func setupSearchTestService() (SearchService, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()
	_ = fs.MkdirAll("/data/media", 0o755)
	return NewSearchService(fs, SearchServiceConfig{
		MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}},
	}), fs
}

func TestSearchFindsCaseInsensitiveRecursiveMatches(t *testing.T) {
	svc, fs := setupSearchTestService()
	if err := fs.MkdirAll("/data/media/library/sub", 0o755); err != nil {
		t.Fatal(err)
	}
	for path, content := range map[string]string{
		"/data/media/library/Annual-Report.pdf":    "report",
		"/data/media/library/sub/report-notes.txt": "notes",
		"/data/media/library/photo.jpg":            "photo",
	} {
		if err := fs.WriteFile(path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	results, err := svc.Search(context.Background(), "media/library", "REPORT")
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 2 {
		t.Fatalf("got %d search results, want 2: %+v", len(results), results)
	}
	for _, result := range results {
		if result.Path != "media/library/Annual-Report.pdf" && result.Path != "media/library/sub/report-notes.txt" {
			t.Fatalf("unexpected search result path %q", result.Path)
		}
		if result.Name == "" || result.ModTime.IsZero() || result.Size < 0 {
			t.Fatalf("incomplete search result metadata: %+v", result)
		}
	}
}

func TestSearchRejectsEmptyQueryAndReturnsNoMatches(t *testing.T) {
	svc, fs := setupSearchTestService()
	if err := fs.MkdirAll("/data/media/library", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/library/readme.txt", []byte("content"), 0o644); err != nil {
		t.Fatal(err)
	}

	if _, err := svc.Search(context.Background(), "media/library", "   "); !errors.Is(err, ErrEmptyQuery) {
		t.Fatalf("empty query error = %v, want ErrEmptyQuery", err)
	}
	results, err := svc.Search(context.Background(), "media/library", "missing")
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 0 {
		t.Fatalf("got unexpected search results: %+v", results)
	}
}
