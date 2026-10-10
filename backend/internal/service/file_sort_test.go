package service

import (
	"fmt"
	"io/fs"
	"sort"
	"strings"
	"testing"
)

// referenceSortEntries is the comparator sortEntries replaced, kept as the oracle for ordering.
func referenceSortEntries(entries []fs.DirEntry, sortBy, sortDir string) {
	infos := make(map[string]fs.FileInfo, len(entries))
	if sortBy == "size" || sortBy == "modTime" {
		for _, entry := range entries {
			if info, err := entry.Info(); err == nil {
				infos[entry.Name()] = info
			}
		}
	}
	lessFor := func(entry fs.DirEntry) fs.FileInfo {
		if info, ok := infos[entry.Name()]; ok {
			return info
		}
		info, _ := entry.Info()
		return info
	}
	sort.Slice(entries, func(i, j int) bool {
		iName := entries[i].Name()
		jName := entries[j].Name()
		iHidden := strings.HasPrefix(iName, ".")
		jHidden := strings.HasPrefix(jName, ".")

		if iHidden != jHidden {
			return !iHidden
		}

		comparison := 0
		switch sortBy {
		case "name":
			comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
		case "size":
			iInfo := lessFor(entries[i])
			jInfo := lessFor(entries[j])
			iSize := int64(0)
			jSize := int64(0)
			if iInfo != nil {
				iSize = iInfo.Size()
			}
			if jInfo != nil {
				jSize = jInfo.Size()
			}
			comparison = compareInt64(iSize, jSize)
		case "modTime":
			iInfo := lessFor(entries[i])
			jInfo := lessFor(entries[j])
			if iInfo != nil && jInfo != nil {
				comparison = iInfo.ModTime().Compare(jInfo.ModTime())
			}
		case "type":
			// Directories first, then by name
			iDir := entries[i].IsDir()
			jDir := entries[j].IsDir()
			if iDir != jDir {
				return iDir
			} else {
				comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
			}
		default:
			comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
		}
		if comparison == 0 {
			comparison = strings.Compare(strings.ToLower(iName), strings.ToLower(jName))
		}

		if sortDir == "desc" {
			return comparison > 0
		}
		return comparison < 0
	})
}

func TestSortEntriesKeepsTheReferenceOrder(t *testing.T) {
	svc, fsys := setupTestFileService()
	dir := "/data/media/sorting"
	if err := fsys.MkdirAll(dir, 0755); err != nil {
		t.Fatal(err)
	}
	names := []string{"b.txt", "B.txt", "a.TXT", ".hidden", ".Alpha", "Zed", "zed.md", "10.txt", "9.txt", "éclair.txt", "Éclair2.txt"}
	for i, name := range names {
		if err := fsys.WriteFile(fmt.Sprintf("%s/%s", dir, name), []byte(strings.Repeat("x", i%4)), 0644); err != nil {
			t.Fatal(err)
		}
	}
	for _, folder := range []string{"Folder", "apps", ".config"} {
		if err := fsys.MkdirAll(dir+"/"+folder, 0755); err != nil {
			t.Fatal(err)
		}
	}
	entries, err := fsys.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}

	for _, sortBy := range []string{"name", "type", "size", "modTime", ""} {
		for _, sortDir := range []string{"asc", "desc"} {
			want := append([]fs.DirEntry(nil), entries...)
			got := append([]fs.DirEntry(nil), entries...)
			referenceSortEntries(want, sortBy, sortDir)
			svc.(*fileService).sortEntries(got, sortBy, sortDir)
			for i := range want {
				if want[i].Name() != got[i].Name() {
					t.Fatalf("sort=%q dir=%s: position %d is %q, reference has %q", sortBy, sortDir, i, got[i].Name(), want[i].Name())
				}
			}
		}
	}

}
