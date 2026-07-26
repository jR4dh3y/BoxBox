// Package static provides embedded static file serving for the SPA frontend.
// Files are embedded at build time via go:embed and served with:
// - SPA fallback (unknown routes serve index.html)
// - Pre-compressed file support (brotli, gzip)
// - Immutable cache headers for hashed assets
package static

import (
	"bytes"
	"embed"
	"io"
	"io/fs"
	"mime"
	"net/http"
	"path/filepath"
	"strings"
)

//go:embed all:dist
var embeddedFiles embed.FS

// Handler serves static files with SPA fallback and compression support
type Handler struct {
	fsys      fs.FS
	indexHTML []byte
}

// NewHandler creates a static file handler from embedded files
func NewHandler(devMode ...bool) (*Handler, error) {
	// Get sub-filesystem starting at "dist"
	fsys, err := fs.Sub(embeddedFiles, "dist")
	if err != nil {
		return nil, err
	}
	// Pre-load index.html for SPA fallback
	indexHTML, err := fs.ReadFile(fsys, "index.html")
	if err != nil {
		// If index.html doesn't exist, create a placeholder
		// This allows the binary to run even without embedded frontend (dev mode)
		indexHTML = []byte(`<!DOCTYPE html><html><head><title>BoxBox</title></head><body><h1>Frontend not embedded</h1><p>Build with frontend assets to enable the web UI.</p></body></html>`)
	}
	if len(devMode) > 0 && devMode[0] {
		indexHTML = bytes.Replace(
			indexHTML,
			[]byte("<head>"),
			[]byte(`<head><meta name="boxbox-dev-mode" content="true">`),
			1,
		)
	}

	return &Handler{
		fsys:      fsys,
		indexHTML: indexHTML,
	}, nil
}

// ServeHTTP implements http.Handler with SPA fallback and compression
func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Only handle GET and HEAD requests
	if r.Method != http.MethodGet && r.Method != http.MethodHead {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	path := strings.TrimPrefix(r.URL.Path, "/")
	if path == "" {
		path = "index.html"
	}

	// Development mode modifies index.html at startup, so serve the in-memory
	// version instead of an unmodified pre-compressed copy.
	if path == "index.html" && bytes.Contains(h.indexHTML, []byte(`name="boxbox-dev-mode"`)) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("Cache-Control", "no-cache")
		if r.Method == http.MethodGet {
			w.Write(h.indexHTML)
		}
		return
	}

	// Check for pre-compressed versions based on Accept-Encoding
	acceptEncoding := r.Header.Get("Accept-Encoding")

	// Try brotli first, then gzip
	if strings.Contains(acceptEncoding, "br") {
		if h.tryServeCompressed(w, r, path, ".br", "br") {
			return
		}
	}
	if strings.Contains(acceptEncoding, "gzip") {
		if h.tryServeCompressed(w, r, path, ".gz", "gzip") {
			return
		}
	}

	// Try to serve uncompressed file
	if h.tryServeFile(w, r, path) {
		return
	}

	// File not found - serve index.html (SPA fallback)
	// This enables client-side routing for paths like /browse/some/folder
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Cache-Control", "no-cache")
	w.Write(h.indexHTML)
}

func (h *Handler) tryServeCompressed(w http.ResponseWriter, r *http.Request, path, ext, encoding string) bool {
	compressedPath := path + ext
	file, err := h.fsys.Open(compressedPath)
	if err != nil {
		return false
	}
	defer file.Close()

	// Get file info for size
	stat, err := file.Stat()
	if err != nil || stat.IsDir() {
		return false
	}

	// Set headers
	w.Header().Set("Content-Encoding", encoding)
	w.Header().Set("Vary", "Accept-Encoding")
	h.setCacheHeaders(w, path)
	h.setContentType(w, path)

	// Serve the file content
	if r.Method == http.MethodHead {
		return true
	}

	if seeker, ok := file.(io.ReadSeeker); ok {
		http.ServeContent(w, r, compressedPath, stat.ModTime(), seeker)
	} else {
		io.Copy(w, file)
	}
	return true
}

func (h *Handler) tryServeFile(w http.ResponseWriter, r *http.Request, path string) bool {
	file, err := h.fsys.Open(path)
	if err != nil {
		return false
	}
	defer file.Close()

	stat, err := file.Stat()
	if err != nil || stat.IsDir() {
		// For directories, try index.html inside them
		if stat != nil && stat.IsDir() {
			indexPath := filepath.Join(path, "index.html")
			return h.tryServeFile(w, r, indexPath)
		}
		return false
	}

	h.setCacheHeaders(w, path)
	h.setContentType(w, path)

	if r.Method == http.MethodHead {
		return true
	}

	if seeker, ok := file.(io.ReadSeeker); ok {
		http.ServeContent(w, r, path, stat.ModTime(), seeker)
	} else {
		io.Copy(w, file)
	}
	return true
}

func (h *Handler) setCacheHeaders(w http.ResponseWriter, path string) {
	// Immutable assets (content-hashed) get long cache
	if strings.HasPrefix(path, "_app/immutable/") {
		w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
	} else if strings.HasPrefix(path, "_app/") {
		// Other _app files get medium cache
		w.Header().Set("Cache-Control", "public, max-age=3600")
	} else if path == "index.html" || path == "" {
		// HTML should not be cached to ensure updates are picked up
		w.Header().Set("Cache-Control", "no-cache")
	} else {
		// Other static files get short cache
		w.Header().Set("Cache-Control", "public, max-age=300")
	}
}

func (h *Handler) setContentType(w http.ResponseWriter, path string) {
	// Strip compression extensions for content type detection
	cleanPath := strings.TrimSuffix(strings.TrimSuffix(path, ".gz"), ".br")
	ext := filepath.Ext(cleanPath)

	// Use mime package for standard types
	contentType := mime.TypeByExtension(ext)
	if contentType != "" {
		w.Header().Set("Content-Type", contentType)
		return
	}

	// Fallback for common types not in mime database
	switch ext {
	case ".js", ".mjs":
		w.Header().Set("Content-Type", "application/javascript; charset=utf-8")
	case ".css":
		w.Header().Set("Content-Type", "text/css; charset=utf-8")
	case ".json":
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
	case ".svg":
		w.Header().Set("Content-Type", "image/svg+xml")
	case ".png":
		w.Header().Set("Content-Type", "image/png")
	case ".jpg", ".jpeg":
		w.Header().Set("Content-Type", "image/jpeg")
	case ".ico":
		w.Header().Set("Content-Type", "image/x-icon")
	case ".woff2":
		w.Header().Set("Content-Type", "font/woff2")
	case ".woff":
		w.Header().Set("Content-Type", "font/woff")
	case ".ttf":
		w.Header().Set("Content-Type", "font/ttf")
	case ".html", ".htm":
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
	case ".txt":
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	case ".xml":
		w.Header().Set("Content-Type", "application/xml; charset=utf-8")
	default:
		w.Header().Set("Content-Type", "application/octet-stream")
	}
}
