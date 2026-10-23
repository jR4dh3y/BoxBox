package service

import (
	"context"
	"fmt"
	"mime"
	"testing"
)

const benchDirectoryEntries = 5000

func setupBenchDirectory(tb testing.TB, entries int) FileService {
	tb.Helper()
	svc, fs := setupTestFileService()
	dir := "/data/media/bench"
	if err := fs.MkdirAll(dir, 0755); err != nil {
		tb.Fatal(err)
	}
	for i := range entries {
		name := fmt.Sprintf("%s/File-%05d.TXT", dir, (i*7919)%entries)
		if err := fs.WriteFile(name, []byte("x"), 0644); err != nil {
			tb.Fatal(err)
		}
	}
	return svc
}

// One folder open: the client receives every entry and sorts locally.
func BenchmarkListDirectory(b *testing.B) {
	svc := setupBenchDirectory(b, benchDirectoryEntries)
	ctx := context.Background()
	// MIME tables initialize once per process, not once per directory listing.
	mime.TypeByExtension(".TXT")
	b.ReportAllocs()
	for b.Loop() {
		count, err := listWholeFolder(ctx, svc, "media/bench")
		if err != nil {
			b.Fatal(err)
		}
		if count != benchDirectoryEntries {
			b.Fatalf("got %d items, want %d", count, benchDirectoryEntries)
		}
	}
}

// Allocation counts are deterministic, unlike wall-clock time, so they can gate CI.
// Lower the ceiling when an optimisation lands; never raise it to make a change pass.
func TestListDirectoryAllocationCeiling(t *testing.T) {
	const entries = 1000
	svc := setupBenchDirectory(t, entries)
	ctx := context.Background()
	ceiling := allocationCeiling(t, "list")
	allocs := testing.AllocsPerRun(5, func() {
		count, err := listWholeFolder(ctx, svc, "media/bench")
		if err != nil {
			t.Fatal(err)
		}
		if count != entries {
			t.Fatalf("got %d items, want %d", count, entries)
		}
	})
	t.Logf("allocs=%.0f ceiling=%.0f", allocs, ceiling)
	if allocs > ceiling {
		t.Errorf("%.0f allocs per List of %d entries exceeds ceiling %.0f", allocs, entries, ceiling)
	}
}
