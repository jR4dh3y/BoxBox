package static

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"testing/fstest"
)

func TestCacheHeaders(t *testing.T) {
	handler := &Handler{
		fsys: fstest.MapFS{
			"index.html":                      {Data: []byte("<html></html>")},
			"early-fetch.js":                  {Data: []byte("(() => {})();")},
			"early-fetch.js.gz":               {Data: []byte("gz")},
			"robots.txt":                      {Data: []byte("User-agent: *")},
			"_app/immutable/chunks/abc123.js": {Data: []byte("export {};")},
		},
		indexHTML: []byte("<html></html>"),
	}

	tests := []struct {
		name, path, acceptEncoding, want string
	}{
		{"page", "/", "", "no-cache"},
		{"early script", "/early-fetch.js", "", "no-cache"},
		{"early script, compressed", "/early-fetch.js", "gzip", "no-cache"},
		{"hashed chunk", "/_app/immutable/chunks/abc123.js", "", "public, max-age=31536000, immutable"},
		{"other static file", "/robots.txt", "", "public, max-age=300"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, tc.path, nil)
			if tc.acceptEncoding != "" {
				req.Header.Set("Accept-Encoding", tc.acceptEncoding)
			}
			rec := httptest.NewRecorder()
			handler.ServeHTTP(rec, req)
			if rec.Code != http.StatusOK {
				t.Fatalf("status %d", rec.Code)
			}
			if got := rec.Header().Get("Cache-Control"); got != tc.want {
				t.Fatalf("Cache-Control = %q, want %q", got, tc.want)
			}
		})
	}
}
