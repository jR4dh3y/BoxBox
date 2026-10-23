package main

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"golang.org/x/crypto/bcrypt"
)

func connectRouterFixture(t *testing.T, dev bool) http.Handler {
	t.Helper()
	hash, err := bcrypt.GenerateFromPassword([]byte("pairing-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	mount := t.TempDir()
	if err := os.WriteFile(filepath.Join(mount, "example.txt"), []byte("paired file"), 0o600); err != nil {
		t.Fatal(err)
	}
	cfg := &model.ServerConfig{
		Host: "127.0.0.1", DataDir: t.TempDir(), JWTSecret: "connect-api-test-secret",
		Users:       map[string]string{"alice": string(hash), "bob": string(hash)},
		MountPoints: []model.MountPoint{{Name: "test", Path: mount, ReadOnly: true}},
		MaxUploadMB: 10, ChunkSizeMB: 5,
	}
	server, _, _, _, _, err := initializeServer(cfg, dev, false)
	if err != nil {
		t.Fatal(err)
	}
	return server.Handler
}

func connectRequest(t *testing.T, router http.Handler, method, path, token, body string, want int) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(method, "http://boxbox.test/api/v1"+path, strings.NewReader(body))
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != want {
		// Query strings and response bodies can contain credentials.
		t.Fatalf("%s %s: status %d, want %d", method, req.URL.Path, rec.Code, want)
	}
	if strings.HasPrefix(path, "/connect/") && rec.Header().Get("Cache-Control") != "no-store" {
		t.Fatal("connect response is cacheable")
	}
	return rec
}

func connectLogin(t *testing.T, router http.Handler, username string) string {
	t.Helper()
	body, err := json.Marshal(struct {
		Username string `json:"username"`
		Password string `json:"password"`
	}{username, "pairing-password"})
	if err != nil {
		t.Fatal(err)
	}
	rec := connectRequest(t, router, "POST", "/auth/login", "", string(body), http.StatusOK)
	var pair service.TokenPair
	if err := json.Unmarshal(rec.Body.Bytes(), &pair); err != nil || pair.AccessToken == "" {
		t.Fatal("missing access token")
	}
	return pair.AccessToken
}

func connectEnable(t *testing.T, router http.Handler, token string) service.PairingCode {
	t.Helper()
	rec := connectRequest(t, router, "POST", "/connect/pairing", token, `{"password":"pairing-password"}`, http.StatusOK)
	var code service.PairingCode
	if err := json.Unmarshal(rec.Body.Bytes(), &code); err != nil || code.Code == "" || code.ExpiresAt.IsZero() {
		t.Fatal("missing pairing code or expiration")
	}
	return code
}

func connectRedeem(t *testing.T, router http.Handler, code service.PairingCode, name string, want int) *httptest.ResponseRecorder {
	t.Helper()
	body, err := json.Marshal(struct {
		Code string `json:"code"`
		Name string `json:"clientName"`
	}{code.Code, name})
	if err != nil {
		t.Fatal(err)
	}
	return connectRequest(t, router, "POST", "/connect/redeem", "", string(body), want)
}

func TestConnectProductionRouterPairingAndRestrictions(t *testing.T) {
	router := connectRouterFixture(t, false)
	alice, bob := connectLogin(t, router, "alice"), connectLogin(t, router, "bob")
	connectRequest(t, router, "GET", "/connect/pairing", "", "", 401)
	for _, body := range []string{`{`, `{}`, `{"password":"pairing-password","extra":true}`, `{"password":"pairing-password"} {}`} {
		connectRequest(t, router, "POST", "/connect/pairing", alice, body, 400)
	}
	connectRequest(t, router, "POST", "/connect/pairing", alice, `{"password":"wrong"}`, 401)
	req := httptest.NewRequest("POST", "http://boxbox.test/api/v1/connect/pairing", strings.NewReader(`{"password":"pairing-password"}`))
	req.Header.Set("Authorization", "Bearer "+alice)
	req.Header.Set("Origin", "https://other.test")
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != 403 {
		t.Fatal("cross-origin pairing enabled")
	}
	code := connectEnable(t, router, alice)
	status := connectRequest(t, router, "GET", "/connect/pairing", alice, "", 200)
	if strings.Contains(status.Body.String(), "code") || strings.Contains(status.Body.String(), code.Code) {
		t.Fatal("status discloses pairing code")
	}
	connectRequest(t, router, "POST", "/connect/pairing", bob, `{"password":"pairing-password"}`, 409)
	connectRequest(t, router, "DELETE", "/connect/pairing", bob, "", 204)
	status = connectRequest(t, router, "GET", "/connect/pairing", bob, "", 200)
	var hidden service.PairingStatus
	if err := json.Unmarshal(status.Body.Bytes(), &hidden); err != nil || hidden.Enabled || hidden.ExpiresAt != nil {
		t.Fatal("other account's window disclosed")
	}
	connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 404)
	connectRedeem(t, router, code, "", 400)
	rec = connectRedeem(t, router, code, "Laptop", 200)
	var client service.ConnectRedemption
	if err := json.Unmarshal(rec.Body.Bytes(), &client); err != nil || client.Username != "alice" || client.ClientID == "" || client.Credential == "" {
		t.Fatal("invalid redemption response")
	}
	connectRedeem(t, router, code, "Replay", 404)
	connectRequest(t, router, "GET", "/files/test", client.Credential, "", 200)
	connectRequest(t, router, "DELETE", "/files/test/example.txt", client.Credential, "", 403)
	connectRequest(t, router, "GET", "/system/upload", client.Credential, "", 200)
	connectRequest(t, router, "GET", "/search/?path=test&q=example", client.Credential, "", 200)
	for _, target := range []struct{ method, path string }{
		{"GET", "/settings/"}, {"PUT", "/settings/"}, {"GET", "/system/drives"},
		{"POST", "/system/upload"}, {"GET", "/connect/pairing"},
		{"POST", "/connect/pairing"}, {"DELETE", "/connect/pairing"},
		{"GET", "/connect/clients"}, {"DELETE", "/connect/clients/other"},
		{"POST", "/auth/login"}, {"POST", "/auth/refresh"}, {"POST", "/auth/logout"}, {"GET", "/ws"},
	} {
		connectRequest(t, router, target.method, target.path, client.Credential, "", 403)
	}
	connectRequest(t, router, "GET", "/stream/preview/test/example.txt?token="+client.Credential, "", "", 401)
	connectRequest(t, router, "GET", "/files/test?token="+client.Credential, "", "", 401)
	connectRequest(t, router, "GET", "/ws?token="+client.Credential, "", "", 403)
	connectRequest(t, router, "GET", "/settings/", alice, "", 200)
	list := connectRequest(t, router, "GET", "/connect/clients", alice, "", 200)
	var clients struct {
		Clients []service.ConnectClient `json:"clients"`
	}
	if err := json.Unmarshal(list.Body.Bytes(), &clients); err != nil || len(clients.Clients) != 1 || clients.Clients[0].ClientID != client.ClientID {
		t.Fatal("owned client missing")
	}
	if strings.Contains(list.Body.String(), "credential") {
		t.Fatal("client list discloses credential material")
	}
	connectRequest(t, router, "DELETE", "/connect/clients/"+client.ClientID, bob, "", 404)
	connectRequest(t, router, "DELETE", "/connect/client", alice, "", 403)
	connectRequest(t, router, "DELETE", "/connect/client", client.Credential, "", 204)
	connectRequest(t, router, "GET", "/files/test", client.Credential, "", 401)
	code = connectEnable(t, router, alice)
	rec = connectRedeem(t, router, code, "Replacement", 200)
	if err := json.Unmarshal(rec.Body.Bytes(), &client); err != nil {
		t.Fatal("decode replacement client")
	}
	connectRequest(t, router, "DELETE", "/connect/clients/"+client.ClientID, alice, "", 204)
	connectRequest(t, router, "GET", "/files/test", client.Credential, "", 401)
	connectEnable(t, router, alice)
	connectRequest(t, router, "DELETE", "/connect/pairing", alice, "", 204)
}

func TestConnectProductionRouterRedemptionRateLimit(t *testing.T) {
	router := connectRouterFixture(t, false)
	for range 5 {
		connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 404)
	}
	connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 429)
}

func TestConnectDevelopmentIdentityCannotPair(t *testing.T) {
	router := connectRouterFixture(t, true)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest("POST", "/api/v1/connect/pairing", strings.NewReader(`{"password":"pairing-password"}`)))
	if rec.Code == 200 {
		t.Fatal("development identity enabled pairing")
	}
}

func TestConnectDevelopmentModeRejectsPairedCredentials(t *testing.T) {
	router := connectRouterFixture(t, true)
	for _, target := range []string{"/settings/", "/files/test", "/system/upload", "/ws"} {
		connectRequest(t, router, "GET", target, service.ClientCredentialPrefix+"unsupported", "", http.StatusForbidden)
	}
}
