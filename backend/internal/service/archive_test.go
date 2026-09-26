package service

import (
	"archive/zip"
	"bytes"
	"context"
	"errors"
	"io"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestDirectoryArchiveSanitizesWindowsVolumeSyntax(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media/C:/C:", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/media/C:/root.txt", []byte("root"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/media/C:/C:/nested.txt", []byte("nested"), 0o644); err != nil {
		t.Fatal(err)
	}

	archive, err := prepareDirectoryArchive(context.Background(), fsys, "/data/media/C:", "/data/media")
	if err != nil {
		t.Fatal(err)
	}
	if archive.Name != "C%3A" {
		t.Fatalf("archive name = %q, want C%%3A", archive.Name)
	}

	var output bytes.Buffer
	if err := archive.WriteTo(context.Background(), &output); err != nil {
		t.Fatal(err)
	}
	reader, err := zip.NewReader(bytes.NewReader(output.Bytes()), int64(output.Len()))
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]bool{
		"C%3A/":                true,
		"C%3A/root.txt":        true,
		"C%3A/C%3A/":           true,
		"C%3A/C%3A/nested.txt": true,
	}
	for _, entry := range reader.File {
		if entry.Name != "" && want[entry.Name] {
			delete(want, entry.Name)
		}
		if strings.Contains(entry.Name, ":") || strings.HasPrefix(entry.Name, "C:/") {
			t.Fatalf("unsafe archive entry name %q", entry.Name)
		}
	}
	if len(want) != 0 {
		t.Fatalf("archive is missing expected entries: %v", want)
	}
}

func TestDirectoryArchiveEscapesDistinctNamesWithoutCollisions(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media/folder", 0o755); err != nil {
		t.Fatal(err)
	}
	files := map[string]string{
		"a:b.txt":   "colon",
		"a_b.txt":   "underscore",
		"a%3Ab.txt": "percent",
	}
	for name, content := range files {
		if err := fsys.WriteFile("/data/media/folder/"+name, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	archive, err := prepareDirectoryArchive(context.Background(), fsys, "/data/media/folder", "/data/media")
	if err != nil {
		t.Fatal(err)
	}
	var output bytes.Buffer
	if err := archive.WriteTo(context.Background(), &output); err != nil {
		t.Fatal(err)
	}
	reader, err := zip.NewReader(bytes.NewReader(output.Bytes()), int64(output.Len()))
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]string{
		"folder/a%3Ab.txt":   "colon",
		"folder/a_b.txt":     "underscore",
		"folder/a%253Ab.txt": "percent",
	}
	for _, entry := range reader.File {
		if entry.FileInfo().IsDir() {
			continue
		}
		expected, ok := want[entry.Name]
		if !ok {
			t.Fatalf("unexpected or duplicate archive entry %q", entry.Name)
		}
		file, err := entry.Open()
		if err != nil {
			t.Fatal(err)
		}
		content, readErr := io.ReadAll(file)
		closeErr := file.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read archive item: read=%v close=%v", readErr, closeErr)
		}
		if string(content) != expected {
			t.Fatalf("archive content for %q = %q, want %q", entry.Name, content, expected)
		}
		delete(want, entry.Name)
	}
	if len(want) != 0 {
		t.Fatalf("archive is missing entries: %v", want)
	}
}

func TestDirectoryArchiveRejectsCaseInsensitiveNameCollisions(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media/folder", 0o755); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"Foo.txt", "foo.txt"} {
		if err := fsys.WriteFile("/data/media/folder/"+name, []byte(name), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	if _, err := prepareDirectoryArchive(context.Background(), fsys, "/data/media/folder", "/data/media"); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("prepare archive error = %v, want %v", err, ErrInvalidOperation)
	}
}

func TestDirectoryArchiveRejectsWindowsTrailingDotCollisions(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media/folder", 0o755); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"report", "report."} {
		if err := fsys.WriteFile("/data/media/folder/"+name, []byte(name), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	if _, err := prepareDirectoryArchive(context.Background(), fsys, "/data/media/folder", "/data/media"); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("prepare archive error = %v, want %v", err, ErrInvalidOperation)
	}
}

func TestArchiveCollisionKeyRejectsWindowsDotSegments(t *testing.T) {
	for _, name := range []string{"folder/. /file.txt", "folder/.. /file.txt", ".. "} {
		if _, ok := archiveCollisionKey(name); ok {
			t.Errorf("archiveCollisionKey(%q) accepted a Windows dot segment", name)
		}
	}
}
