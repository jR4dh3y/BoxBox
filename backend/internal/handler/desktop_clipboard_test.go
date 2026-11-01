package handler

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type fakeDesktopClipboard struct {
	contents  *service.DesktopClipboardContents
	operation service.DesktopClipboardOperation
	paths     []string
	cleared   []string
	err       error
}

func (f *fakeDesktopClipboard) Get(context.Context) (*service.DesktopClipboardContents, error) {
	return f.contents, f.err
}

func (f *fakeDesktopClipboard) Set(_ context.Context, operation service.DesktopClipboardOperation, paths []string) error {
	f.operation = operation
	f.paths = append([]string(nil), paths...)
	return f.err
}

func (f *fakeDesktopClipboard) ClearCut(_ context.Context, paths []string) error {
	f.cleared = append([]string(nil), paths...)
	return f.err
}

func desktopClipboardRouter(clipboard service.DesktopClipboard) http.Handler {
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", func(r chi.Router) {
		NewDesktopClipboardHandler(clipboard).RegisterRoutes(r)
	})
	return router
}

func TestDesktopClipboardHandlerReturnsTypedClipboardOrNull(t *testing.T) {
	clipboard := &fakeDesktopClipboard{contents: &service.DesktopClipboardContents{
		Operation: service.DesktopClipboardCut,
		Items:     []service.DesktopClipboardItem{{Path: "home/report.txt", Name: "report.txt", IsDir: false}},
	}}
	response := httptest.NewRecorder()
	desktopClipboardRouter(clipboard).ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/api/v1/desktop/clipboard", nil))
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), `"operation":"cut"`) || !strings.Contains(response.Body.String(), `"path":"home/report.txt"`) {
		t.Fatalf("GET response = %d %s", response.Code, response.Body.String())
	}

	clipboard.contents = nil
	response = httptest.NewRecorder()
	desktopClipboardRouter(clipboard).ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/api/v1/desktop/clipboard", nil))
	if response.Code != http.StatusOK || strings.TrimSpace(response.Body.String()) != "null" {
		t.Fatalf("empty GET response = %d %s", response.Code, response.Body.String())
	}
}

func TestDesktopClipboardHandlerDispatchesSetAndClearRequests(t *testing.T) {
	clipboard := &fakeDesktopClipboard{}
	router := desktopClipboardRouter(clipboard)
	for _, test := range []struct {
		body      string
		operation service.DesktopClipboardOperation
		clear     bool
	}{
		{body: `{"operation":"copy","paths":["home/one.txt"]}`, operation: service.DesktopClipboardCopy},
		{body: `{"operation":"cut","paths":["home/one.txt","home/two.txt"]}`, operation: service.DesktopClipboardCut},
		{body: `{"operation":"clear-cut","paths":["home/one.txt"]}`, clear: true},
	} {
		response := httptest.NewRecorder()
		request := httptest.NewRequest(http.MethodPost, "/api/v1/desktop/clipboard", strings.NewReader(test.body))
		router.ServeHTTP(response, request)
		if response.Code != http.StatusOK {
			t.Fatalf("POST %s = %d %s", test.body, response.Code, response.Body.String())
		}
		if test.clear {
			if len(clipboard.cleared) != 1 || clipboard.cleared[0] != "home/one.txt" {
				t.Fatalf("clear paths = %v", clipboard.cleared)
			}
			continue
		}
		if clipboard.operation != test.operation {
			t.Fatalf("operation = %q, want %q", clipboard.operation, test.operation)
		}
	}
}

func TestDesktopClipboardHandlerValidatesRequestsAndMapsUnavailableHelper(t *testing.T) {
	clipboard := &fakeDesktopClipboard{}
	router := desktopClipboardRouter(clipboard)
	for _, body := range []string{
		`{"operation":"move","paths":["home/file.txt"]}`,
		`{"operation":"copy","paths":["home/file.txt"],"unexpected":true}`,
		`{"operation":"copy","paths":[]}{"operation":"cut"}`,
	} {
		response := httptest.NewRecorder()
		router.ServeHTTP(response, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/clipboard", strings.NewReader(body)))
		if response.Code != http.StatusBadRequest {
			t.Fatalf("invalid body %s = %d", body, response.Code)
		}
	}

	clipboard.err = service.ErrDesktopClipboardUnavailable
	response := httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/api/v1/desktop/clipboard", nil))
	if response.Code != http.StatusServiceUnavailable {
		t.Fatalf("unavailable helper response = %d", response.Code)
	}
	clipboard.err = service.ErrDesktopClipboardChanged
	response = httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/clipboard", strings.NewReader(`{"operation":"clear-cut","paths":["home/file.txt"]}`)))
	if response.Code != http.StatusConflict {
		t.Fatalf("changed clipboard response = %d", response.Code)
	}
	if !errors.Is(clipboard.err, service.ErrDesktopClipboardChanged) {
		t.Fatal("test setup lost clipboard error")
	}
}
