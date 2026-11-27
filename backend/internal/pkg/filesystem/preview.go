package filesystem

import (
	"io/fs"
	"sort"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fileutil"
)

func (a *AferoFS) ReadDirPreview(name string, includeHidden bool) ([]fs.FileInfo, error) {
	return ReadDirectoryPreview(a, name, includeHidden)
}

// ReadDirectoryPreview ranks the first 64 entries before asking for their metadata.
// Call it where the filesystem lives so a worker only serializes the selected entries.
func ReadDirectoryPreview(fsys FS, name string, includeHidden bool) ([]fs.FileInfo, error) {
	entries, _, err := fsys.ReadDirLimit(name, 64)
	if err != nil {
		return nil, err
	}
	type candidate struct {
		entry   fs.DirEntry
		folded  string
		picture bool
	}
	shown := make([]candidate, 0, len(entries))
	for _, entry := range entries {
		name := entry.Name()
		if includeHidden || !strings.HasPrefix(name, ".") {
			shown = append(shown, candidate{
				entry:   entry,
				folded:  strings.ToLower(name),
				picture: !entry.IsDir() && fileutil.RendersImageThumbnail(name),
			})
		}
	}
	sort.SliceStable(shown, func(i, j int) bool {
		if shown[i].picture != shown[j].picture {
			return shown[i].picture
		}
		return shown[i].folded < shown[j].folded
	})
	const limit = 3
	result := make([]fs.FileInfo, 0, min(len(shown), limit))
	for _, candidate := range shown {
		info, err := candidate.entry.Info()
		if err != nil {
			continue
		}
		result = append(result, info)
		if len(result) == limit {
			break
		}
	}
	return result, nil
}
