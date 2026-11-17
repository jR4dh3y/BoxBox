// Package validator provides path validation and sanitization utilities.
// Focused tests for path traversal prevention.
package validator

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// TestContainsTraversalSequence checks path components, not dot substrings.
func TestContainsTraversalSequence(t *testing.T) {
	for _, path := range []string{"..", "../secret", "parent/../secret", `parent\..\secret`} {
		if !containsTraversalSequence(path) {
			t.Errorf("expected parent component in %q to be detected", path)
		}
	}

	for _, path := range []string{"report..txt", "...hidden", "folder/report..txt", "folder/...hidden"} {
		if containsTraversalSequence(path) {
			t.Errorf("valid dot substring in %q was treated as traversal", path)
		}
	}
}

func TestValidatePathAgainstMountsRejectsTraversalBeforeMountMatching(t *testing.T) {
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "backup", Path: "/data/backup", ReadOnly: false},
	}

	tests := []string{
		"media/../backup/file.txt",
		"media/%2e%2e/backup/file.txt",
		"media/subdir/../../backup/file.txt",
		"media/%252e%252e/backup/file.txt",
		"media/%252e%252e%252fbackup/file.txt",
		"media/subdir/%252e%252e/%252e%252e/backup/file.txt",
	}

	for _, requestedPath := range tests {
		t.Run(requestedPath, func(t *testing.T) {
			_, _, err := ValidatePathAgainstMounts(requestedPath, mounts)
			if err != ErrPathTraversal {
				t.Fatalf("expected %v, got %v", ErrPathTraversal, err)
			}
		})
	}
}

func TestSanitizePathRejectsEncodedAndSeparatorEscapes(t *testing.T) {
	tests := []string{
		"../secret.txt",
		"%2e%2e/secret.txt",
		`media\..\secret.txt`,
		"/../../secret.txt",
		"%252e%252e/secret.txt",
		"%252e%252e%252fsecret.txt",
		"%252e%252e%255csecret.txt",
	}

	for _, requestedPath := range tests {
		t.Run(requestedPath, func(t *testing.T) {
			if _, err := SanitizePath("/data", requestedPath); err == nil {
				t.Fatalf("expected %q to be rejected", requestedPath)
			}
		})
	}
}

func TestSanitizePathRejectsNULAfterDecoding(t *testing.T) {
	for _, requestedPath := range []string{"secret\x00.txt", "%00secret.txt", "%2500secret.txt"} {
		t.Run(requestedPath, func(t *testing.T) {
			if _, err := SanitizePath("/data", requestedPath); err == nil {
				t.Fatalf("expected NUL path %q to be rejected", requestedPath)
			}
		})
	}
}

func TestSanitizePathAcceptsDotSubstrings(t *testing.T) {
	for _, requestedPath := range []string{"report..txt", "...hidden", "folder/report..txt", "folder/...hidden"} {
		t.Run(requestedPath, func(t *testing.T) {
			got, err := SanitizePath("/data", requestedPath)
			if err != nil {
				t.Fatalf("SanitizePath(%q): %v", requestedPath, err)
			}
			want := filepath.Join("/data", requestedPath)
			if got != want {
				t.Fatalf("SanitizePath(%q) = %q, want %q", requestedPath, got, want)
			}
		})
	}
}

func TestValidatePathAgainstMountsAcceptsSafeAbsoluteVirtualPath(t *testing.T) {
	mounts := []model.MountPoint{{Name: "media", Path: "/data/media"}}

	_, got, err := ValidatePathAgainstMounts("/media/notes.txt", mounts)
	if err != nil {
		t.Fatal(err)
	}
	if got != "/data/media/notes.txt" {
		t.Fatalf("resolved path = %q, want /data/media/notes.txt", got)
	}
}

func TestValidatePathAgainstMountsAcceptsDotSubstrings(t *testing.T) {
	mounts := []model.MountPoint{{Name: "media", Path: "/data/media"}}

	for _, requestedPath := range []string{"media/report..txt", "media/...hidden"} {
		t.Run(requestedPath, func(t *testing.T) {
			_, got, err := ValidatePathAgainstMounts(requestedPath, mounts)
			if err != nil {
				t.Fatalf("ValidatePathAgainstMounts(%q): %v", requestedPath, err)
			}
			want := filepath.Join("/data/media", strings.TrimPrefix(requestedPath, "media/"))
			if got != want {
				t.Fatalf("resolved path = %q, want %q", got, want)
			}
		})
	}
}

func TestValidatePathAgainstMountsRejectsNULAfterDecoding(t *testing.T) {
	mounts := []model.MountPoint{{Name: "media", Path: "/data/media"}}

	for _, requestedPath := range []string{"media/bad\x00name", "media/%00name", "media/%2500name"} {
		t.Run(requestedPath, func(t *testing.T) {
			if _, _, err := ValidatePathAgainstMounts(requestedPath, mounts); err == nil {
				t.Fatalf("expected NUL path %q to be rejected", requestedPath)
			}
		})
	}
}
