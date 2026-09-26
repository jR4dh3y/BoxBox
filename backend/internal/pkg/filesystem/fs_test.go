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
