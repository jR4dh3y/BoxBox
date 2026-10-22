package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func TestDesktopHandler(t *testing.T) {
	token := strings.Repeat("a", 64)
	address := "127.0.0.1:12345"
	handler := desktopHandler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}), address, token)
	for _, tc := range []struct {
		name, path, cookie, host, origin string
		status                           int
	}{
		{name: "no cookie", path: "/api/v1/files", status: 401},
		{name: "wrong cookie", path: "/api/v1/files", cookie: "wrong", status: 401},
		{name: "bootstrap rejected", path: "/_desktop?token=wrong", status: 401},
		{name: "bootstrap", path: "/_desktop?token=" + token, status: 303},
		{name: "authenticated", path: "/api/v1/files", cookie: token, status: 204},
		{name: "static protected", path: "/", status: 401},
		{name: "websocket protected", path: "/api/v1/ws", status: 401},
		{name: "websocket authenticated", path: "/api/v1/ws", cookie: token, origin: "http://" + address, status: 204},
		{name: "cross origin", path: "/api/v1/files", cookie: token, origin: "https://evil.example", status: 403},
		{name: "rebound host", path: "/api/v1/files", cookie: token, host: "evil.example:12345", status: 403},
	} {
		t.Run(tc.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "http://"+address+tc.path, nil)
			if tc.host != "" {
				req.Host = tc.host
			}
			if tc.origin != "" {
				req.Header.Set("Origin", tc.origin)
			}
			if tc.cookie != "" {
				req.AddCookie(&http.Cookie{Name: desktopCookie, Value: tc.cookie})
			}
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, req)
			if response.Code != tc.status {
				t.Fatalf("status = %d, want %d", response.Code, tc.status)
			}
			if tc.status == 303 {
				cookies := response.Result().Cookies()
				if len(cookies) != 1 || !cookies[0].HttpOnly || cookies[0].SameSite != http.SameSiteStrictMode || cookies[0].Value != token {
					t.Fatal("bootstrap must set a private, strict session cookie")
				}
				if response.Header().Get("Location") != "/" {
					t.Fatal("bootstrap must remove token from URL")
				}
			}
		})
	}
}

func TestDesktopTokenRequired(t *testing.T) {
	for _, value := range []string{"", "short", strings.Repeat("a", 64)} {
		t.Setenv("BOXBOX_DESKTOP_TOKEN", value)
		_, err := desktopToken()
		if (err == nil) != (len(value) == 64) {
			t.Fatal("desktop token validation mismatch")
		}
	}
}

func TestDesktopBootstrapNavigationStaysLocal(t *testing.T) {
	token := strings.Repeat("a", 64)
	address := "127.0.0.1:12345"
	handler := desktopHandler(http.NotFoundHandler(), address, token)
	for _, tc := range []struct{ next, want string }{
		{"/browse?path=root%2Ftmp&select=root%2Ftmp%2Fexample.txt", "/browse?path=root%2Ftmp&select=root%2Ftmp%2Fexample.txt"},
		{"https://outside.example/browse", "/"},
		{"//outside.example/browse", "/"},
		{"/api/v1/desktop/open", "/"},
	} {
		r := httptest.NewRequest("GET", "http://"+address+"/_desktop?token="+token+"&next="+url.QueryEscape(tc.next), nil)
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, r)
		if w.Code != 303 || w.Header().Get("Location") != tc.want {
			t.Fatalf("next=%q got status %d location %q", tc.next, w.Code, w.Header().Get("Location"))
		}
	}
}

func TestDesktopOpenLaunchesResolvedFile(t *testing.T) {
	dir := t.TempDir()
	file := filepath.Join(dir, "hello world.txt")
	capture := filepath.Join(dir, "opened")
	if err := os.WriteFile(file, []byte("hello"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "xdg-open"), []byte("#!/bin/sh\nprintf '%s' \"$1\" > \"$DESKTOP_OPEN_CAPTURE\"\n"), 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir)
	t.Setenv("DESKTOP_OPEN_CAPTURE", capture)
	cfg := &model.ServerConfig{MountPoints: []model.MountPoint{{Name: "home", Path: dir}}}
	routes := desktopRoutes(http.NotFoundHandler(), cfg)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/desktop/open", strings.NewReader(`{"path":"home/hello world.txt"}`))
	req.Header.Set("Content-Type", "application/json")
	response := httptest.NewRecorder()
	routes.ServeHTTP(response, req)
	if response.Code != http.StatusOK {
		t.Fatalf("status = %d", response.Code)
	}
	opened, err := os.ReadFile(capture)
	if err != nil {
		t.Fatal(err)
	}
	if string(opened) != file {
		t.Fatalf("opened %q, want %q", opened, file)
	}
}
