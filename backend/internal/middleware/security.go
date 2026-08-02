package middleware

import (
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// SecurityHeaders adds security headers to all responses
// Implements: Requirements 7.3
func SecurityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Prevent MIME type sniffing
		w.Header().Set("X-Content-Type-Options", "nosniff")

		// SvelteKit emits hash-based script/style directives in the prerendered
		// HTML. These response-wide directives enforce framing and navigation
		// restrictions without weakening those generated hashes.
		w.Header().Set(
			"Content-Security-Policy",
			"object-src 'none'; base-uri 'self'; frame-ancestors 'self'; form-action 'self'",
		)

		// Referrer Policy
		w.Header().Set("Referrer-Policy", "strict-origin-when-cross-origin")

		next.ServeHTTP(w, r)
	})
}

// MountPointGuard creates a middleware that validates paths against configured mount points
// and enforces read-only restrictions
// Implements: Requirements 6.2, 6.3, 6.4
func MountPointGuard(mounts []model.MountPoint) func(next http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Get the path from the URL parameter
			path := chi.URLParam(r, "*")

			// If no path parameter, allow the request (e.g., listing roots)
			if path == "" {
				next.ServeHTTP(w, r)
				return
			}

			// Check if path is within any configured mount point
			allowed := false
			var matchedMount *model.MountPoint

			for i := range mounts {
				mount := &mounts[i]
				// Path should start with mount name
				if strings.HasPrefix(path, mount.Name) {
					// Ensure it's an exact match or followed by a separator
					remainder := strings.TrimPrefix(path, mount.Name)
					if remainder == "" || strings.HasPrefix(remainder, "/") {
						allowed = true
						matchedMount = mount
						break
					}
				}
			}

			if !allowed {
				writeForbiddenError(w, "Access denied: path not within configured mount points")
				return
			}

			// Check read-only restriction for write operations
			if matchedMount != nil && matchedMount.ReadOnly && isWriteMethod(r.Method) {
				writeForbiddenError(w, "Access denied: mount point is read-only")
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// isWriteMethod returns true if the HTTP method is a write operation
func isWriteMethod(method string) bool {
	switch method {
	case http.MethodPost, http.MethodPut, http.MethodPatch, http.MethodDelete:
		return true
	default:
		return false
	}
}

// writeForbiddenError writes a 403 Forbidden error response
func writeForbiddenError(w http.ResponseWriter, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusForbidden)
	w.Write([]byte(`{"error":"` + message + `","code":"FORBIDDEN"}`))
}
