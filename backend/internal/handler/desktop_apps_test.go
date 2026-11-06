package handler

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type desktopAppsTestService struct {
	listPath      string
	openPath      string
	openAppID     string
	defaultPath   string
	defaultAppID  string
	defaultError  error
	terminalPath  string
	listResult    service.DesktopAppsResult
	listError     error
	openWithError error
	terminalError error
}

func (s *desktopAppsTestService) SetDefault(_ context.Context, path, appID string) error {
	s.defaultPath, s.defaultAppID = path, appID
	return s.defaultError
}

func (s *desktopAppsTestService) Icon(_ context.Context, appID string) (service.DesktopAppIcon, error) {
	return service.DesktopAppIcon{PNG: []byte("png:" + appID)}, nil
}

func (s *desktopAppsTestService) Open(_ context.Context, path string) error {
	s.openPath = path
	return nil
}

func (s *desktopAppsTestService) ListApps(_ context.Context, path string) (service.DesktopAppsResult, error) {
	s.listPath = path
	return s.listResult, s.listError
}

func (s *desktopAppsTestService) OpenWith(_ context.Context, path, appID string) error {
	s.openPath = path
	s.openAppID = appID
	return s.openWithError
}

func (s *desktopAppsTestService) OpenTerminal(_ context.Context, path string) error {
	s.terminalPath = path
	return s.terminalError
}

func TestDesktopAppsHandlerRegistersDocumentedRoutes(t *testing.T) {
	serviceFake := &desktopAppsTestService{
		listResult: service.DesktopAppsResult{
			MIMEType:    "text/plain",
			Recommended: []service.DesktopApplication{{ID: "editor.desktop", Name: "Editor", IsDefault: true}},
			Other:       []service.DesktopApplication{{ID: "viewer.desktop", Name: "Viewer"}},
		},
	}
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", NewDesktopAppsHandler(serviceFake).RegisterRoutes)

	list := httptest.NewRecorder()
	router.ServeHTTP(list, httptest.NewRequest(http.MethodGet, "/api/v1/desktop/apps?path=home%2Fnotes.txt", nil))
	if list.Code != http.StatusOK || serviceFake.listPath != "home/notes.txt" {
		t.Fatalf("GET /apps status/path = %d/%q", list.Code, serviceFake.listPath)
	}
	var listBody service.DesktopAppsResult
	if err := json.Unmarshal(list.Body.Bytes(), &listBody); err != nil {
		t.Fatal(err)
	}
	if listBody.MIMEType != "text/plain" || len(listBody.Recommended) != 1 || listBody.Recommended[0].ID != "editor.desktop" ||
		len(listBody.Other) != 1 || listBody.Other[0].ID != "viewer.desktop" {
		t.Fatalf("GET /apps body = %#v", listBody)
	}

	open := httptest.NewRecorder()
	router.ServeHTTP(open, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/open-with", strings.NewReader(`{"path":"home/notes.txt","appId":"editor.desktop"}`)))
	if open.Code != http.StatusOK || serviceFake.openPath != "home/notes.txt" || serviceFake.openAppID != "editor.desktop" {
		t.Fatalf("POST /open-with status/path/app = %d/%q/%q", open.Code, serviceFake.openPath, serviceFake.openAppID)
	}

	terminal := httptest.NewRecorder()
	router.ServeHTTP(terminal, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/terminal", strings.NewReader(`{"path":"home/notes.txt"}`)))
	if terminal.Code != http.StatusOK || serviceFake.terminalPath != "home/notes.txt" {
		t.Fatalf("POST /terminal status/path = %d/%q", terminal.Code, serviceFake.terminalPath)
	}
	setDefault := httptest.NewRecorder()
	router.ServeHTTP(setDefault, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/apps/default", strings.NewReader(`{"path":"home/notes.txt","appId":"viewer.desktop"}`)))
	if setDefault.Code != http.StatusOK || serviceFake.defaultPath != "home/notes.txt" || serviceFake.defaultAppID != "viewer.desktop" {
		t.Fatalf("default response = %d, path/app = %q/%q", setDefault.Code, serviceFake.defaultPath, serviceFake.defaultAppID)
	}
	icon := httptest.NewRecorder()
	router.ServeHTTP(icon, httptest.NewRequest(http.MethodGet, "/api/v1/desktop/apps/icon?appId=viewer.desktop", nil))
	if icon.Code != http.StatusOK || icon.Header().Get("Content-Type") != "image/png" || icon.Body.String() != "png:viewer.desktop" {
		t.Fatalf("icon response = %d %s", icon.Code, icon.Body.String())
	}
}

func TestDesktopDefaultValidationAndFailure(t *testing.T) {
	fake := &desktopAppsTestService{defaultError: service.ErrDesktopDefaultFailed}
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", NewDesktopAppsHandler(fake).RegisterRoutes)
	for _, body := range []string{`{}`, `{"path":"home/a","appId":""}`, `{"path":"home/a","appId":"a.desktop","mimeType":"text/plain"}`} {
		response := httptest.NewRecorder()
		router.ServeHTTP(response, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/apps/default", strings.NewReader(body)))
		if response.Code != http.StatusBadRequest {
			t.Fatalf("accepted %s: %d", body, response.Code)
		}
	}
	response := httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/apps/default", strings.NewReader(`{"path":"home/a","appId":"a.desktop"}`)))
	if response.Code != http.StatusInternalServerError || !strings.Contains(response.Body.String(), "default application") {
		t.Fatalf("response = %d %s", response.Code, response.Body.String())
	}
}

func TestDesktopAppsHandlerReturnsActionableErrorsAndRejectsUnknownFields(t *testing.T) {
	serviceFake := &desktopAppsTestService{openWithError: service.ErrDesktopApplicationUnavailable}
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", NewDesktopAppsHandler(serviceFake).RegisterRoutes)

	open := httptest.NewRecorder()
	router.ServeHTTP(open, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/open-with", strings.NewReader(`{"path":"home/notes.txt","appId":"stale.desktop"}`)))
	if open.Code != http.StatusNotFound || !strings.Contains(open.Body.String(), "Refresh the list") {
		t.Fatalf("stale app response = %d %s", open.Code, open.Body.String())
	}

	invalid := httptest.NewRecorder()
	router.ServeHTTP(invalid, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/terminal", strings.NewReader(`{"path":"home","command":"sh"}`)))
	if invalid.Code != http.StatusBadRequest {
		t.Fatalf("unknown terminal field status = %d, want 400", invalid.Code)
	}

	serviceFake.listError = errors.New("unexpected")
	list := httptest.NewRecorder()
	router.ServeHTTP(list, httptest.NewRequest(http.MethodGet, "/api/v1/desktop/apps?path=home%2Fnotes.txt", nil))
	if list.Code != http.StatusInternalServerError {
		t.Fatalf("unexpected service error status = %d, want 500", list.Code)
	}
}

func TestDesktopTerminalLaunchErrorNamesRecovery(t *testing.T) {
	serviceFake := &desktopAppsTestService{terminalError: service.ErrDesktopTerminalLaunchFailed}
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", NewDesktopAppsHandler(serviceFake).RegisterRoutes)

	response := httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest(http.MethodPost, "/api/v1/desktop/terminal", strings.NewReader(`{"path":"home/work"}`)))
	if response.Code != http.StatusInternalServerError || !strings.Contains(response.Body.String(), "open a terminal") {
		t.Fatalf("terminal launch response = %d %s", response.Code, response.Body.String())
	}
}
