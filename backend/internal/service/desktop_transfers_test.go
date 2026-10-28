package service_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/handler"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type transferFixture struct {
	server    *httptest.Server
	root      string
	mutations atomic.Int32
	supported atomic.Bool
	slow      atomic.Bool
	partial   atomic.Int64 // download bytes sent before the response stalls
	cancelled chan struct{}
}

func newTransferFixture(t *testing.T) *transferFixture {
	t.Helper()
	f := &transferFixture{root: t.TempDir(), cancelled: make(chan struct{}, 1)}
	f.supported.Store(true)
	fs := filesystem.NewOsFS()
	files := service.NewFileService(fs, service.FileServiceConfig{MountPoints: []model.MountPoint{{Name: "files", Path: f.root}}})

	router := chi.NewRouter()
	router.Route("/api/v1", func(r chi.Router) {
		r.Post("/connect/redeem", func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]interface{}{"credential": "boxbox_client_" + strings.Repeat("A", 43), "clientId": "paired-client", "expiresAt": time.Now().Add(time.Hour)})
		})
		r.Group(func(r chi.Router) {
			r.Use(func(next http.Handler) http.Handler {
				return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
					if r.Header.Get("Authorization") != "Bearer boxbox_client_"+strings.Repeat("A", 43) {
						t.Error("missing private bearer")
						http.Error(w, "Unauthorized", 401)
						return
					}
					if r.Method == "POST" {
						f.mutations.Add(1)
					}
					next.ServeHTTP(w, r)
				})
			})
			r.Route("/files", handler.NewFileHandler(files).RegisterRoutes)
			r.Route("/stream", handler.NewStreamHandler(files, 0).RegisterRoutes)
			r.Get("/system/upload", func(w http.ResponseWriter, r *http.Request) {
				w.Header().Set("Content-Type", "application/json")
				json.NewEncoder(w).Encode(map[string]interface{}{"chunkSizeBytes": 1048576, "createOnly": f.supported.Load()})
			})
		})
	})
	f.server = httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if sent := f.partial.Load(); sent > 0 && strings.Contains(r.URL.Path, "/stream/download/files/") {
			data, err := os.ReadFile(filepath.Join(f.root, strings.SplitN(r.URL.Path, "/stream/download/files/", 2)[1]))
			if err != nil {
				http.Error(w, err.Error(), http.StatusNotFound)
				return
			}
			w.Header().Set("Content-Length", fmt.Sprint(len(data)))
			w.Write(data[:sent])
			w.(http.Flusher).Flush()
			<-r.Context().Done()
			return
		}
		if f.slow.Load() && strings.Contains(r.URL.Path, "/stream/download/") {
			select {
			case <-r.Context().Done():
				select {
				case f.cancelled <- struct{}{}:
				default:
				}
			case <-time.After(5 * time.Second):
			}
			return
		}
		router.ServeHTTP(w, r)
	}))
	t.Cleanup(f.server.Close)
	return f
}
func (f *transferFixture) save(t *testing.T, path string, data []byte) {
	t.Helper()
	full := filepath.Join(f.root, path)
	if err := os.MkdirAll(filepath.Dir(full), 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(full, data, 0600); err != nil {
		t.Fatal(err)
	}
}
func transferService(t *testing.T, f *transferFixture) (*service.DesktopTransfers, string, string) {
	t.Helper()
	servers, err := service.NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), f.server.Client())
	if err != nil {
		t.Fatal(err)
	}
	remote, err := servers.Create(context.Background(), "Remote", f.server.URL, "ABCD-EFGH", "Tests", "server")
	if err != nil {
		t.Fatal(err)
	}
	local := t.TempDir()
	s := service.NewDesktopTransfers(filesystem.NewOsFS(), []model.MountPoint{{Name: "files", Path: local}}, servers, t.TempDir())
	return s, "@remote/" + remote.ID + "/files", local
}
func waitTransfer(t *testing.T, s *service.DesktopTransfers, id string, want model.JobState) *model.Job {
	t.Helper()
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		job, err := s.Get(id)
		if err != nil {
			t.Fatal(err)
		}
		if job.State.IsTerminal() {
			if job.State != want {
				t.Fatalf("state %s, expected %s: %s", job.State, want, job.Error)
			}
			return job
		}
		time.Sleep(5 * time.Millisecond)
	}
	t.Fatal("transfer timeout")
	return nil
}
func TestDesktopTransfersTLSCopiesAndNoOverwrite(t *testing.T) {
	f := newTransferFixture(t)
	s, remote, local := transferService(t, f)
	data := bytes.Repeat([]byte("bounded-copy"), 250000)
	f.save(t, "source/large.bin", data)
	f.save(t, "source/empty", nil)
	f.save(t, "source/.hidden", []byte("hidden"))
	// Whole-folder responses must copy entries beyond the former transfer page size.
	for i := range 101 {
		f.save(t, fmt.Sprintf("source/entry-%03d", i), nil)
	}
	job, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: remote + "/source", DestPath: "files/copied"})
	if err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateCompleted)
	copied, err := os.ReadFile(filepath.Join(local, "copied/large.bin"))
	if err != nil || !bytes.Equal(copied, data) {
		t.Fatal("large TLS->local copy differs")
	}
	for _, name := range []string{"empty", ".hidden", "entry-100"} {
		if _, err := os.Stat(filepath.Join(local, "copied", name)); err != nil {
			t.Fatal(err)
		}
	}
	job, err = s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: "files/copied", DestPath: remote + "/back"})
	if err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateCompleted)
	copied, err = os.ReadFile(filepath.Join(f.root, "back/large.bin"))
	if err != nil || !bytes.Equal(copied, data) {
		t.Fatal("local->TLS copy differs")
	}
	if _, err := os.Stat(filepath.Join(f.root, "back/entry-100")); err != nil {
		t.Fatal(err)
	}
	job, err = s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: "files/copied/empty", DestPath: remote + "/back/large.bin"})
	if err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateFailed)
	copied, _ = os.ReadFile(filepath.Join(f.root, "back/large.bin"))
	if !bytes.Equal(copied, data) {
		t.Fatal("overwrote destination")
	}
	if _, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeMove, SourcePath: "files/copied", DestPath: remote + "/moved"}); err != service.ErrCrossSourceMove {
		t.Fatalf("move error: %v", err)
	}
}
func TestDesktopTransfersCapabilityAndCancel(t *testing.T) {
	f := newTransferFixture(t)
	s, remote, local := transferService(t, f)
	os.WriteFile(filepath.Join(local, "source"), []byte("keep"), 0600)
	f.supported.Store(false)
	if _, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: "files/source", DestPath: remote + "/dest"}); err != service.ErrInvalidOperation {
		t.Fatalf("old server accepted: %v", err)
	}
	if f.mutations.Load() != 0 {
		t.Fatal("old server mutated")
	}
	f.save(t, "slow", []byte("source retained"))
	f.slow.Store(true)
	job, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: remote + "/slow", DestPath: "files/cancelled"})
	if err != nil {
		t.Fatal(err)
	}
	time.Sleep(30 * time.Millisecond)
	if err := s.Cancel(job.ID); err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateCancelled)
	select {
	case <-f.cancelled:
	case <-time.After(time.Second):
		t.Fatal("upstream did not observe cancellation")
	}
	if _, err := os.Stat(filepath.Join(f.root, "slow")); err != nil {
		t.Fatal("source lost")
	}
}
func TestDesktopTransferHTTPBoundary(t *testing.T) {
	f := newTransferFixture(t)
	s, remote, _ := transferService(t, f)
	f.save(t, "small", []byte("copy"))
	r := chi.NewRouter()
	handler.NewDesktopTransfersHandler(s).RegisterRoutes(r)
	server := httptest.NewServer(r)
	defer server.Close()
	body, _ := json.Marshal(model.JobParams{Type: model.JobTypeCopy, SourcePath: remote + "/small", DestPath: "files/result"})
	response, err := http.Post(server.URL+"/transfers", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != 202 {
		t.Fatal(response.StatusCode)
	}
	var job model.Job
	if err := json.NewDecoder(response.Body).Decode(&job); err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateCompleted)
	response, err = http.Get(server.URL + "/transfers")
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	contents, _ := io.ReadAll(response.Body)
	if !bytes.Contains(contents, []byte(job.ID)) {
		t.Fatal("job absent from list")
	}
}

func TestDesktopTransfersSeparateAuthoritiesWithOverlappingPaths(t *testing.T) {
	first, second := newTransferFixture(t), newTransferFixture(t)
	first.save(t, "same/file", []byte("first authority"))
	second.save(t, "same/file", []byte("second authority"))
	servers, err := service.NewDesktopServers(filesystem.NewOsFS(), t.TempDir(), first.server.Client())
	if err != nil {
		t.Fatal(err)
	}
	a, err := servers.Create(context.Background(), "First", first.server.URL, "ABCD-EFGH", "Tests", "server")
	if err != nil {
		t.Fatal(err)
	}
	b, err := servers.Create(context.Background(), "Second", second.server.URL, "ABCD-EFGH", "Tests", "server")
	if err != nil {
		t.Fatal(err)
	}
	s := service.NewDesktopTransfers(filesystem.NewOsFS(), nil, servers, t.TempDir())
	job, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: "@remote/" + a.ID + "/files/same", DestPath: "@remote/" + b.ID + "/files/copied"})
	if err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateCompleted)
	data, err := os.ReadFile(filepath.Join(second.root, "copied/file"))
	if err != nil || string(data) != "first authority" {
		t.Fatal("authority identity lost")
	}
	untouched, _ := os.ReadFile(filepath.Join(second.root, "same/file"))
	if string(untouched) != "second authority" {
		t.Fatal("overlapping source path changed")
	}
}

func TestDesktopTransferReportsByteProgress(t *testing.T) {
	f := newTransferFixture(t)
	s, remote, _ := transferService(t, f)
	f.save(t, "source.bin", bytes.Repeat([]byte("a"), 4<<20))
	f.partial.Store(3 << 20)
	job, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: remote + "/source.bin", DestPath: "files/copied.bin"})
	if err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(10 * time.Second)
	for {
		current, err := s.Get(job.ID)
		if err != nil {
			t.Fatal(err)
		}
		if current.Progress == 75 {
			if current.State != model.JobStateRunning || current.BytesTotal != 4<<20 {
				t.Fatalf("state=%s bytesTotal=%d", current.State, current.BytesTotal)
			}
			break
		}
		if current.State.IsTerminal() || time.Now().After(deadline) {
			t.Fatalf("progress stayed at %d in %s total=%d", current.Progress, current.State, current.BytesTotal)
		}
		time.Sleep(5 * time.Millisecond)
	}
	if err := s.Cancel(job.ID); err != nil {
		t.Fatal(err)
	}
	waitTransfer(t, s, job.ID, model.JobStateCancelled)
}

func TestDesktopTransferPublishesLifecycle(t *testing.T) {
	f := newTransferFixture(t)
	s, remote, _ := transferService(t, f)
	f.save(t, "source.bin", bytes.Repeat([]byte("a"), 3<<20))
	updates, stop := s.Watch(context.Background())
	defer stop()
	job, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: remote + "/source.bin", DestPath: "files/copied.bin"})
	if err != nil {
		t.Fatal(err)
	}
	var seen []string
	timeout := time.After(10 * time.Second)
	for {
		select {
		case update := <-updates:
			if update.ID != job.ID {
				continue
			}
			seen = append(seen, fmt.Sprintf("%s:%d:%d", update.State, update.Progress, update.BytesTotal))
			if update.State.IsTerminal() {
				if seen[0] != "pending:0:0" || update.State != model.JobStateCompleted || len(seen) < 5 {
					t.Fatalf("lifecycle %v", seen)
				}
				return
			}
		case <-timeout:
			t.Fatalf("no terminal event: %v", seen)
		}
	}
}

func TestDesktopTransferRefusesExistingDestinationBeforeReading(t *testing.T) {
	f := newTransferFixture(t)
	s, remote, local := transferService(t, f)
	f.save(t, "source.bin", bytes.Repeat([]byte("a"), 2<<20))
	if err := os.WriteFile(filepath.Join(local, "taken.bin"), []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	f.slow.Store(true) // any download would stall for five seconds
	job, err := s.Create(context.Background(), model.JobParams{Type: model.JobTypeCopy, SourcePath: remote + "/source.bin", DestPath: "files/taken.bin"})
	if err != nil {
		t.Fatal(err)
	}
	started := time.Now()
	failed := waitTransfer(t, s, job.ID, model.JobStateFailed)
	if failed.Error != "Destination already exists" || time.Since(started) > 2*time.Second {
		t.Fatalf("error %q after %s", failed.Error, time.Since(started))
	}
	if data, _ := os.ReadFile(filepath.Join(local, "taken.bin")); string(data) != "keep" {
		t.Fatal("destination changed")
	}
}
