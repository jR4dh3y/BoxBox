package service

import (
	"context"
	"fmt"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

const benchDirectoryEntries = 5000

func setupBenchDirectory(tb testing.TB, entries int) FileService {
	tb.Helper()
	svc, fs := setupTestFileService()
	dir := "/data/media/bench"
	if err := fs.MkdirAll(dir, 0755); err != nil {
		tb.Fatal(err)
	}
	for i := 0; i < entries; i++ {
		name := fmt.Sprintf("%s/File-%05d.TXT", dir, (i*7919)%entries)
		if err := fs.WriteFile(name, []byte("x"), 0644); err != nil {
			tb.Fatal(err)
		}
	}
	return svc
}

func BenchmarkListDirectory(b *testing.B) {
	svc := setupBenchDirectory(b, benchDirectoryEntries)
	ctx := context.Background()
	for _, sortBy := range []string{"name", "type", "size", "modTime"} {
		b.Run(sortBy, func(b *testing.B) {
			opts := model.ListOptions{Page: 1, PageSize: 50, SortBy: sortBy, SortDir: "asc"}
			b.ReportAllocs()
			for b.Loop() {
				if _, err := svc.List(ctx, "media/bench", opts); err != nil {
					b.Fatal(err)
				}
			}
		})
	}
}

// Allocation counts are deterministic, unlike wall-clock time, so they can gate CI.
// Lower the ceiling when an optimisation lands; never raise it to make a change pass.
func TestListDirectoryAllocationCeiling(t *testing.T) {
	const entries = 1000
	svc := setupBenchDirectory(t, entries)
	ctx := context.Background()
	ceilings := map[string]float64{"name": 2300, "type": 2300, "size": 2300, "modTime": 2300}

	for sortBy, ceiling := range ceilings {
		opts := model.ListOptions{Page: 1, PageSize: 50, SortBy: sortBy, SortDir: "asc"}
		allocs := testing.AllocsPerRun(5, func() {
			if _, err := svc.List(ctx, "media/bench", opts); err != nil {
				t.Fatal(err)
			}
		})
		t.Logf("sort=%s allocs=%.0f ceiling=%.0f", sortBy, allocs, ceiling)
		if allocs > ceiling {
			t.Errorf("sort=%s: %.0f allocs per List of %d entries exceeds ceiling %.0f", sortBy, allocs, entries, ceiling)
		}
	}
}
