// Package validator provides path validation and sanitization utilities.
// This file contains property-based tests for path traversal prevention.
package validator

import (
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// **Feature: boxbox, Property 13: Path Traversal Prevention**
// **Validates: Requirements 7.2**
//
// Property: For any path containing "..", "/../", or URL-encoded traversal sequences,
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

	// Property 2: Double-dot at any position should be rejected
	properties.Property("double-dot anywhere in path is rejected", prop.ForAll(
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

// TestContainsTraversalSequence tests the helper function directly
func TestContainsTraversalSequence(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100

	properties := gopter.NewProperties(parameters)

	// Property: All known traversal patterns should be detected
	properties.Property("known traversal patterns are detected", prop.ForAll(
		func(prefix string, suffix string) bool {
			knownPatterns := []string{
				"..",
				"../",
				"..\\",
				"/..",
				"\\..",
				"%2e%2e",
				"%2e%2e%2f",
				"..%2f",
				"%2e%2e%5c",
				"..%5c",
			}

			for _, pattern := range knownPatterns {
				testPath := prefix + pattern + suffix
				if !containsTraversalSequence(testPath) {
					return false
				}
			}
			return true
		},
		safeSegment(),
		safeSegment(),
	))

	properties.TestingRun(t)
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
