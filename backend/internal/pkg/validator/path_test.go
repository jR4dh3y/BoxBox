// Package validator provides path validation and sanitization utilities.
// This file contains property-based tests for path traversal prevention.
package validator

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// **Feature: boxbox, Property 13: Path Traversal Prevention**
//
// Property: For any path containing a parent component or encoded traversal,
// the API SHALL reject the request with an error.

// safeSegment generates a safe path segment (letters only, no dots)
func safeSegment() gopter.Gen {
	return gen.Identifier().Map(func(s string) string {
		// Ensure no dots in the segment
		result := strings.ReplaceAll(s, ".", "")
		if len(result) == 0 {
			return "safe"
		}
		if len(result) > 15 {
			return result[:15]
		}
		return result
	})
}

// TestPathTraversalPrevention is a property-based test that verifies
// path traversal attacks are always detected and rejected.
func TestPathTraversalPrevention(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for base paths (valid directory paths)
	basePathGen := gen.OneConstOf(
		"/data",
		"/home/user",
		"/mnt/storage",
		"/var/files",
	)

	// Generator for path traversal sequences
	traversalSequenceGen := gen.OneConstOf(
		"..",
		"../",
		"..\\",
		"/../",
		"\\..\\",
		"..%2f",
		"%2e%2e",
		"%2e%2e%2f",
		"%2e%2e/",
		"..%2F",
		"%2E%2E",
		"%2e%2e%5c",
		"..%5c",
		"%252e%252e",
		"%252e%252e%252f",
		"..%252f",
		"%252E%252E",
		"%252e%252e%255c",
		"..%255c",
	)

	// Property 1: Any path containing traversal sequences should be rejected
	properties.Property("paths with traversal sequences are rejected", prop.ForAll(
		func(basePath string, traversal string, prefix string, suffix string) bool {
			// Construct a path with traversal sequence embedded
			maliciousPath := prefix + "/" + traversal + "/" + suffix

			_, err := SanitizePath(basePath, maliciousPath)

			// The function should return an error for traversal attempts
			return err != nil
		},
		basePathGen,
		traversalSequenceGen,
		safeSegment(),
		safeSegment(),
	))

	// Property 2: A parent component at any position should be rejected
	properties.Property("parent components anywhere in path are rejected", prop.ForAll(
		func(basePath string, seg1 string, seg2 string, seg3 string) bool {
			// Insert ".." in the middle of path segments
			maliciousPath := seg1 + "/../" + seg2 + "/" + seg3

			_, err := SanitizePath(basePath, maliciousPath)

			return err != nil
		},
		basePathGen,
		safeSegment(),
		safeSegment(),
		safeSegment(),
	))

	// Property 3: URL-encoded traversal should be rejected
	properties.Property("URL-encoded traversal sequences are rejected", prop.ForAll(
		func(basePath string, prefix string) bool {
			encodedTraversals := []string{
				prefix + "/%2e%2e/secret",
				prefix + "/%2e%2e%2f/secret",
				prefix + "/..%2f/secret",
				prefix + "/%2E%2E/secret",
				prefix + "/%2e%2e%5c/secret",
				prefix + "/..%5c/secret",
				prefix + "/%252e%252e/secret",
				prefix + "/%252e%252e%252fsecret",
				prefix + "/..%252fsecret",
				prefix + "/%252E%252E/secret",
				prefix + "/%252e%252e%255c/secret",
				prefix + "/..%255csecret",
			}

			for _, path := range encodedTraversals {
				_, err := SanitizePath(basePath, path)
				if err == nil {
					return false // Should have been rejected
				}
			}
			return true
		},
		basePathGen,
		safeSegment(),
	))

	// Property 4: Paths attempting to escape base directory should be rejected
	properties.Property("paths escaping base directory are rejected", prop.ForAll(
		func(basePath string, depth int) bool {
			// Create a path that tries to go up more levels than the base has
			traversals := strings.Repeat("../", depth+5)
			maliciousPath := traversals + "etc/passwd"

			_, err := SanitizePath(basePath, maliciousPath)

			return err != nil
		},
		basePathGen,
		gen.IntRange(1, 10),
	))

	// Property 5: Mixed traversal attempts should be rejected
	properties.Property("mixed traversal patterns are rejected", prop.ForAll(
		func(basePath string, segment string) bool {
			mixedPatterns := []string{
				segment + "/./../../" + segment,
				segment + "/./../.." + segment,
				"./" + segment + "/../../../" + segment,
				segment + "/foo/bar/../../../..",
			}

			for _, path := range mixedPatterns {
				_, err := SanitizePath(basePath, path)
				if err == nil {
					return false
				}
			}
			return true
		},
		basePathGen,
		safeSegment(),
	))

	properties.TestingRun(t)
}

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

// TestSafePathsAreAccepted ensures valid paths without traversal are accepted
func TestSafePathsAreAccepted(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100

	properties := gopter.NewProperties(parameters)

	basePathGen := gen.OneConstOf(
		"/data",
		"/home/user",
		"/mnt/storage",
	)

	// Property: Safe paths should be accepted
	properties.Property("safe paths without traversal are accepted", prop.ForAll(
		func(basePath string, seg1 string, seg2 string, seg3 string) bool {
			safePath := seg1 + "/" + seg2 + "/" + seg3
			result, err := SanitizePath(basePath, safePath)

			// Should succeed
			if err != nil {
				return false
			}

			// Normalize both paths for comparison (handles Windows vs Unix separators)
			normalizedResult := NormalizePath(result)
			normalizedBase := NormalizePath(basePath)

			return strings.HasPrefix(normalizedResult, normalizedBase)
		},
		basePathGen,
		safeSegment(),
		safeSegment(),
		safeSegment(),
	))

	properties.TestingRun(t)
}
