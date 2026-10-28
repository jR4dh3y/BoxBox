package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
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
	mount := t.TempDir()
	if err := os.WriteFile(filepath.Join(mount, "example.txt"), []byte("paired file"), 0o600); err != nil {
		t.Fatal(err)
	}
	return connectRouterWithMounts(t, dev, []model.MountPoint{{Name: "test", Path: mount, ReadOnly: true}})
}

func connectRouterWithMounts(t *testing.T, dev bool, mountPoints []model.MountPoint) http.Handler {
	t.Helper()
	hash, err := bcrypt.GenerateFromPassword([]byte("pairing-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	cfg := &model.ServerConfig{
		Host: "127.0.0.1", DataDir: t.TempDir(), JWTSecret: "connect-api-test-secret",
		Users:       map[string]string{"alice": string(hash), "bob": string(hash)},
		MountPoints: mountPoints,
		MaxUploadMB: 10, ChunkSizeMB: 5,
	}
	server, _, _, _, _, err := initializeServer(cfg, dev, false)
	if err != nil {
		t.Fatal(err)
	}
	return server.Handler
}

func TestPairedClientOperationsOnReservedMountNames(t *testing.T) {
	listRoot, statusRoot := t.TempDir(), t.TempDir()
	router := connectRouterWithMounts(t, false, []model.MountPoint{
		{Name: "list", Path: listRoot},
		{Name: "status", Path: statusRoot},
	})
	account := connectLogin(t, router, "alice")
	code := connectEnable(t, router, account)
	redeemed := connectRedeem(t, router, code, "Desktop", http.StatusOK)
	var client service.ConnectRedemption
	if err := json.Unmarshal(redeemed.Body.Bytes(), &client); err != nil || client.Credential == "" {
		t.Fatal("missing paired credential")
	}

	connectRequest(t, router, "POST", "/files/list", client.Credential, `{"type":"file","name":"created.txt","content":"before"}`, http.StatusCreated)
	connectRequest(t, router, "PATCH", "/files/list/created.txt", client.Credential, `{"content":"after"}`, http.StatusOK)
	connectRequest(t, router, "PUT", "/files/list/created.txt", client.Credential, `{"newPath":"list/renamed.txt"}`, http.StatusOK)
	if content, err := os.ReadFile(filepath.Join(listRoot, "renamed.txt")); err != nil || string(content) != "after" {
		t.Fatal("paired file edit or rename did not reach the list mount")
	}
	connectRequest(t, router, "DELETE", "/files/list/renamed.txt", client.Credential, "", http.StatusOK)

	capabilities := connectRequest(t, router, "GET", "/system/upload", client.Credential, "", http.StatusOK)
	var uploadConfig struct {
		CreateOnly bool `json:"createOnly"`
	}
	if json.Unmarshal(capabilities.Body.Bytes(), &uploadConfig) != nil || !uploadConfig.CreateOnly {
		t.Fatal("paired client cannot discover create-only upload support")
	}

	content := []byte("paired upload")
	checksum := sha256.Sum256(content)
	req := httptest.NewRequest(http.MethodPost, "http://boxbox.test/api/v1/stream/upload/status/uploaded.txt", bytes.NewReader(content))
	req.Header.Set("Authorization", "Bearer "+client.Credential)
	req.Header.Set("Content-Type", "application/octet-stream")
	req.Header.Set("X-Upload-ID", "reserved-status")
	req.Header.Set("X-Chunk-Index", "0")
	req.Header.Set("X-Total-Chunks", "1")
	req.Header.Set("X-Chunk-Size", "13")
	req.Header.Set("X-Total-Size", "13")
	req.Header.Set("X-Checksum", hex.EncodeToString(checksum[:]))
	req.Header.Set("X-Create-Only", "true")
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusCreated {
		t.Fatalf("paired upload status %d, want %d", rec.Code, http.StatusCreated)
	}
	if uploaded, err := os.ReadFile(filepath.Join(statusRoot, "uploaded.txt")); err != nil || !bytes.Equal(uploaded, content) {
		t.Fatal("paired one-chunk upload did not reach the status mount")
	}
	// A second upload must preserve the previously published destination.
	repeated := req.Clone(req.Context())
	repeated.Body = io.NopCloser(bytes.NewReader(content))
	repeated.Header.Set("X-Upload-ID", "reserved-status-conflict")
	conflict := httptest.NewRecorder()
	router.ServeHTTP(conflict, repeated)
	if conflict.Code != http.StatusConflict {
		t.Fatalf("create-only paired overwrite: %d", conflict.Code)
	}
	if uploaded, err := os.ReadFile(filepath.Join(statusRoot, "uploaded.txt")); err != nil || !bytes.Equal(uploaded, content) {
		t.Fatal("create-only paired upload changed existing destination")
	}
}

func connectRequest(t *testing.T, router http.Handler, method, path, token, body string, want int, peer ...string) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(method, "http://boxbox.test/api/v1"+path, strings.NewReader(body))
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	if len(peer) > 0 {
		req.RemoteAddr = peer[0]
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

func connectRedeem(t *testing.T, router http.Handler, code service.PairingCode, name string, want int, peer ...string) *httptest.ResponseRecorder {
	t.Helper()
	body, err := json.Marshal(struct {
		Code string `json:"code"`
		Name string `json:"clientName"`
	}{code.Code, name})
	if err != nil {
		t.Fatal(err)
	}
	return connectRequest(t, router, "POST", "/connect/redeem", "", string(body), want, peer...)
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
	connectRedeem(t, router, code, "", 400, "192.0.2.2:1234")
	rec = connectRedeem(t, router, code, "Laptop", 200, "192.0.2.3:1234")
	var client service.ConnectRedemption
	if err := json.Unmarshal(rec.Body.Bytes(), &client); err != nil || client.Username != "alice" || client.ClientID == "" || client.Credential == "" {
		t.Fatal("invalid redemption response")
	}
	connectRedeem(t, router, code, "Replay", 404, "192.0.2.4:1234")
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
	rec = connectRedeem(t, router, code, "Replacement", 200, "192.0.2.5:1234")
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
	for index := range 5 {
		connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 404, fmt.Sprintf("192.0.2.%d:1234", index+1))
	}
	connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 429, "192.0.2.6:1234")
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

func TestConnectRedemptionPerIPLimitPreservesOtherClients(t *testing.T) {
	router := connectRouterFixture(t, false)
	account := connectLogin(t, router, "alice")
	code := connectEnable(t, router, account)
	connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 404)
	for range 8 {
		rec := connectRequest(t, router, "POST", "/connect/redeem", "", `{"code":"wrong","clientName":"Laptop"}`, 429)
		if rec.Header().Get("Retry-After") != "2" {
			t.Fatal("per-IP rejection did not advertise its refill interval")
		}
	}
	connectRedeem(t, router, code, "Other device", 200, "192.0.2.2:1234")
	connectRequest(t, router, "GET", "/connect/clients", account, "", 200)
}
