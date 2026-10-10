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

func acceptsGzip(acceptEncoding string) bool {
	for _, part := range strings.Split(acceptEncoding, ",") {
		coding, params, _ := strings.Cut(part, ";")
		if strings.EqualFold(strings.TrimSpace(coding), "gzip") {
			return gzipQuality(params) > 0
		}
	}
	return false
}

// gzipQuality reads the q parameter of an Accept-Encoding entry. No q means 1; an unreadable q counts as a refusal.
func gzipQuality(params string) float64 {
	name, value, found := strings.Cut(strings.TrimSpace(params), "=")
	if !found || !strings.EqualFold(strings.TrimSpace(name), "q") {
		return 1
	}
	q, err := strconv.ParseFloat(strings.TrimSpace(value), 64)
	if err != nil {
		return 0
	}
	return q
}

type bufferedResponse struct {
	http.ResponseWriter
	body   bytes.Buffer
	status int
}

func (b *bufferedResponse) WriteHeader(status int) { b.status = status }

func (b *bufferedResponse) Write(p []byte) (int, error) { return b.body.Write(p) }
