package service

import (
	"context"
	"fmt"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func BenchmarkDirectoryListing(b *testing.B) {
	for _, count := range []int{1_000, 10_000} {
		b.Run(fmt.Sprintf("%d_entries", count), func(b *testing.B) {
			fs := filesystem.NewOsFS()
			dir := b.TempDir()
			svc := NewFileService(fs, FileServiceConfig{MountPoints: []model.MountPoint{{Name: "benchmark", Path: dir}}})
			for i := range count {
				if err := fs.WriteFile(filepath.Join(dir, fmt.Sprintf("Project File %05d.MKV", i)), nil, 0644); err != nil {
					b.Fatal(err)
				}
			}

			ctx := context.Background()
			b.ReportAllocs()
			b.ResetTimer()
			for b.Loop() {
				list, err := svc.List(ctx, "benchmark")
				if err != nil {
					b.Fatal(err)
				}
				if list.Path != "benchmark" || len(list.Items) != count {
					b.Fatalf("unexpected listing: path %q, %d items; want benchmark and %d", list.Path, len(list.Items), count)
				}
			}
		})
	}
}
