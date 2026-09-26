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

func TestRenameNoReplaceFallsBackWhenHardLinksAreUnsupported(t *testing.T) {
	directory := t.TempDir()
	source := filepath.Join(directory, "staged.txt")
	destination := filepath.Join(directory, "uploaded.txt")
	if err := os.WriteFile(source, []byte("upload"), 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(source, 0o640); err != nil {
		t.Fatal(err)
	}

	errLinkUnsupported := errors.New("hard links unsupported")
	err := renameNoReplaceWithLink(source, destination, func(string, string) error {
		return errLinkUnsupported
	})
	if err != nil {
		t.Fatalf("rename with copy fallback: %v", err)
	}
	if _, err := os.Stat(source); !errors.Is(err, fs.ErrNotExist) {
		t.Fatalf("staged file still exists: %v", err)
	}
	data, err := os.ReadFile(destination)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "upload" {
		t.Fatalf("destination = %q, want upload", data)
	}
	info, err := os.Stat(destination)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o640 {
		t.Fatalf("destination mode = %04o, want 0640", got)
	}
}

func TestRenameNoReplaceFallbackDoesNotOverwrite(t *testing.T) {
	directory := t.TempDir()
	source := filepath.Join(directory, "staged.txt")
	destination := filepath.Join(directory, "uploaded.txt")
	if err := os.WriteFile(source, []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, []byte("existing"), 0o600); err != nil {
		t.Fatal(err)
	}

	err := renameNoReplaceWithLink(source, destination, func(string, string) error {
		return errors.New("hard links unsupported")
	})
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
}
