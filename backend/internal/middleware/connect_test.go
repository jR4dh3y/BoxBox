package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type connectValidator struct {
	client *service.ConnectClient
	err    error
}

func (v connectValidator) ValidateClientCredential(string) (*service.ConnectClient, error) {
	return v.client, v.err
}

func TestConnectMiddlewareAccessBoundary(t *testing.T) {
	validator := connectValidator{client: &service.ConnectClient{ClientID: "test-client", Username: "alice"}}
	protected := JWTAuth(newTestAuthService("test-secret"), validator)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if authcontext.ClientID(r.Context()) != "test-client" || authcontext.Username(r.Context()) != "alice" || authcontext.IsAccountSession(r.Context()) {
			t.Error("paired identity has incorrect authentication context")
		}
		w.WriteHeader(http.StatusNoContent)
	}))
	for _, test := range []struct {
		name, method, path string
		want               int
	}{
		{"files", "GET", "/api/v1/files", 204},
		{"file upload", "POST", "/api/v1/files/test", 204},
		{"search", "GET", "/api/v1/search/", 204},
		{"jobs", "GET", "/api/v1/jobs/123", 204},
		{"job cancellation", "DELETE", "/api/v1/jobs/123", 204},
		{"stream", "GET", "/api/v1/stream/preview/test", 204},
		{"upload status lookup", "GET", "/api/v1/stream/upload/status/", 204},
		{"upload to status mount", "POST", "/api/v1/stream/upload/status/file.bin", 204},
		{"shares", "POST", "/api/v1/shares/", 204},
		{"upload configuration", "GET", "/api/v1/system/upload", 204},
		{"self revoke", "DELETE", "/api/v1/connect/client", 204},
		{"self inspection", "GET", "/api/v1/connect/client", 403},
		{"file prefix confusion", "GET", "/api/v1/files-other", 403},
		{"path traversal", "GET", "/api/v1/files/../settings", 403},
		{"encoded traversal", "GET", "/api/v1/files/%2e%2e/settings", 403},
		{"double slash", "GET", "/api/v1/files//test", 403},
		{"unknown endpoint", "GET", "/api/v1/future", 403},
		{"settings", "GET", "/api/v1/settings", 403},
		{"drives", "GET", "/api/v1/system/drives", 403},
		{"auth", "POST", "/api/v1/auth/login", 403},
		{"websocket", "GET", "/api/v1/ws", 403},
	} {
		t.Run(test.name, func(t *testing.T) {
			req := httptest.NewRequest(test.method, test.path, nil)
			req.Header.Set("Authorization", "Bearer "+service.ClientCredentialPrefix+"test")
			rec := httptest.NewRecorder()
			protected.ServeHTTP(rec, req)
			if rec.Code != test.want {
				t.Fatalf("status %d, want %d", rec.Code, test.want)
			}
		})
	}
}

func TestConnectMiddlewareRejectsInvalidOrUnsupportedCredentials(t *testing.T) {
	for _, validator := range []ClientCredentialValidator{nil, connectValidator{}, connectValidator{err: service.ErrInvalidToken}} {
		protected := JWTAuth(newTestAuthService("test-secret"), validator)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			t.Error("invalid client reached handler")
		}))
		req := httptest.NewRequest("GET", "/api/v1/files", nil)
		req.Header.Set("Authorization", "Bearer "+service.ClientCredentialPrefix+"test")
		rec := httptest.NewRecorder()
		protected.ServeHTTP(rec, req)
		if rec.Code != http.StatusUnauthorized {
			t.Fatalf("status %d, want 401", rec.Code)
		}
	}
}
