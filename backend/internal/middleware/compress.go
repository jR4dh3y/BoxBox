package middleware

import (
	"bytes"
	"compress/gzip"
	"net/http"
	"strconv"
	"strings"
)

// minCompressBytes is the smallest body worth gzipping. Below it the gzip header makes the body larger.
const minCompressBytes = 1024

// JSONCompression gzips a JSON response that is at least minCompressBytes long, for clients that accept gzip.
// It buffers the body, so use it on bounded listings and not on streams or downloads.
func JSONCompression(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Add("Vary", "Accept-Encoding")
		if !acceptsGzip(r.Header.Get("Accept-Encoding")) {
			next.ServeHTTP(w, r)
			return
		}

		buffered := &bufferedResponse{ResponseWriter: w, status: http.StatusOK}
		next.ServeHTTP(buffered, r)

		header := w.Header()
		body := buffered.body.Bytes()
		isJSON := strings.HasPrefix(header.Get("Content-Type"), "application/json")
		if len(body) >= minCompressBytes && isJSON && header.Get("Content-Encoding") == "" {
			var compressed bytes.Buffer
			zipper, _ := gzip.NewWriterLevel(&compressed, 5)
			zipper.Write(body)
			zipper.Close()
			body = compressed.Bytes()
			header.Set("Content-Encoding", "gzip")
		}
		header.Set("Content-Length", strconv.Itoa(len(body)))
		w.WriteHeader(buffered.status)
		w.Write(body)
	})
}

// acceptsGzip reports whether the Accept-Encoding header allows gzip. A quality of zero in any
// spelling (q=0, q=0.0, q=0.000) refuses it, and so does a quality that is not a number.
func acceptsGzip(acceptEncoding string) bool {
	for _, part := range strings.Split(acceptEncoding, ",") {
		coding, params, hasParams := strings.Cut(strings.TrimSpace(part), ";")
		if !strings.EqualFold(strings.TrimSpace(coding), "gzip") {
			continue
		}
		if !hasParams {
			return true
		}
		name, value, _ := strings.Cut(strings.TrimSpace(params), "=")
		if !strings.EqualFold(strings.TrimSpace(name), "q") {
			return true
		}
		quality, err := strconv.ParseFloat(strings.TrimSpace(value), 64)
		return err == nil && quality > 0
	}
	return false
}

type bufferedResponse struct {
	http.ResponseWriter
	body   bytes.Buffer
	status int
}

func (b *bufferedResponse) WriteHeader(status int) { b.status = status }

func (b *bufferedResponse) Write(p []byte) (int, error) { return b.body.Write(p) }
