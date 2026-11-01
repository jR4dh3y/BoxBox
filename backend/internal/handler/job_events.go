package handler

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
)

// Subscribe before listing so a mutation between snapshot and stream cannot be lost.
func jobEvents(w http.ResponseWriter, r *http.Request,
	list func(context.Context) ([]*model.Job, error),
	watch func(context.Context) (<-chan model.Job, func())) {
	updates, stop := watch(r.Context())
	defer stop()
	jobs, err := list(r.Context())
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	if jobs == nil {
		jobs = []*model.Job{}
	}
	controller := http.NewResponseController(w)
	_ = controller.SetWriteDeadline(time.Time{})
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Accel-Buffering", "no")
	send := func(event string, value interface{}) bool {
		if !authcontext.Authorized(r.Context()) {
			return false
		}
		data, err := json.Marshal(value)
		if err != nil {
			return false
		}
		if _, err := fmt.Fprintf(w, "event: %s\ndata: %s\n\n", event, data); err != nil {
			return false
		}
		return controller.Flush() == nil
	}
	if !send("snapshot", JobListResponse{Jobs: jobs}) {
		return
	}
	heartbeat := time.NewTicker(15 * time.Second)
	defer heartbeat.Stop()
	for {
		select {
		case <-r.Context().Done():
			return
		case job, ok := <-updates:
			if !ok || !send("job", job) {
				return
			}
		case <-heartbeat.C:
			if !authcontext.Authorized(r.Context()) {
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

func (h *JobHandler) Events(w http.ResponseWriter, r *http.Request) {
	jobEvents(w, r, h.jobService.List, h.jobService.Watch)
}

func (h *DesktopTransfersHandler) Events(w http.ResponseWriter, r *http.Request) {
	jobEvents(w, r, func(context.Context) ([]*model.Job, error) { return h.transfers.List(), nil }, h.transfers.Watch)
}
