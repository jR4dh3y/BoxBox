package filesystem

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"testing"
)

func TestAferoFSKeepsSafePathsAndRejectsTraversal(t *testing.T) {
	fsys := NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/media/safe.txt", []byte("safe"), 0o644); err != nil {
		t.Fatal(err)
	}

	for _, path := range []string{"/data/media/safe.txt", "data/media/safe.txt"} {
		data, err := fsys.ReadFile(path)
		if err != nil {
			t.Fatalf("read %q: %v", path, err)
		}
		if string(data) != "safe" {
			t.Fatalf("read %q = %q, want safe", path, data)
		}
	}

	for _, path := range []string{
		"../data/media/safe.txt",
		"/data/media/../secret.txt",
		`/data/media\..\secret.txt`,
		"/data/media/\x00secret.txt",
	} {
		_, err := fsys.ReadFile(path)
		if !errors.Is(err, errInvalidFilesystemPath) {
			t.Errorf("read %q error = %v, want invalid filesystem path", path, err)
		}
	}
}

func TestRenameNoReplaceDoesNotPublishWhenHardLinksAreUnsupported(t *testing.T) {
	directory := t.TempDir()
	source := filepath.Join(directory, "staged.txt")
	destination := filepath.Join(directory, "uploaded.txt")
	if err := os.WriteFile(source, []byte("complete upload"), 0o640); err != nil {
		t.Fatal(err)
	}

	errLinkUnsupported := errors.New("hard links unsupported")
	err := renameNoReplaceWithLink(source, destination, func(string, string) error {
		return errLinkUnsupported
	})
	if !errors.Is(err, errLinkUnsupported) {
		t.Fatalf("rename error = %v, want hard-link error", err)
	}
	data, err := os.ReadFile(source)
	if err != nil || string(data) != "complete upload" {
		t.Fatalf("staged source after failure = %q, error = %v", data, err)
	}
	if _, err := os.Stat(destination); !errors.Is(err, fs.ErrNotExist) {
		t.Fatalf("destination appeared before atomic publication: %v", err)
	}
}

func TestRenameNoReplacePublishesAtomicallyWithoutOverwriting(t *testing.T) {
	directory := t.TempDir()
	source := filepath.Join(directory, "staged.txt")
	destination := filepath.Join(directory, "uploaded.txt")
	if err := os.WriteFile(source, []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, []byte("existing"), 0o600); err != nil {
		t.Fatal(err)
	}

	fsys := NewOsFS()
	err := fsys.RenameNoReplace(source, destination)
	if !errors.Is(err, fs.ErrExist) {
		t.Fatalf("rename error = %v, want fs.ErrExist", err)
	}
	data, err := os.ReadFile(destination)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "existing" {
		t.Fatalf("existing destination = %q, want unchanged", data)
	}
	if err := os.Remove(destination); err != nil {
		t.Fatal(err)
	}
	if err := fsys.RenameNoReplace(source, destination); err != nil {
		t.Fatalf("publish complete upload: %v", err)
	}
	if _, err := os.Stat(source); !errors.Is(err, fs.ErrNotExist) {
		t.Fatalf("source remains after successful publish: %v", err)
	}
	data, err = os.ReadFile(destination)
	if err != nil || string(data) != "new" {
		t.Fatalf("published destination = %q, error = %v", data, err)
	}
}

func TestAferoFSLstatDoesNotFollowSymlinks(t *testing.T) {
	directory := t.TempDir()
	target := filepath.Join(directory, "target.txt")
	link := filepath.Join(directory, "link.txt")
	if err := os.WriteFile(target, []byte("target"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, link); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}

	info, err := NewOsFS().Lstat(link)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("Lstat mode = %v, want symlink", info.Mode())
	}
}

func TestMkdirAllWithinConfinesBothFilesystems(t *testing.T) {
	for _, makeFS := range []struct {
		name string
		new  func() *AferoFS
	}{{"disk", NewOsFS}, {"memory", NewMemMapFS}} {
		t.Run(makeFS.name, func(t *testing.T) {
			fsys := makeFS.new()
			root := filepath.Join(t.TempDir(), "shared")
			if err := fsys.MkdirAll(root, 0o755); err != nil {
				t.Fatal(err)
			}
			if err := fsys.MkdirAllWithin(root, "parent/child", 0o755); err != nil {
				t.Fatal(err)
			}
			if isDir, err := fsys.IsDir(filepath.Join(root, "parent/child")); err != nil || !isDir {
				t.Fatalf("nested folder missing: %v", err)
			}
			for _, path := range []string{"../escape", "/escape", "parent/../../escape", "\x00escape"} {
				if err := fsys.MkdirAllWithin(root, path, 0o755); err == nil {
					t.Errorf("unsafe relative path accepted: %q", path)
				}
			}
		})
	}
}

func TestReadDirFileInfoLimitReportsTruncation(t *testing.T) {
	fsys := NewMemMapFS()
	if err := fsys.MkdirAll("/data", 0o755); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"a.txt", "b.txt", "c.txt"} {
		if err := fsys.WriteFile("/data/"+name, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}

	entries, truncated, err := fsys.ReadDirFileInfoLimit("/data", 2)
	if err != nil {
		t.Fatal(err)
	}
	if !truncated || len(entries) != 0 {
		t.Fatalf("limited read returned %d entries, truncated=%v; want no entries and true", len(entries), truncated)
	}

	entries, truncated, err = fsys.ReadDirFileInfoLimit("/data", 3)
	if err != nil {
		t.Fatal(err)
	}
	if truncated || len(entries) != 3 {
		t.Fatalf("exact-limit read returned %d entries, truncated=%v; want 3 and false", len(entries), truncated)
	}
}

func TestReadDirFileInfoLimitCountsRawOSEntries(t *testing.T) {
	fsys := NewOsFS()
	dir := t.TempDir()
	for _, name := range []string{"a.txt", "b.txt", "c.txt"} {
		if err := os.WriteFile(filepath.Join(dir, name), nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}

	entries, truncated, err := fsys.ReadDirFileInfoLimit(dir, 2)
	if err != nil {
		t.Fatal(err)
	}
	if !truncated || len(entries) != 0 {
		t.Fatalf("limited OS read returned %d entries, truncated=%v; want no entries and true", len(entries), truncated)
	}

	entries, truncated, err = fsys.ReadDirFileInfoLimit(dir, 3)
	if err != nil {
		t.Fatal(err)
	}
	if truncated || len(entries) != 3 {
		t.Fatalf("exact-limit OS read returned %d entries, truncated=%v; want 3 and false", len(entries), truncated)
	}
}

func TestRemoveFileWithinKeepsEntryThatReplacesPathDuringRemoval(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "file.txt")
	if err := os.WriteFile(target, []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	original, err := os.Lstat(target)
	if err != nil {
		t.Fatal(err)
	}
	errChanged := errors.New("changed")
	calls := 0
	err = NewOsFS().RemoveFileWithin(root, "file.txt", func(info fs.FileInfo) error {
		calls++
		if calls == 1 {
			// A save publishes a new version right after the first check passes.
			staged := filepath.Join(root, "staged.tmp")
			if err := os.WriteFile(staged, []byte("saved"), 0o600); err != nil {
				t.Fatal(err)
			}
			if err := os.Rename(staged, target); err != nil {
				t.Fatal(err)
			}
		}
		if !os.SameFile(original, info) {
			return errChanged
		}
		return nil
	})
	if !errors.Is(err, errChanged) {
		t.Fatalf("RemoveFileWithin() error = %v, want %v", err, errChanged)
	}
	data, err := os.ReadFile(target)
	if err != nil || string(data) != "saved" {
		t.Fatalf("saved version = %q, %v; want it kept", data, err)
	}
	assertOnlyEntries(t, root, "file.txt")
}

func TestRemoveFileWithinRemovesVerifiedEntry(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "media"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "media", "file.txt"), []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	err := NewOsFS().RemoveFileWithin(root, filepath.Join("media", "file.txt"), func(fs.FileInfo) error { return nil })
	if err != nil {
		t.Fatalf("RemoveFileWithin() = %v", err)
	}
	assertOnlyEntries(t, filepath.Join(root, "media"))
}

func assertOnlyEntries(t *testing.T, directory string, want ...string) {
	t.Helper()
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatal(err)
	}
	var got []string
	for _, entry := range entries {
		got = append(got, entry.Name())
	}
	if len(got) != len(want) {
		t.Fatalf("entries in %s = %v, want %v", directory, got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("entries in %s = %v, want %v", directory, got, want)
		}
	}
}

func TestRemoveFileWithinNeverRestoresOverANewEntry(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "file.txt")
	if err := os.WriteFile(target, []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	errChanged := errors.New("changed")
	calls := 0
	err := NewOsFS().RemoveFileWithin(root, "file.txt", func(fs.FileInfo) error {
		calls++
		if calls == 1 {
			return nil
		}
		// A new file takes the name while the checked entry is aside.
		if err := os.WriteFile(target, []byte("new"), 0o600); err != nil {
			t.Fatal(err)
		}
		return errChanged
	})
	if !errors.Is(err, errChanged) || !errors.Is(err, fs.ErrExist) {
		t.Fatalf("RemoveFileWithin() error = %v, want %v and fs.ErrExist", err, errChanged)
	}
	data, err := os.ReadFile(target)
	if err != nil || string(data) != "new" {
		t.Fatalf("new file = %q, %v; want it untouched", data, err)
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 2 {
		t.Fatalf("entries = %v, want the new file and the entry left aside", entries)
	}
}

func TestRemoveFileWithinRestoresIntoRenamedFolder(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, "a"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "a", "file.txt"), []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	errChanged := errors.New("changed")
	calls := 0
	err := NewOsFS().RemoveFileWithin(root, filepath.Join("a", "file.txt"), func(fs.FileInfo) error {
		calls++
		if calls == 1 {
			return nil
		}
		// Another process renames the folder after the file was moved aside.
		if err := os.Rename(filepath.Join(root, "a"), filepath.Join(root, "b")); err != nil {
			t.Fatal(err)
		}
		return errChanged
	})
	if !errors.Is(err, errChanged) {
		t.Fatalf("RemoveFileWithin() error = %v, want %v", err, errChanged)
	}
	data, err := os.ReadFile(filepath.Join(root, "b", "file.txt"))
	if err != nil || string(data) != "shared" {
		t.Fatalf("restored file = %q, %v", data, err)
	}
	assertOnlyEntries(t, filepath.Join(root, "b"), "file.txt")
}

func TestRemoveFileWithinRestoresRejectedEntry(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "file.txt")
	if err := os.WriteFile(target, []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	errChanged := errors.New("changed")
	calls := 0
	err := NewOsFS().RemoveFileWithin(root, "file.txt", func(fs.FileInfo) error {
		calls++
		if calls == 1 {
			return nil
		}
		return errChanged
	})
	if !errors.Is(err, errChanged) {
		t.Fatalf("RemoveFileWithin() error = %v, want %v", err, errChanged)
	}
	data, err := os.ReadFile(target)
	if err != nil || string(data) != "shared" {
		t.Fatalf("restored file = %q, %v", data, err)
	}
	assertOnlyEntries(t, root, "file.txt")
}

func TestRemoveFileWithinKeepsFileWhoseFolderMovedBeforeRemoval(t *testing.T) {
	root := t.TempDir()
	for _, folder := range []string{"a", "shared"} {
		if err := os.Mkdir(filepath.Join(root, folder), 0o700); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "a", "file.txt"), []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	err := NewOsFS().RemoveFileWithin(root, filepath.Join("a", "file.txt"), func(fs.FileInfo) error {
		// A user moves the folder into a shared folder after it was checked.
		if _, err := os.Stat(filepath.Join(root, "a")); err == nil {
			if err := os.Rename(filepath.Join(root, "a"), filepath.Join(root, "shared", "a")); err != nil {
				t.Fatal(err)
			}
		}
		return nil
	})
	if err == nil {
		t.Fatal("RemoveFileWithin() removed a file whose folder moved")
	}
	data, err := os.ReadFile(filepath.Join(root, "shared", "a", "file.txt"))
	if err != nil || string(data) != "shared" {
		t.Fatalf("moved file = %q, %v; want it kept", data, err)
	}
	assertOnlyEntries(t, filepath.Join(root, "shared", "a"), "file.txt")
}

func TestRemoveFileWithinKeepsFileWhoseFolderBecameASymlink(t *testing.T) {
	root := t.TempDir()
	for _, folder := range []string{"a", "shared"} {
		if err := os.Mkdir(filepath.Join(root, folder), 0o700); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "a", "file.txt"), []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	calls := 0
	err := NewOsFS().RemoveFileWithin(root, filepath.Join("a", "file.txt"), func(fs.FileInfo) error {
		calls++
		if calls == 1 {
			// The folder moves into a shared folder and its old name becomes a
			// symlink to the new place.
			if err := os.Rename(filepath.Join(root, "a"), filepath.Join(root, "shared", "a")); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(filepath.Join("shared", "a"), filepath.Join(root, "a")); err != nil {
				t.Skipf("symlinks unavailable: %v", err)
			}
		}
		return nil
	})
	if err == nil {
		t.Fatal("RemoveFileWithin() removed a file whose folder became a symlink")
	}
	data, err := os.ReadFile(filepath.Join(root, "shared", "a", "file.txt"))
	if err != nil || string(data) != "shared" {
		t.Fatalf("moved file = %q, %v; want it kept", data, err)
	}
	assertOnlyEntries(t, filepath.Join(root, "shared", "a"), "file.txt")
}

func TestRemoveFileWithinKeepsFileWhoseFolderMovesDuringTheFinalCheck(t *testing.T) {
	root := t.TempDir()
	for _, folder := range []string{"a", "shared"} {
		if err := os.Mkdir(filepath.Join(root, folder), 0o700); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "a", "file.txt"), []byte("shared"), 0o600); err != nil {
		t.Fatal(err)
	}
	calls := 0
	err := NewOsFS().RemoveFileWithin(root, filepath.Join("a", "file.txt"), func(fs.FileInfo) error {
		calls++
		if calls == 2 {
			if err := os.Rename(filepath.Join(root, "a"), filepath.Join(root, "shared", "a")); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(filepath.Join("shared", "a"), filepath.Join(root, "a")); err != nil {
				t.Skipf("symlinks unavailable: %v", err)
			}
		}
		return nil
	})
	if err == nil {
		t.Fatal("RemoveFileWithin() removed a file whose folder moved during the final check")
	}
	data, err := os.ReadFile(filepath.Join(root, "shared", "a", "file.txt"))
	if err != nil || string(data) != "shared" {
		t.Fatalf("moved file = %q, %v; want it kept", data, err)
	}
	assertOnlyEntries(t, filepath.Join(root, "shared", "a"), "file.txt")
}
