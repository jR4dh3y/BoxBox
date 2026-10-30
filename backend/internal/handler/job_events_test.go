package handler

import (
	"bufio"
	"context"
	"encoding/json"
	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestJobEventsFlushSnapshotsUpdatesAndStopAfterRevocation(t *testing.T) {
	var events service.JobEvents
	var allowed atomic.Bool
	allowed.Store(true)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ctx := authcontext.WithUsername(r.Context(), "alice")
		ctx = authcontext.WithAuthorization(ctx, allowed.Load)
		jobEvents(w, r.WithContext(ctx), func(context.Context) ([]*model.Job, error) { return nil, nil }, events.Watch)
	}))
	defer server.Close()
	client := http.Client{Timeout: 3 * time.Second}
	response, err := client.Get(server.URL)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	reader := bufio.NewReader(response.Body)
	frame := func() string {
		var result strings.Builder
		for {
			line, err := reader.ReadString('\n')
			if err != nil {
				t.Fatal(err)
			}
			result.WriteString(line)
			if line == "\n" {
				return result.String()
			}
		}
	}
	if got := frame(); !strings.Contains(got, `"jobs":[]`) {
		t.Fatal(got)
	}
	events.Publish(model.Job{ID: "owned", Owner: "alice", State: model.JobStateRunning, Progress: 42})
	got := frame()
	if !strings.Contains(got, `"progress":42`) || strings.Contains(got, `"Owner"`) {
		t.Fatal(got)
	}
	allowed.Store(false)
	events.Publish(model.Job{ID: "revoked", Owner: "alice"})
	if _, err := reader.ReadByte(); err == nil {
		t.Fatal("revoked stream released more data")
	}
}

func TestJobEventsStreamRealCopyLifecycle(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "source"), make([]byte, 4<<20), 0600); err != nil {
		t.Fatal(err)
	}
	jobs := service.NewJobService(filesystem.NewOsFS(), nil, service.JobServiceConfig{Workers: 1, MountPoints: []model.MountPoint{{Name: "files", Path: root}}})
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	jobs.Start(ctx)
	defer jobs.Stop()
	router := chi.NewRouter()
	router.Use(func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			next.ServeHTTP(w, r.WithContext(authcontext.WithUsername(r.Context(), "alice")))
		})
	})
	router.Route("/jobs", NewJobHandler(jobs).RegisterRoutes)
	server := httptest.NewServer(router)
	defer server.Close()
	client := http.Client{Timeout: 5 * time.Second}
	response, err := client.Get(server.URL + "/jobs/events")
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	reader := bufio.NewReader(response.Body)
	frame := func() string {
		var result strings.Builder
		for {
			line, err := reader.ReadString('\n')
			if err != nil {
				t.Fatal(err)
			}
			result.WriteString(line)
			if line == "\n" {
				return result.String()
			}
		}
	}
	frame()
	created, err := jobs.Create(authcontext.WithUsername(ctx, "alice"), model.JobParams{Type: model.JobTypeCopy, SourcePath: "files/source", DestPath: "files/copied"})
	if err != nil {
		t.Fatal(err)
	}
	intermediate := false
	for {
		event := frame()
		parts := strings.SplitN(event, "data: ", 2)
		if len(parts) != 2 {
			t.Fatal(event)
		}
		var job model.Job
		if err := json.Unmarshal([]byte(strings.TrimSpace(parts[1])), &job); err != nil {
			t.Fatal(err)
		}
		if job.ID != created.ID {
			t.Fatal("wrong job")
		}
		if job.Progress > 0 && job.Progress < 100 {
			intermediate = true
		}
		if job.State.IsTerminal() {
			if job.State != model.JobStateCompleted || !intermediate {
				t.Fatalf("lost progress: %+v intermediate=%v", job, intermediate)
			}
			break
		}
	}
}
