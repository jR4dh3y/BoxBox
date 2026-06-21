// Package middleware provides HTTP middleware for the file manager.
// This file contains property-based tests for security middleware.
package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// **Feature: boxbox, Property 14: Security Headers Presence**
// **Validates: Requirements 7.3**
//
// Property: For any API response, the headers SHALL include X-Content-Type-Options,
// X-Frame-Options, and Content-Security-Policy.

func TestSecurityHeadersPresence(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Create a simple handler that returns 200
	innerHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	})

	// Wrap with security headers middleware
	handler := SecurityHeaders(innerHandler)

	// Generator for HTTP methods
	methodGen := gen.OneConstOf("GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS")

	// Generator for request paths
	pathGen := gen.OneConstOf(
		"/",
		"/api/v1/files",
		"/api/v1/files/documents",
		"/api/v1/files/media/movies",
		"/api/v1/jobs",
		"/api/v1/jobs/123",
		"/api/v1/search",
		"/api/v1/auth/login",
		"/api/v1/ws",
	)

	// Property: All responses include required security headers
	properties.Property("all responses include X-Content-Type-Options header", prop.ForAll(
		func(method string, path string) bool {
			req := httptest.NewRequest(method, path, nil)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Header().Get("X-Content-Type-Options") == "nosniff"
		},
		methodGen,
		pathGen,
	))

	properties.Property("all responses include X-Frame-Options header", prop.ForAll(
		func(method string, path string) bool {
			req := httptest.NewRequest(method, path, nil)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Header().Get("X-Frame-Options") == "DENY"
		},
		methodGen,
		pathGen,
	))

	properties.Property("all responses include Content-Security-Policy header", prop.ForAll(
		func(method string, path string) bool {
			req := httptest.NewRequest(method, path, nil)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			csp := rec.Header().Get("Content-Security-Policy")
			return csp != ""
		},
		methodGen,
		pathGen,
	))

	properties.Property("all responses include X-XSS-Protection header", prop.ForAll(
		func(method string, path string) bool {
			req := httptest.NewRequest(method, path, nil)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Header().Get("X-XSS-Protection") == "1; mode=block"
		},
		methodGen,
		pathGen,
	))

	properties.Property("all responses include Referrer-Policy header", prop.ForAll(
		func(method string, path string) bool {
			req := httptest.NewRequest(method, path, nil)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Header().Get("Referrer-Policy") == "strict-origin-when-cross-origin"
		},
		methodGen,
		pathGen,
	))

	properties.TestingRun(t)
}

// **Feature: boxbox, Property 10: Mount Point Isolation**
// **Validates: Requirements 6.2, 6.3**
//
// Property: For any path not prefixed by a configured mount point name,
// the API SHALL return HTTP 403 status code.

// createTestRouter creates a chi router with mount point guard for testing
func createTestRouter(mounts []model.MountPoint, innerHandler http.Handler) *chi.Mux {
	r := chi.NewRouter()
	r.Route("/api/v1/files", func(r chi.Router) {
		r.With(MountPointGuard(mounts)).Handle("/*", innerHandler)
	})
	return r
}

func TestMountPointIsolation(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Configure test mount points
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "documents", Path: "/home/user/docs", ReadOnly: false},
		{Name: "backup", Path: "/mnt/backup", ReadOnly: true},
	}

	// Create a simple handler that returns 200 if reached
	innerHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	})

	// Generator for HTTP methods
	methodGen := gen.OneConstOf("GET", "POST", "PUT", "DELETE")

	// Generator for paths outside configured mount points
	invalidPathGen := gen.OneConstOf(
		"etc/passwd",
		"root/.ssh",
		"var/log",
		"home/other",
		"mnt/secret",
		"data",
		"private",
		"system",
		"unauthorized",
		"notamount",
		"mediax",     // Similar but not exact match
		"documentsx", // Similar but not exact match
		"backupx",    // Similar but not exact match
	)

	// Property: Paths outside mount points return 403
	properties.Property("paths outside mount points return 403", prop.ForAll(
		func(method string, invalidPath string) bool {
			router := createTestRouter(mounts, innerHandler)

			req := httptest.NewRequest(method, "/api/v1/files/"+invalidPath, nil)
			rec := httptest.NewRecorder()

			router.ServeHTTP(rec, req)

			return rec.Code == http.StatusForbidden
		},
		methodGen,
		invalidPathGen,
	))

	// Generator for valid mount point paths
	validPathGen := gen.OneConstOf(
		"media",
		"media/movies",
		"media/music/album",
		"documents",
		"documents/work",
		"documents/personal/taxes",
		"backup",
		"backup/2024",
		"backup/2024/january",
	)

	// Property: Paths within mount points are allowed (for GET requests)
	properties.Property("paths within mount points are allowed for GET", prop.ForAll(
		func(validPath string) bool {
			router := createTestRouter(mounts, innerHandler)

			req := httptest.NewRequest("GET", "/api/v1/files/"+validPath, nil)
			rec := httptest.NewRecorder()

			router.ServeHTTP(rec, req)

			return rec.Code == http.StatusOK
		},
		validPathGen,
	))

	properties.TestingRun(t)
}

// **Feature: boxbox, Property 11: Read-Only Mount Enforcement**
// **Validates: Requirements 6.4**
//
// Property: For any mount point configured as read-only, write operations
// (POST, PUT, DELETE to files) SHALL return HTTP 403 status code.

func TestReadOnlyMountEnforcement(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Configure test mount points with mixed read-only settings
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "readonly", Path: "/mnt/readonly", ReadOnly: true},
		{Name: "backup", Path: "/mnt/backup", ReadOnly: true},
		{Name: "writable", Path: "/data/writable", ReadOnly: false},
	}

	// Create a simple handler that returns 200 if reached
	innerHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	})

	// Generator for write HTTP methods
	writeMethodGen := gen.OneConstOf("POST", "PUT", "PATCH", "DELETE")

	// Generator for read-only mount paths
	readOnlyPathGen := gen.OneConstOf(
		"readonly",
		"readonly/file.txt",
		"readonly/subdir/file.txt",
		"backup",
		"backup/2024",
		"backup/2024/data.zip",
	)

	// Property: Write operations on read-only mounts return 403
	properties.Property("write operations on read-only mounts return 403", prop.ForAll(
		func(method string, path string) bool {
			router := createTestRouter(mounts, innerHandler)

			req := httptest.NewRequest(method, "/api/v1/files/"+path, nil)
			rec := httptest.NewRecorder()

			router.ServeHTTP(rec, req)

			return rec.Code == http.StatusForbidden
		},
		writeMethodGen,
		readOnlyPathGen,
	))

	// Generator for writable mount paths
	writablePathGen := gen.OneConstOf(
		"media",
		"media/movies",
		"media/music/album",
		"writable",
		"writable/data",
		"writable/uploads/file.txt",
	)

	// Property: Write operations on writable mounts are allowed
	properties.Property("write operations on writable mounts are allowed", prop.ForAll(
		func(method string, path string) bool {
			router := createTestRouter(mounts, innerHandler)

			req := httptest.NewRequest(method, "/api/v1/files/"+path, nil)
			rec := httptest.NewRecorder()

			router.ServeHTTP(rec, req)

			return rec.Code == http.StatusOK
		},
		writeMethodGen,
		writablePathGen,
	))

	// Property: Read operations on read-only mounts are allowed
	properties.Property("read operations on read-only mounts are allowed", prop.ForAll(
		func(path string) bool {
			router := createTestRouter(mounts, innerHandler)

			req := httptest.NewRequest("GET", "/api/v1/files/"+path, nil)
			rec := httptest.NewRecorder()

			router.ServeHTTP(rec, req)

			return rec.Code == http.StatusOK
		},
		readOnlyPathGen,
	))

	properties.TestingRun(t)
}
