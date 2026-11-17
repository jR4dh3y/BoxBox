package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func okHandler(w http.ResponseWriter, _ *http.Request) {
	w.WriteHeader(http.StatusOK)
}

func TestSecurityHeaders(t *testing.T) {
	recorder := httptest.NewRecorder()
	SecurityHeaders(http.HandlerFunc(okHandler)).ServeHTTP(
		recorder,
		httptest.NewRequest(http.MethodGet, "/api/v1/files/media", nil),
	)

	for header, want := range map[string]string{
		"X-Content-Type-Options":  "nosniff",
		"Content-Security-Policy": "object-src 'none'; base-uri 'self'; frame-ancestors 'self'; form-action 'self'",
		"Referrer-Policy":         "strict-origin-when-cross-origin",
	} {
		if got := recorder.Header().Get(header); got != want {
			t.Errorf("%s = %q, want %q", header, got, want)
		}
	}
	for _, header := range []string{"X-Frame-Options", "X-XSS-Protection"} {
		if got := recorder.Header().Get(header); got != "" {
			t.Errorf("%s = %q, want it omitted", header, got)
		}
	}
}

func createTestRouter(mounts []model.MountPoint) *chi.Mux {
	r := chi.NewRouter()
	r.Route("/api/v1/files", func(r chi.Router) {
		r.With(MountPointGuard(mounts)).Handle("/*", http.HandlerFunc(okHandler))
	})
	return r
}

func TestMountPointIsolation(t *testing.T) {
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "documents", Path: "/home/user/docs"},
		{Name: "backup", Path: "/mnt/backup", ReadOnly: true},
	}
	for _, test := range []struct {
		name string
		path string
		want int
	}{
		{name: "unknown mount", path: "etc/passwd", want: http.StatusForbidden},
		{name: "mount prefix is not enough", path: "mediax/file", want: http.StatusForbidden},
		{name: "nested mount path", path: "media/movies", want: http.StatusOK},
		{name: "root mount path", path: "documents", want: http.StatusOK},
	} {
		t.Run(test.name, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			request := httptest.NewRequest(http.MethodGet, "/api/v1/files/"+test.path, nil)
			createTestRouter(mounts).ServeHTTP(recorder, request)
			if recorder.Code != test.want {
				t.Fatalf("status = %d, want %d", recorder.Code, test.want)
			}
		})
	}
}

func TestReadOnlyMountEnforcement(t *testing.T) {
	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media"},
		{Name: "readonly", Path: "/mnt/readonly", ReadOnly: true},
		{Name: "writable", Path: "/data/writable"},
	}
	for _, test := range []struct {
		name   string
		method string
		path   string
		want   int
	}{
		{name: "read-only write", method: http.MethodPut, path: "readonly/file.txt", want: http.StatusForbidden},
		{name: "read-only delete", method: http.MethodDelete, path: "readonly/file.txt", want: http.StatusForbidden},
		{name: "writable write", method: http.MethodPut, path: "writable/file.txt", want: http.StatusOK},
		{name: "read-only read", method: http.MethodGet, path: "readonly/file.txt", want: http.StatusOK},
	} {
		t.Run(test.name, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			request := httptest.NewRequest(test.method, "/api/v1/files/"+test.path, nil)
			createTestRouter(mounts).ServeHTTP(recorder, request)
			if recorder.Code != test.want {
				t.Fatalf("status = %d, want %d", recorder.Code, test.want)
			}
		})
	}
}
