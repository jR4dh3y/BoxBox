package service

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func gatewayFixture(t *testing.T, handler http.HandlerFunc) (*DesktopServers, *DesktopServer, *httptest.Server) {
	t.Helper()
	credential := "boxbox_client_" + base64.RawURLEncoding.EncodeToString(make([]byte, 32))
	upstream := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/box/api/v1/connect/redeem" {
			if r.Header.Get("Authorization") != "" || r.Header.Get("Cookie") != "" {
				t.Error("pairing leaked authorization")
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(ConnectRedemption{ClientID: "client", Credential: credential, ExpiresAt: time.Now().Add(time.Hour), Username: "user"})
			return
		}
		if r.Header.Get("Authorization") != "Bearer "+credential {
			t.Error("missing stored bearer")
		}
		handler(w, r)
	}))
	t.Cleanup(upstream.Close)
	servers, err := NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), upstream.Client())
	if err != nil {
		t.Fatal(err)
	}
	server, err := servers.Create(context.Background(), "Test", upstream.URL+"/box", "ABCD-EFGH", "Desktop")
	if err != nil {
		t.Fatal(err)
	}
	return servers, server, upstream
}

func TestDesktopGatewayTLSForwardingAndPersistence(t *testing.T) {
	var revoked atomic.Bool
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/box/api/v1/connect/client" && r.Method == "DELETE" {
			revoked.Store(true)
			w.WriteHeader(204)
			return
		}
		if r.URL.Path != "/box/api/v1/stream/preview/media/a.txt" {
			t.Errorf("path %s", r.URL.Path)
		}
		for _, name := range []string{"Cookie", "Origin", "Forwarded", "X-Forwarded-For", "X-Desktop-Session"} {
			if r.Header.Get(name) != "" {
				t.Errorf("leaked %s", name)
			}
		}
		if r.Header.Get("Range") != "bytes=1-3" {
			t.Error("range lost")
		}
		w.Header().Set("Content-Type", "text/plain")
		w.Header().Set("Content-Range", "bytes 1-3/5")
		w.WriteHeader(206)
		io.WriteString(w, "bcd")
	})
	headers := http.Header{"Cookie": {"private"}, "Origin": {"https://local"}, "Forwarded": {"secret"}, "X-Forwarded-For": {"secret"}, "X-Desktop-Session": {"secret"}, "Authorization": {"Bearer local-jwt"}, "Range": {"bytes=1-3"}}
	response, err := servers.Forward(context.Background(), server.ID, "GET", "/stream/preview/media/a.txt", nil, headers, nil)
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(response.Body)
	response.Body.Close()
	if response.StatusCode != 206 || string(body) != "bcd" {
		t.Fatalf("%d %s", response.StatusCode, body)
	}
	dir := filepath.Dir(servers.path)
	for path, mode := range map[string]os.FileMode{dir: 0700, servers.path: 0600} {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != mode {
			t.Fatalf("permissions %s: %v %v", path, info, err)
		}
	}
	public, _ := json.Marshal(servers.List())
	if strings.Contains(string(public), "credential") || strings.Contains(string(public), "boxbox_client") || strings.Contains(string(public), "clientId") {
		t.Fatal("public credential leak")
	}
	restored, err := NewDesktopServers(filesystem.NewOsFS(), filepath.Dir(dir), nil)
	if err != nil || len(restored.List()) != 1 {
		t.Fatalf("restore %v", err)
	}
	renamed, err := restored.Rename(server.ID, "Renamed")
	if err != nil || renamed.URL != server.URL || renamed.ID != server.ID {
		t.Fatalf("rename %v", err)
	}
	if err := servers.Delete(context.Background(), server.ID); err != nil || !revoked.Load() || len(servers.List()) != 0 {
		t.Fatalf("delete %v", err)
	}
}

func TestDesktopGatewayRejectsUnexpectedUpstream(t *testing.T) {
	for _, tc := range []struct {
		name    string
		status  int
		content string
		want    error
	}{
		{"unauthorized", 401, "application/json", ErrRemoteAuthRequired}, {"forbidden", 403, "text/html", ErrRemoteAuthRequired}, {"redirect", 302, "application/json", ErrRemoteUnavailable}, {"SPA", 200, "text/html", ErrRemoteUnavailable},
	} {
		t.Run(tc.name, func(t *testing.T) {
			servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
				w.Header().Set("Content-Type", tc.content)
				w.Header().Set("Location", "/elsewhere")
				w.WriteHeader(tc.status)
			})
			_, err := servers.Forward(context.Background(), server.ID, "GET", "/files/media", nil, nil, nil)
			if !errors.Is(err, tc.want) {
				t.Fatalf("got %v", err)
			}
		})
	}
}

func TestDesktopGatewayAllowlist(t *testing.T) {
	for _, tc := range []struct {
		method, route string
		allowed       bool
	}{
		{"GET", "/files", true}, {"GET", "/files/list/media", true}, {"PATCH", "/files/media/a", true}, {"POST", "/stream/upload/media/a", true}, {"GET", "/stream/upload/status/id", true}, {"GET", "/system/upload", true}, {"POST", "/jobs", true}, {"PATCH", "/shares/id", true},
		{"POST", "/auth/login", false}, {"GET", "/settings", false}, {"GET", "/desktop/files", false}, {"GET", "/ws", false}, {"GET", "/system/drives", false}, {"POST", "/system/upload", false}, {"DELETE", "/stream/download/media/a", false}, {"GET", "/files/../auth", false}, {"GET", "/files/%2e%2e/auth", false}, {"GET", "/files/media//a", false}, {"GET", "/files/media\\a", false}, {"POST", "/connect/redeem", false}, {"GET", "/jobs/id/extra", false},
	} {
		if got := DesktopGatewayAllowed(tc.method, tc.route); got != tc.allowed {
			t.Errorf("%s %s = %v", tc.method, tc.route, got)
		}
	}
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) { t.Error("disallowed query reached upstream") })
	for _, key := range []string{"token", "access_token", "url", "redirect", "jwt"} {
		_, err := servers.Forward(context.Background(), server.ID, "GET", "/files", url.Values{key: {"secret"}}, nil, nil)
		if !errors.Is(err, ErrRemoteRoute) {
			t.Errorf("query %s: %v", key, err)
		}
	}
}

func TestDesktopGatewayOfflineDeletionAndVerifiedTLS(t *testing.T) {
	servers, server, upstream := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		io.WriteString(w, "{}")
	})
	verified, err := NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = verified.Create(context.Background(), "Test", upstream.URL, "ABCD-EFGH", "Desktop"); !errors.Is(err, ErrRemoteUnavailable) {
		t.Fatalf("untrusted TLS accepted: %v", err)
	}
	upstream.Close()
	if _, err = servers.Forward(context.Background(), server.ID, "GET", "/files", nil, nil, nil); !errors.Is(err, ErrRemoteUnavailable) {
		t.Fatalf("offline %v", err)
	}
	if err = servers.Delete(context.Background(), server.ID); err != nil || len(servers.List()) != 0 {
		t.Fatalf("offline delete %v", err)
	}
}

func TestDesktopGatewayCancellation(t *testing.T) {
	stopped := make(chan struct{})
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/octet-stream")
		w.WriteHeader(200)
		w.(http.Flusher).Flush()
		<-r.Context().Done()
		close(stopped)
	})
	ctx, cancel := context.WithCancel(context.Background())
	response, err := servers.Forward(ctx, server.ID, "GET", "/stream/download/media/a", nil, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	cancel()
	response.Body.Close()
	select {
	case <-stopped:
	case <-time.After(3 * time.Second):
		t.Fatal("upstream context not canceled")
	}
}

func TestDesktopServerStorageFailsClosed(t *testing.T) {
	for _, snapshot := range []string{`{}`, `{"version":1,"servers":{}} {}`, `{"version":1,"servers":{"invalid":{}}}`} {
		dir := t.TempDir()
		if err := os.Mkdir(filepath.Join(dir, "desktop"), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(dir, "desktop", "servers.json"), []byte(snapshot), 0600); err != nil {
			t.Fatal(err)
		}
		if _, err := NewDesktopServers(filesystem.NewOsFS(), dir, nil); !errors.Is(err, ErrDesktopServerStorage) {
			t.Fatalf("accepted %s: %v", snapshot, err)
		}
	}
	dir := t.TempDir()
	target := filepath.Join(t.TempDir(), "outside")
	if err := os.Mkdir(target, 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(dir, "desktop")); err != nil {
		t.Fatal(err)
	}
	if _, err := NewDesktopServers(filesystem.NewOsFS(), dir, nil); !errors.Is(err, ErrDesktopServerStorage) {
		t.Fatalf("accepted symlink: %v", err)
	}
}

func TestDesktopServerPairReplacesAccessAtSameAuthority(t *testing.T) {
	var redemptions atomic.Int32
	var revokedCredential string
	upstream := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/v1/connect/redeem" {
			random := make([]byte, 32)
			random[0] = byte(redemptions.Add(1))
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(ConnectRedemption{ClientID: "client", Credential: "boxbox_client_" + base64.RawURLEncoding.EncodeToString(random), ExpiresAt: time.Now().Add(time.Hour)})
			return
		}
		if r.URL.Path == "/api/v1/connect/client" {
			revokedCredential = r.Header.Get("Authorization")
			w.WriteHeader(204)
			return
		}
		t.Errorf("unexpected path %s", r.URL.Path)
	}))
	defer upstream.Close()
	servers, err := NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), upstream.Client())
	if err != nil {
		t.Fatal(err)
	}
	old, err := servers.Create(context.Background(), "Original", upstream.URL, "ABCD-EFGH", "Desktop")
	if err != nil {
		t.Fatal(err)
	}
	record, err := servers.record(old.ID)
	if err != nil {
		t.Fatal(err)
	}
	updated, err := servers.Pair(context.Background(), old.ID, "JKLM-NPQR", "Desktop")
	if err != nil {
		t.Fatal(err)
	}
	current, err := servers.record(old.ID)
	if err != nil {
		t.Fatal(err)
	}
	if updated.ID != old.ID || updated.URL != old.URL || updated.Name != old.Name || current.Credential == record.Credential || revokedCredential != "Bearer "+record.Credential {
		t.Fatal("repair did not preserve authority and revoke previous access")
	}
}

func TestDesktopServerRejectsUnsafeAuthorities(t *testing.T) {
	for _, raw := range []string{"http://host", "https://user:pass@host", "https://host?a=b", "https://host#fragment", "https://host/base/../admin", "https://host/base//api", "https://host/base%2fadmin", "https://host/base\\admin", "https://host/base/%252e%252e"} {
		if _, err := desktopServerURL(raw); !errors.Is(err, ErrDesktopServerInput) {
			t.Errorf("accepted %s: %v", raw, err)
		}
	}
	for _, raw := range []string{"https://example.com", "https://example.com/base", "https://[::1]:443/base"} {
		if _, err := desktopServerURL(raw); err != nil {
			t.Errorf("rejected %s: %v", raw, err)
		}
	}
}

func TestDesktopGatewayPreservesEncodedVirtualPaths(t *testing.T) {
	path := "/files/media/café #? 100%.txt"
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/box/api/v1"+path || r.URL.RawQuery != "" || r.URL.Fragment != "" {
			t.Errorf("changed virtual path: %s", r.URL)
		}
		w.Header().Set("Content-Type", "application/json")
		io.WriteString(w, "{}")
	})
	response, err := servers.Forward(context.Background(), server.ID, "GET", path, nil, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	response.Body.Close()
}

func TestDesktopServerPersistenceFailures(t *testing.T) {
	for _, stage := range []authPersistenceFailureStage{authPersistenceFailCreate, authPersistenceFailWrite, authPersistenceFailFileSync, authPersistenceFailFileClose, authPersistenceFailRename, authPersistenceFailDirectoryOpen, authPersistenceFailDirectorySync, authPersistenceFailDirectoryClose} {
		t.Run(string(stage), func(t *testing.T) {
			servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) { t.Error("failed-closed request reached upstream") })
			fsys := &authPersistenceFaultFS{FS: servers.fs, storePath: servers.path, stage: stage}
			servers.fs = fsys
			if result, err := servers.Rename(server.ID, "Changed"); result != nil || !errors.Is(err, ErrDesktopServerStorage) {
				t.Fatalf("rename %v %v", result, err)
			}
			published := stage == authPersistenceFailDirectoryOpen || stage == authPersistenceFailDirectorySync || stage == authPersistenceFailDirectoryClose
			if servers.Available() == published {
				t.Fatal("incorrect availability after persistence failure")
			}
			if published {
				if _, err := servers.Forward(context.Background(), server.ID, "GET", "/files", nil, nil, nil); !errors.Is(err, ErrDesktopServerStorage) {
					t.Fatalf("not failed closed: %v", err)
				}
			} else if servers.List()[0].Name != "Test" {
				t.Fatal("failed mutation was not rolled back")
			}
			fsys.stage = ""
			restarted, err := NewDesktopServers(fsys, filepath.Dir(filepath.Dir(servers.path)), nil)
			if err != nil {
				t.Fatal(err)
			}
			expected := "Test"
			if published {
				expected = "Changed"
			}
			if restarted.List()[0].Name != expected {
				t.Fatal("restart disagreed with publication")
			}
			entries, err := os.ReadDir(filepath.Dir(servers.path))
			if err != nil || len(entries) != 1 || entries[0].Name() != "servers.json" {
				t.Fatalf("temporary snapshots left: %v %v", entries, err)
			}
		})
	}
}

func TestDesktopGatewayThumbnailVersion(t *testing.T) {
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/box/api/v1/stream/thumbnail/media/photo.png" || r.URL.Query().Get("v") != "mtime-size" {
			t.Errorf("thumbnail request lost path or version: %s", r.URL)
		}
		w.Header().Set("Content-Type", "image/png")
		w.Write([]byte("thumbnail"))
	})
	response, err := servers.Forward(context.Background(), server.ID, http.MethodGet, "/stream/thumbnail/media/photo.png", url.Values{"v": {"mtime-size"}}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("thumbnail status: %d", response.StatusCode)
	}
}
