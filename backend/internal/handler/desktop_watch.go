package handler

import (
	"fmt"
	"net/http"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func DesktopWatch(watcher *service.DesktopWatcher) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		watch, err := watcher.Watch(r.URL.Query().Get("path"))
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		defer watch.Close()
		controller := http.NewResponseController(w)
		_ = controller.SetWriteDeadline(time.Time{})
		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Accel-Buffering", "no")
		send := func(event string) bool {
			if _, err := fmt.Fprintf(w, "event: %s\ndata: {}\n\n", event); err != nil {
				return false
			}
			return controller.Flush() == nil
		}
		if !send("ready") {
			return
		}
		tick := time.NewTicker(150 * time.Millisecond)
		defer tick.Stop()
		heartbeat := time.NewTicker(15 * time.Second)
		defer heartbeat.Stop()
		pending := false
		for {
			select {
			case <-r.Context().Done():
				return
			case _, ok := <-watch.Events:
				if !ok {
					return
				}
				pending = true
			case <-tick.C:
				if pending {
					pending = false
					if !send("change") {
						return
					}
				}
			case <-heartbeat.C:
				if !send("heartbeat") {
					return
				}
			case <-watch.Errors:
				_ = send("change")
				return
			}
		}
	}
}
