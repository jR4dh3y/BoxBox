package service

import (
	"context"
	"fmt"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func setupBenchTree(tb testing.TB, folders, filesPerFolder int) SearchService {
	tb.Helper()
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0755); err != nil {
		tb.Fatal(err)
	}
	for i := 0; i < folders; i++ {
		dir := fmt.Sprintf("/data/media/Folder-%03d", i)
		if err := fsys.MkdirAll(dir, 0755); err != nil {
			tb.Fatal(err)
		}
		for j := 0; j < filesPerFolder; j++ {
			if err := fsys.WriteFile(fmt.Sprintf("%s/Report-%04d.TXT", dir, j), []byte("x"), 0644); err != nil {
				tb.Fatal(err)
			}
		}
	}
	return NewSearchService(fsys, SearchServiceConfig{
		MountPoints: []model.MountPoint{{Name: "media", Path: "/data/media"}},
	})
}

// A query that matches almost nothing, so the cost is the walk plus the per-entry name test.
func BenchmarkSearchWalk(b *testing.B) {
	svc := setupBenchTree(b, 50, 100)
	ctx := context.Background()
	b.ReportAllocs()
	for b.Loop() {
		if _, err := svc.Search(ctx, "media", "zzz"); err != nil {
			b.Fatal(err)
		}
	}
}

// Allocation counts are deterministic, unlike wall-clock time, so they can gate CI.
// Lower the ceiling when an optimisation lands; never raise it to make a change pass.
func TestSearchAllocationCeiling(t *testing.T) {
	svc := setupBenchTree(t, 10, 100)
	ctx := context.Background()
	const ceiling = 4300
	allocs := testing.AllocsPerRun(5, func() {
		if _, err := svc.Search(ctx, "media", "zzz"); err != nil {
			t.Fatal(err)
		}
	})
	t.Logf("allocs=%.0f", allocs)
	if allocs > ceiling {
		t.Errorf("%.0f allocs per search exceeds ceiling %d", allocs, ceiling)
	}
}
