package service

import "context"

// listWholeFolder is one folder open as the client performs it. scripts/perf/backend.ts replaces
// this file in a base revision that still pages, so both revisions measure the same work.
func listWholeFolder(ctx context.Context, svc FileService, path string) (int, error) {
	list, err := svc.List(ctx, path)
	if err != nil {
		return 0, err
	}
	return len(list.Items), nil
}
