package filesystem

import (
	"errors"
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
