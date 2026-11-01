package service

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestRemoteJobStreamRecoversAndDiscoversLegacyOnce(t *testing.T) {
	for _, legacy := range []bool{false, true} {
		t.Run(fmt.Sprint("legacy=", legacy), func(t *testing.T) {
			var connects, lists atomic.Int32
			upstream := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path == "/api/v1/connect/redeem" {
					w.Header().Set("Content-Type", "application/json")
					json.NewEncoder(w).Encode(ConnectRedemption{ClientID: "client", Credential: "boxbox_client_" + strings.Repeat("A", 43), ExpiresAt: time.Now().Add(time.Hour)})
					return
				}
				if r.URL.Path == "/api/v1/jobs" {
					lists.Add(1)
					w.Header().Set("Content-Type", "application/json")
					fmt.Fprint(w, `{"jobs":[]}`)
					return
				}
				attempt := connects.Add(1)
				if legacy {
					http.NotFound(w, r)
					return
				}
				w.Header().Set("Content-Type", "text/event-stream")
				fmt.Fprint(w, "event: snapshot\ndata: {\"jobs\":[]}\n\n")
				http.NewResponseController(w).Flush()
				if attempt == 1 {
					return
				}
				fmt.Fprint(w, "event: job\ndata: {\"id\":\"done\"}\n\n")
				http.NewResponseController(w).Flush()
				<-r.Context().Done()
			}))
			defer upstream.Close()
			servers, err := NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), upstream.Client())
			if err != nil {
				t.Fatal(err)
			}
			saved, err := servers.Create(context.Background(), "Fixture", upstream.URL, "ABCD-EFGH", "Tests", "server")
			if err != nil {
				t.Fatal(err)
			}
			ctx, cancel := context.WithTimeout(context.Background(), 4*time.Second)
			defer cancel()
			events := make(chan RemoteJobEvent, 8)
			done := make(chan struct{})
			go func() {
				defer close(done)
				servers.StreamJobs(ctx, saved.ID, func(event RemoteJobEvent) bool {
					select {
					case events <- event:
						return true
					case <-ctx.Done():
						return false
					}
				})
			}()
			for {
				select {
				case event := <-events:
					if event.ServerID != saved.ID {
						t.Fatal("wrong source")
					}
					if legacy && event.Kind == "unavailable" {
						if lists.Load() != 1 || connects.Load() != 1 {
							t.Fatal("legacy source repeatedly polled")
						}
						cancel()
						<-done
						return
					}
					if !legacy && event.Kind == "job" {
						if connects.Load() != 2 || lists.Load() != 0 {
							t.Fatal("reconnect did not use snapshot stream")
						}
						cancel()
						<-done
						return
					}
				case <-ctx.Done():
					t.Fatal("source recovery timed out")
				}
			}
		})
	}
}
