package service

import (
	"archive/zip"
	"bytes"
	"context"
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

func TestDirectoryArchivePreservesNamesThatCollideOnWindows(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	for _, path := range []string{
		"/data/media/folder",
		"/data/media/folder/Group",
		"/data/media/folder/group",
	} {
		if err := fsys.MkdirAll(path, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	files := map[string]string{
		"Foo.txt":          "uppercase",
		"foo.txt":          "lowercase",
		"report":           "plain extensionless",
		"report.":          "trailing dot",
		"CON.txt":          "reserved uppercase",
		"con.txt":          "reserved lowercase",
		"Group/inside.txt": "first directory",
		"group/inside.txt": "second directory",
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
	want := make(map[string]bool, len(files))
	for _, content := range files {
		want[content] = true
	}
	seenNames := make(map[string]struct{}, len(files))
	for _, entry := range reader.File {
		if entry.FileInfo().IsDir() {
			continue
		}
		key := archiveNameKey(entry.Name)
		if _, exists := seenNames[key]; exists {
			t.Fatalf("portable ZIP path collision at %q", entry.Name)
		}
		seenNames[key] = struct{}{}
		file, err := entry.Open()
		if err != nil {
			t.Fatal(err)
		}
		content, readErr := io.ReadAll(file)
		closeErr := file.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read %q: read=%v close=%v", entry.Name, readErr, closeErr)
		}
		if !want[string(content)] {
			t.Fatalf("unexpected or duplicate file content %q in %q", content, entry.Name)
		}
		delete(want, string(content))
	}
	if len(want) != 0 {
		t.Fatalf("archive omitted valid files: %v", want)
	}
}

func TestArchiveComponentsRemainSafeForWindowsExtraction(t *testing.T) {
	allocator := newArchiveNameAllocator()
	root := sanitizeArchiveComponent("folder")
	for _, component := range []string{".", "..", ".. ", "report."} {
		got := allocator.component(root, component)
		if got == "" || got == "." || got == ".." || strings.TrimRight(got, " .") == "" {
			t.Errorf("component %q produced unsafe ZIP name %q", component, got)
		}
	}
	for _, component := range []string{"CON", "con.txt", "nul", "COM1.log"} {
		got := sanitizeArchiveComponent(component)
		if isWindowsReservedArchiveName(got) {
			t.Errorf("reserved device %q stayed reserved as %q", component, got)
		}
	}
}
