package handler

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestRemoteJobMultiplexerLeavesHTTPConnectionsAvailable(t *testing.T) {
	var closed atomic.Int32
	// All fixtures share one TLS transport without trusting arbitrary certificates.
	var upstreams []*httptest.Server
	for range 4 {
		upstream := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path == "/api/v1/connect/redeem" {
				w.Header().Set("Content-Type", "application/json")
				json.NewEncoder(w).Encode(service.ConnectRedemption{ClientID: "test", Credential: "boxbox_client_" + strings.Repeat("A", 43), ExpiresAt: time.Now().Add(time.Hour)})
				return
			}
			if r.URL.Path != "/api/v1/jobs/events" || r.Header.Get("Authorization") == "" || r.Header.Get("Cookie") != "" {
				t.Error("upstream authority or route")
			}
			w.Header().Set("Content-Type", "text/event-stream")
			fmt.Fprint(w, "event: snapshot\ndata: {\"jobs\":[]}\n\n")
			http.NewResponseController(w).Flush()
			<-r.Context().Done()
			closed.Add(1)
		}))
		upstreams = append(upstreams, upstream)
		defer upstream.Close()
	}
	// httptest servers use the same test certificate.
	servers, err := service.NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), upstreams[0].Client())
	if err != nil {
		t.Fatal(err)
	}
	query := url.Values{}
	for _, upstream := range upstreams {
		saved, err := servers.Create(context.Background(), "Fixture", upstream.URL, "ABCD-EFGH", "Tests", "server")
		if err != nil {
			t.Fatal(err)
		}
		query.Add("server", saved.ID)
	}
	router := chi.NewRouter()
	NewDesktopServersHandler(servers).RegisterRoutes(router)
	router.Get("/hold", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/event-stream")
		http.NewResponseController(w).Flush()
		<-r.Context().Done()
	})
	router.Get("/probe", func(w http.ResponseWriter, r *http.Request) { fmt.Fprint(w, "ready") })
	gateway := httptest.NewServer(router)
	defer gateway.Close()
	transport := &http.Transport{MaxConnsPerHost: 6}
	defer transport.CloseIdleConnections()
	client := http.Client{Transport: transport, Timeout: 3 * time.Second}
	for range 4 {
		response, err := client.Get(gateway.URL + "/hold")
		if err != nil {
			t.Fatal(err)
		}
		defer response.Body.Close()
	}
	response, err := client.Get(gateway.URL + "/servers/events?" + query.Encode())
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	reader := bufio.NewScanner(response.Body)
	seen := make(map[string]bool)
	for len(seen) < 4 && reader.Scan() {
		if !strings.HasPrefix(reader.Text(), "data: ") {
			continue
		}
		var event service.RemoteJobEvent
		if err := json.Unmarshal([]byte(strings.TrimPrefix(reader.Text(), "data: ")), &event); err != nil {
			t.Fatal(err)
		}
		if event.Kind != "snapshot" || string(event.Data) != "{\"jobs\":[]}" {
			t.Fatalf("wrong envelope: %+v", event)
		}
		seen[event.ServerID] = true
	}
	if len(seen) != 4 {
		t.Fatalf("missing sources: %v (%v)", seen, reader.Err())
	}
	probe, err := client.Get(gateway.URL + "/probe")
	if err != nil {
		t.Fatal("ordinary request starved:", err)
	}
	data, _ := io.ReadAll(probe.Body)
	probe.Body.Close()
	if string(data) != "ready" {
		t.Fatalf("probe: %s", data)
	}
	response.Body.Close()
	deadline := time.Now().Add(2 * time.Second)
	for closed.Load() != 4 && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if closed.Load() != 4 {
		t.Fatalf("upstreams leaked: %d", closed.Load())
	}
}
