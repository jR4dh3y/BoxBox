// Package validator provides path validation and sanitization utilities.
// It ensures paths are safe from traversal attacks and stay within mount point boundaries.
package validator

import (
	"errors"
	"net/url"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// Errors returned by path validation functions
var (
	ErrPathTraversal      = errors.New("path traversal detected")
	ErrEmptyPath          = errors.New("path cannot be empty")
	ErrInvalidPath        = errors.New("invalid path")
	ErrOutsideMountPoint  = errors.New("path is outside configured mount points")
	ErrMountPointNotFound = errors.New("mount point not found")
)

// SanitizePath cleans and validates a path against a base path.
// It returns the sanitized full path or an error if the path is invalid.
// This function prevents path traversal attacks by ensuring the resulting
// path stays within the base directory.
func SanitizePath(basePath, requestedPath string) (string, error) {
	if basePath == "" {
		return "", ErrEmptyPath
	}

	decodedPath, err := decodePath(requestedPath)
	if err != nil {
		return "", ErrInvalidPath
	}
	return sanitizeDecodedPath(basePath, decodedPath)
}

func sanitizeDecodedPath(basePath, decodedPath string) (string, error) {
	if strings.ContainsRune(basePath, 0) || strings.ContainsRune(decodedPath, 0) {
		return "", ErrInvalidPath
	}

	// Check for traversal sequences before cleaning
	if containsTraversalSequence(decodedPath) {
		return "", ErrPathTraversal
	}

	// Clean the path to normalize it
	cleaned := filepath.Clean(decodedPath)

	// Check again after cleaning (in case cleaning revealed hidden traversal)
	if containsTraversalSequence(cleaned) {
		return "", ErrPathTraversal
	}

	// Clean the base path as well
	cleanBase := filepath.Clean(basePath)

	// Join with base path
	fullPath := filepath.Join(cleanBase, cleaned)

	// Verify the result is still under the base path
	// Use filepath.Rel to check if fullPath is relative to cleanBase
	rel, err := filepath.Rel(cleanBase, fullPath)
	if err != nil {
		return "", ErrPathTraversal
	}

	// Reject a parent component in the relative result while allowing names like "...hidden".
	if rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", ErrPathTraversal
	}

	return fullPath, nil
}

// containsTraversalSequence reports parent components without rejecting dot substrings.
func containsTraversalSequence(path string) bool {
	// Treat both platform separators as delimiters while preserving dot substrings.
	normalizedPath := strings.ReplaceAll(path, "\\", "/")
	for _, component := range strings.Split(normalizedPath, "/") {
		if component == ".." {
			return true
		}
	}
	return false
}

// decodePath handles a request decoded by the HTTP stack and one extra encoded layer.
func decodePath(path string) (string, error) {
	decoded, err := url.PathUnescape(path)
	if err != nil {
		return "", err
	}
	if strings.ContainsRune(decoded, 0) {
		return "", ErrInvalidPath
	}

	doubleDecoded, err := url.PathUnescape(decoded)
	if err == nil {
		decoded = doubleDecoded
	}
	if strings.ContainsRune(decoded, 0) {
		return "", ErrInvalidPath
	}
	return decoded, nil
}

// ValidatePathAgainstMounts validates that a path is within one of the configured mount points.
// It returns the matching mount point and the resolved full filesystem path.
func ValidatePathAgainstMounts(requestedPath string, mounts []model.MountPoint) (*model.MountPoint, string, error) {
	if requestedPath == "" {
		return nil, "", ErrEmptyPath
	}

	decodedPath, err := decodePath(requestedPath)
	if err != nil {
		return nil, "", ErrInvalidPath
	}
	if containsTraversalSequence(decodedPath) {
		return nil, "", ErrPathTraversal
	}
	// Clean and normalize the requested path
	cleanPath := filepath.Clean(decodedPath)
	if containsTraversalSequence(cleanPath) {
		return nil, "", ErrPathTraversal
	}

	// Remove leading slash for comparison
	cleanPath = strings.TrimPrefix(cleanPath, "/")
	cleanPath = strings.TrimPrefix(cleanPath, "\\")

	// Find the mount point that matches the path prefix
	for i := range mounts {
		mount := &mounts[i]
		mountName := strings.TrimPrefix(mount.Name, "/")

		// Check if the path starts with this mount point name
		if cleanPath == mountName || strings.HasPrefix(cleanPath, mountName+"/") || strings.HasPrefix(cleanPath, mountName+"\\") {
			// Extract the relative path within the mount
			relativePath := strings.TrimPrefix(cleanPath, mountName)
			relativePath = strings.TrimPrefix(relativePath, "/")
			relativePath = strings.TrimPrefix(relativePath, "\\")

			// Sanitize the relative path against the mount's filesystem path
			fullPath, err := sanitizeDecodedPath(mount.Path, relativePath)
			if err != nil {
				return nil, "", err
			}

			return mount, fullPath, nil
		}
	}

	return nil, "", ErrOutsideMountPoint
}

// IsValidFileName checks if a filename is valid (no path separators or special chars)
func IsValidFileName(name string) bool {
	if name == "" {
		return false
	}

	// Check for path separators
	if strings.ContainsAny(name, "/\\") {
		return false
	}

	// Check for special names
	if name == "." || name == ".." {
		return false
	}

	// Check for null bytes
	if strings.ContainsRune(name, 0) {
		return false
	}

	return true
}

// NormalizePath normalizes a path by cleaning it and converting separators
func NormalizePath(path string) string {
	// Clean the path
	cleaned := filepath.Clean(path)

	// Convert to forward slashes for consistency
	normalized := filepath.ToSlash(cleaned)

	return normalized
}
