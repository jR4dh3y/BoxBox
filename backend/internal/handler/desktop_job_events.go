package handler

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// All remote jobs share one loopback connection, leaving HTTP/1.1 slots for browsing.
func (h *DesktopServersHandler) Events(w http.ResponseWriter, r *http.Request) {
	available := make(map[string]bool)
	for _, server := range h.servers.List() {
		available[server.ID] = true
	}
	selected := make(map[string]bool)
	for _, id := range r.URL.Query()["server"] {
		if !available[id] {
			HandleServiceError(w, service.ErrDesktopServerNotFound)
			return
		}
		selected[id] = true
	}
	ctx, cancel := context.WithCancel(r.Context())
	defer cancel()
	events := make(chan service.RemoteJobEvent)
	for id := range selected {
		go h.servers.StreamJobs(ctx, id, func(event service.RemoteJobEvent) bool {
			select {
			case events <- event:
				return true
			case <-ctx.Done():
				return false
			}
		})
	}
	controller := http.NewResponseController(w)
	_ = controller.SetWriteDeadline(time.Time{})
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Accel-Buffering", "no")
	if controller.Flush() != nil {
		return
	}
	heartbeat := time.NewTicker(15 * time.Second)
	defer heartbeat.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case event := <-events:
			if !authcontext.Authorized(ctx) {
				return
			}
			data, err := json.Marshal(event)
			if err != nil {
				return
			}
			if _, err := fmt.Fprintf(w, "event: source\ndata: %s\n\n", data); err != nil {
				return
			}
			if controller.Flush() != nil {
				return
			}
		case <-heartbeat.C:
			if !authcontext.Authorized(ctx) {
				return
			}
			if _, err := fmt.Fprint(w, ": heartbeat\n\n"); err != nil {
				return
			}
			if controller.Flush() != nil {
				return
			}
		}
	}
}
