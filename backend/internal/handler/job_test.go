package handler

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type stubJobService struct {
	service.JobService
	jobs []*model.Job
}

func (s stubJobService) List(context.Context) ([]*model.Job, error) { return s.jobs, nil }

func (s stubJobService) Get(_ context.Context, id string) (*model.Job, error) {
	for _, job := range s.jobs {
		if job.ID == id {
			return job, nil
		}
	}
	return nil, service.ErrJobNotFound
}

func serveJobs(t *testing.T, jobs []*model.Job, path string) *httptest.ResponseRecorder {
	t.Helper()
	router := chi.NewRouter()
	router.Route("/api/v1/jobs", NewJobHandler(stubJobService{jobs: jobs}).RegisterRoutes)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, path, nil))
	return rec
}

func TestListJobsReturnsEmptyArrayWhenThereAreNoJobs(t *testing.T) {
	rec := serveJobs(t, nil, "/api/v1/jobs")
	if got := strings.TrimSpace(rec.Body.String()); got != `{"jobs":[]}` {
		t.Fatalf("body = %s, want {\"jobs\":[]}", got)
	}
}

func TestGetJobHidesInternalFieldsAndKeepsTimesParseable(t *testing.T) {
	created := time.Date(2026, 10, 1, 7, 35, 37, 123456789, time.UTC)
	job := &model.Job{
		ID:                 "job-1",
		Owner:              "alice",
		Type:               model.JobTypeCopy,
		State:              model.JobStateRunning,
		SourcePath:         "media/a.txt",
		DestPath:           "media/b.txt",
		ResolvedSourcePath: "/srv/media/a.txt",
		ResolvedDestPath:   "/srv/media/b.txt",
		ResolvedSourceRoot: "/srv/media",
		ResolvedDestRoot:   "/srv/media",
		CreatedAt:          created,
	}
	rec := serveJobs(t, []*model.Job{job}, "/api/v1/jobs/job-1")

	body := rec.Body.String()
	for _, leaked := range []string{"alice", "/srv/media", "owner", "resolved"} {
		if strings.Contains(strings.ToLower(body), strings.ToLower(leaked)) {
			t.Fatalf("response leaks %q: %s", leaked, body)
		}
	}

	var fields map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &fields); err != nil {
		t.Fatal(err)
	}
	if _, ok := fields["startedAt"]; ok {
		t.Fatalf("zero startedAt should be omitted: %s", body)
	}
	parsed, err := time.Parse(time.RFC3339Nano, fields["createdAt"].(string))
	if err != nil || !parsed.Equal(created) {
		t.Fatalf("createdAt = %v (%v), want %v", fields["createdAt"], err, created)
	}
}
