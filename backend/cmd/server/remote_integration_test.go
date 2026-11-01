package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/handler"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"golang.org/x/crypto/bcrypt"
)

func TestRemoteProductionTLSJourney(t *testing.T) {
	hash, err := bcrypt.GenerateFromPassword([]byte("pairing-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	upstreamRoot := t.TempDir()
	os.WriteFile(filepath.Join(upstreamRoot, "hello.txt"), []byte("Hello remote"), 0600)
	cfg := &model.ServerConfig{Host: "127.0.0.1", DataDir: t.TempDir(), JWTSecret: "integration-test-secret", Users: map[string]string{"alice": string(hash)}, MountPoints: []model.MountPoint{{Name: "files", Path: upstreamRoot}}, MaxUploadMB: 10, ChunkSizeMB: 1}
	upstream, _, jobs, auth, stream, err := initializeServer(cfg, false, false)
	if err != nil {
		t.Fatal(err)
	}
	defer jobs.Stop()
	defer auth.StopCleanup()
	defer stream.StopCleanup()
	tls := httptest.NewTLSServer(upstream.Handler)
	defer tls.Close()
	account := connectLogin(t, upstream.Handler, "alice")
	code := connectEnable(t, upstream.Handler, account)
	servers, err := service.NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), tls.Client())
	if err != nil {
		t.Fatal(err)
	}
	localRoot := t.TempDir()
	transfers := service.NewDesktopTransfers(filesystem.NewOsFS(), []model.MountPoint{{Name: "files", Path: localRoot}}, servers, t.TempDir())
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", func(r chi.Router) {
		handler.NewDesktopServersHandler(servers).RegisterRoutes(r)
		handler.NewDesktopTransfersHandler(transfers).RegisterRoutes(r)
	})
	token := strings.Repeat("c", 64)
	host := "127.0.0.1:12345"
	local := desktopHandler(router, host, token)
	request := func(method, path string, body []byte, headers http.Header, want int) *httptest.ResponseRecorder {
		t.Helper()
		r := httptest.NewRequest(method, "http://"+host+"/api/v1/desktop"+path, bytes.NewReader(body))
		r.AddCookie(&http.Cookie{Name: desktopCookie, Value: token})
		for key, values := range headers {
			for _, value := range values {
				r.Header.Add(key, value)
			}
		}
		w := httptest.NewRecorder()
		local.ServeHTTP(w, r)
		if w.Code != want {
			t.Fatalf("%s %s status%d want%d body%s", method, path, w.Code, want, w.Body.String())
		}
		return w
	}
	body, _ := json.Marshal(map[string]string{"name": "Production TLS", "url": tls.URL, "code": code.Code, "clientName": "Desktop test"})
	saved := request("POST", "/servers", body, nil, 201)
	var remote service.DesktopServer
	if err := json.Unmarshal(saved.Body.Bytes(), &remote); err != nil {
		t.Fatal(err)
	}
	base := "/servers/" + remote.ID + "/api/v1"
	request("GET", base+"/files", nil, nil, 200)
	listed := request("GET", base+"/files/list/files", nil, nil, 200)
	if !strings.Contains(listed.Body.String(), "hello.txt") {
		t.Fatal("remote file absent")
	}
	preview := request("GET", base+"/stream/preview/files/hello.txt", nil, http.Header{"Range": {"bytes=0-4"}}, 206)
	if preview.Body.String() != "Hello" || !strings.Contains(preview.Header().Get("Content-Security-Policy"), "sandbox") {
		t.Fatal("range/sandbox lost")
	}
	request("POST", base+"/files/files", []byte(`{"name":"folder","type":"directory"}`), nil, 201)
	upload := []byte("uploaded through gateway")
	sum := sha256.Sum256(upload)
	h := http.Header{"X-Create-Only": {"true"}, "X-Chunk-Index": {"0"}, "X-Total-Chunks": {"2"}, "X-Chunk-Size": {"12"}, "X-Total-Size": {"24"}, "X-Checksum": {"sha256:" + hex.EncodeToString(sum[:])}}
	h.Set("X-Upload-ID", "journey-upload")
	request("GET", base+"/stream/upload/status/?uploadId=journey-upload", nil, nil, 404)
	request("POST", base+"/stream/upload/files/folder/uploaded.txt", upload[:12], h, 200)
	status := request("GET", base+"/stream/upload/status/?uploadId=journey-upload", nil, nil, 200)
	if !strings.Contains(status.Body.String(), `"receivedChunks":1`) {
		t.Fatal("remote resume status missing received chunk")
	}
	h.Set("X-Chunk-Index", "1")
	request("POST", base+"/stream/upload/files/folder/uploaded.txt", upload[12:], h, 201)
	request("GET", base+"/stream/upload/status/?uploadId=journey-upload", nil, nil, 404)
	request("POST", base+"/shares", []byte(`{"path":"files/hello.txt","permissions":{"view":true,"download":true}}`), nil, 201)
	request("GET", base+"/shares", nil, nil, 200)
	transferBody, _ := json.Marshal(model.JobParams{Type: model.JobTypeCopy, SourcePath: "@remote/" + remote.ID + "/files/folder", DestPath: "files/copied"})
	created := request("POST", "/transfers", transferBody, nil, 202)
	var job model.Job
	json.Unmarshal(created.Body.Bytes(), &job)
	deadline := time.Now().Add(5 * time.Second)
	for {
		polled := request("GET", "/transfers/"+job.ID, nil, nil, 200)
		json.Unmarshal(polled.Body.Bytes(), &job)
		if job.State.IsTerminal() {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("copy timeout")
		}
		time.Sleep(time.Millisecond * 5)
	}
	if job.State != model.JobStateCompleted {
		t.Fatalf("copy failed: %s", job.Error)
	}
	copied, err := os.ReadFile(filepath.Join(localRoot, "copied/uploaded.txt"))
	if err != nil || !bytes.Equal(copied, upload) {
		t.Fatal("cross-source copied bytes differ")
	}
	// Revocation at the real server must not expire the local desktop session.
	clients := connectRequest(t, upstream.Handler, "GET", "/connect/clients", account, "", 200)
	var clientList struct {
		Clients []service.ConnectClient `json:"clients"`
	}
	if err := json.Unmarshal(clients.Body.Bytes(), &clientList); err != nil || len(clientList.Clients) != 1 {
		t.Fatal("client missing")
	}
	connectRequest(t, upstream.Handler, "DELETE", "/connect/clients/"+clientList.Clients[0].ClientID, account, "", 204)
	failed := request("GET", base+"/files", nil, nil, 502)
	if !strings.Contains(failed.Body.String(), "REMOTE_AUTH_REQUIRED") {
		t.Fatal("revocation masked as local auth")
	}
	request("GET", "/servers", nil, nil, 200)
	tls.Close()
	request("DELETE", "/servers/"+remote.ID, nil, nil, 204)
}
