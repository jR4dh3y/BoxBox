package service

import (
	"context"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
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
			names := make([]string, count)
			for i := count - 1; i >= 0; i-- {
				prefix := "Project"
				if i%2 != 0 {
					prefix = "project"
				}
				names[i] = fmt.Sprintf("%s File %05d.MKV", prefix, i)
				if err := fs.WriteFile(filepath.Join(dir, names[i]), nil, 0644); err != nil {
					b.Fatal(err)
				}
			}
			sort.Slice(names, func(i, j int) bool {
				return strings.ToLower(names[i]) < strings.ToLower(names[j])
			})

			ctx := context.Background()
			for _, page := range []int{1, 2} {
				page := page
				b.Run(fmt.Sprintf("page_%d", page), func(b *testing.B) {
					opts := model.ListOptions{Page: page, PageSize: 50, SortBy: "name", SortDir: "asc"}
					b.ReportAllocs()
					b.ResetTimer()
					for i := 0; i < b.N; i++ {
						list, err := svc.List(ctx, "benchmark", opts)
						if err != nil {
							b.Fatal(err)
						}
						if list.TotalCount != count || len(list.Items) != 50 || list.Page != page {
							b.Fatalf("unexpected listing: page %d, %d total, %d items", list.Page, list.TotalCount, len(list.Items))
						}
						for i, item := range list.Items {
							if want := names[(page-1)*50+i]; item.Name != want {
								b.Fatalf("unexpected item %d: got %q, want %q", i, item.Name, want)
							}
						}
					}
				})
			}

			b.Run("full_traversal", func(b *testing.B) {
				opts := model.ListOptions{Page: 1, PageSize: 50, SortBy: "name", SortDir: "asc"}
				b.ReportAllocs()
				b.ResetTimer()
				for i := 0; i < b.N; i++ {
					seen := 0
					for page := 1; seen < count; page++ {
						opts.Page = page
						list, err := svc.List(ctx, "benchmark", opts)
						if err != nil {
							b.Fatal(err)
						}
						wantLen := min(50, count-seen)
						if list.TotalCount != count || list.Page != page || len(list.Items) != wantLen {
							b.Fatalf("unexpected listing at page %d: %d total, %d items; want %d", list.Page, list.TotalCount, len(list.Items), wantLen)
						}
						for j, item := range list.Items {
							if want := names[seen+j]; item.Name != want {
								b.Fatalf("unexpected item %d on page %d: got %q, want %q", j, page, item.Name, want)
							}
						}
						seen += len(list.Items)
					}
					if seen != count {
						b.Fatalf("unexpected traversal count: got %d, want %d", seen, count)
					}
				}
			})
		})
	}
}
