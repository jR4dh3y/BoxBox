package handler

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestDesktopGatewayHTTPBoundary(t *testing.T) {
	upstream := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/v1/connect/redeem" {
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(service.ConnectRedemption{ClientID: "client", Credential: "boxbox_client_" + base64.RawURLEncoding.EncodeToString(make([]byte, 32)), ExpiresAt: time.Now().Add(time.Hour)})
			return
		}
		if r.URL.Path == "/api/v1/files/denied" {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		if r.URL.Path == "/api/v1/stream/preview/media/a.html" {
			w.Header().Set("Content-Type", "text/html")
			w.Header().Set("Set-Cookie", "secret=value")
			w.Header().Set("Location", "https://evil.test")
			w.Header().Set("Access-Control-Allow-Origin", "*")
			io.WriteString(w, "<script>alert(1)</script>")
			return
		}
		t.Errorf("unexpected upstream %s", r.URL)
		w.WriteHeader(500)
	}))
	defer upstream.Close()
	servers, err := service.NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), upstream.Client())
	if err != nil {
		t.Fatal(err)
	}
	server, err := servers.Create(context.Background(), "Test", upstream.URL, "ABCD-EFGH", "Desktop")
	if err != nil {
		t.Fatal(err)
	}
	router := chi.NewRouter()
	NewDesktopServersHandler(servers).RegisterRoutes(router)
	prefix := "/servers/" + server.ID
	for _, tc := range []struct {
		method, path, body string
		status             int
		code               string
	}{
		{"GET", prefix + "/api/v1/files/denied", "", 502, "REMOTE_AUTH_REQUIRED"},
		{"GET", prefix + "/api/v1/auth/login", "", 403, ""},
		{"GET", prefix + "/api/v1/files?token=local-secret", "", 403, ""},
		{"GET", prefix + "/api/v1/files?bad=%ZZ", "", 403, ""},
		{"PATCH", prefix, `{"name":"Renamed","url":"https://evil.test"}`, 400, ""},
	} {
		response := httptest.NewRecorder()
		router.ServeHTTP(response, httptest.NewRequest(tc.method, tc.path, strings.NewReader(tc.body)))
		if response.Code != tc.status {
			t.Errorf("%s: %d %s", tc.path, response.Code, response.Body.String())
		}
		if tc.code != "" && !strings.Contains(response.Body.String(), tc.code) {
			t.Errorf("missing code %s", response.Body.String())
		}
	}
	response := httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest("GET", prefix+"/api/v1/stream/preview/media/a.html", nil))
	if response.Code != 200 || response.Header().Get("Content-Security-Policy") != streamSandboxCSP || response.Header().Get("X-Content-Type-Options") != "nosniff" || response.Header().Get("Cache-Control") != "no-store" {
		t.Fatalf("preview security headers: %d %v", response.Code, response.Header())
	}
	for _, name := range []string{"Set-Cookie", "Location", "Access-Control-Allow-Origin"} {
		if response.Header().Get(name) != "" {
			t.Errorf("unsafe response header %s", name)
		}
	}
	response = httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest("GET", "/servers", nil))
	if strings.Contains(response.Body.String(), "boxbox_client_") {
		t.Fatal("credential leak")
	}
}

func TestDesktopGatewayUnavailableBoundary(t *testing.T) {
	router := chi.NewRouter()
	NewDesktopServersHandler(nil).RegisterRoutes(router)
	for _, path := range []string{"/servers", "/servers/id/api/v1/files"} {
		response := httptest.NewRecorder()
		router.ServeHTTP(response, httptest.NewRequest("GET", path, nil))
		if response.Code != 503 || !strings.Contains(response.Header().Get("Content-Type"), "application/json") {
			t.Fatalf("%s: %d %s", path, response.Code, response.Body.String())
		}
	}
}
