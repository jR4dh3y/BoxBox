package service

import (
	"context"
	"errors"
	"sort"
	"strings"
	"sync"
	"time"
	"unicode"
	"uuid"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type transferPath struct{ server, path string }

// Qualified references are consumed here, never by a local filesystem resolver.
func parseTransferPath(value string) (transferPath, error) {
	ref := transferPath{path: value}
	if value == "@remote" || strings.HasPrefix(value, "@remote/") {
		parts := strings.SplitN(value, "/", 3)
		if len(parts) != 3 {
			return ref, ErrInvalidJobParams
		}
		id, err := uuid.Parse(parts[1])
		if err != nil || !strings.EqualFold(id.String(), parts[1]) {
			return ref, ErrInvalidJobParams
		}
		ref = transferPath{server: parts[1], path: parts[2]}
	}
	if ref.path == "" || strings.ContainsAny(ref.path, "\\") || strings.HasPrefix(ref.path, "/") {
		return ref, ErrInvalidJobParams
	}
	for _, r := range ref.path {
		if unicode.IsControl(r) {
			return ref, ErrInvalidJobParams
		}
	}
	for _, p := range strings.Split(ref.path, "/") {
		if p == "" || p == "." || p == ".." {
			return ref, ErrInvalidJobParams
		}
	}
	if strings.HasPrefix(ref.path, "@remote/") {
		return ref, ErrInvalidJobParams
	}
	return ref, nil
}

type desktopTransfer struct {
	job    model.Job
	cancel context.CancelFunc
}
type DesktopTransfers struct {
	mu      sync.Mutex
	jobs    map[string]*desktopTransfer
	servers *DesktopServers
	files   FileService
	uploads UploadService
}

func NewDesktopTransfers(fsys filesystem.FS, mounts []model.MountPoint, servers *DesktopServers, tempDir string) *DesktopTransfers {
	files := NewFileService(fsys, FileServiceConfig{MountPoints: mounts})
	return &DesktopTransfers{jobs: make(map[string]*desktopTransfer), servers: servers, files: files, uploads: NewUploadService(files, tempDir)}
}
func (s *DesktopTransfers) List() []*model.Job {
	s.mu.Lock()
	defer s.mu.Unlock()
	result := make([]*model.Job, 0, len(s.jobs))
	for _, t := range s.jobs {
		copy := t.job
		result = append(result, &copy)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].CreatedAt.Before(result[j].CreatedAt) })
	return result
}
func (s *DesktopTransfers) Get(id string) (*model.Job, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	t, ok := s.jobs[id]
	if !ok {
		return nil, ErrJobNotFound
	}
	copy := t.job
	return &copy, nil
}
func (s *DesktopTransfers) Cancel(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	t, ok := s.jobs[id]
	if !ok {
		return ErrJobNotFound
	}
	if t.job.State.IsTerminal() {
		return ErrJobNotCancellable
	}
	t.cancel()
	return nil
}
func (s *DesktopTransfers) Create(ctx context.Context, params model.JobParams) (*model.Job, error) {
	source, err := parseTransferPath(params.SourcePath)
	if err != nil {
		return nil, err
	}
	dest, err := parseTransferPath(params.DestPath)
	if err != nil {
		return nil, err
	}
	if source.server == dest.server {
		return nil, ErrInvalidJobParams
	}
	if params.Type == model.JobTypeMove {
		return nil, ErrCrossSourceMove
	}
	if params.Type != model.JobTypeCopy {
		return nil, ErrInvalidJobType
	}
	if s.servers == nil {
		return nil, ErrDesktopServerStorage
	}
	// Confirm both authorities and destination capability before starting any mutation.
	for _, ref := range []transferPath{source, dest} {
		if ref.server != "" {
			if _, err := s.servers.record(ref.server); err != nil {
				return nil, err
			}
		}
	}
	if err := s.destinationReady(ctx, dest); err != nil {
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if len(s.jobs) >= 100 {
		for id, t := range s.jobs {
			if t.job.State.IsTerminal() {
				delete(s.jobs, id)
			}
		}
	}
	if len(s.jobs) >= 100 {
		return nil, ErrInvalidOperation
	}
	run, cancel := context.WithCancel(context.Background())
	job := model.Job{ID: "transfer-" + uuid.New().String(), Type: params.Type, SourcePath: params.SourcePath, DestPath: params.DestPath, State: model.JobStatePending, CreatedAt: time.Now()}
	s.jobs[job.ID] = &desktopTransfer{job: job, cancel: cancel}
	go s.run(run, job.ID, source, dest)
	return &job, nil
}
func (s *DesktopTransfers) run(ctx context.Context, id string, source, dest transferPath) {

	s.mu.Lock()
	t := s.jobs[id]
	defer t.cancel()
	t.job.State = model.JobStateRunning
	t.job.StartedAt = time.Now()
	s.mu.Unlock()
	remaining := 10000
	err := s.copyTree(ctx, source, dest, 0, &remaining)
	s.mu.Lock()
	defer s.mu.Unlock()
	t.job.CompletedAt = time.Now()
	switch {
	case errors.Is(err, context.Canceled) || ctx.Err() != nil:
		t.job.State = model.JobStateCancelled
	case err != nil:
		t.job.State = model.JobStateFailed
		t.job.Error = transferErrorMessage(err)
	default:
		t.job.State = model.JobStateCompleted
		t.job.Progress = 100
	}
}
func transferErrorMessage(err error) string {
	switch {
	case errors.Is(err, ErrPathExists):
		return "Destination already exists"
	case errors.Is(err, ErrRemoteAuthRequired):
		return "Remote server requires pairing"
	case errors.Is(err, ErrRemoteUnavailable):
		return "Remote server unavailable"
	case errors.Is(err, ErrPermissionDenied):
		return "Permission denied"
	case errors.Is(err, ErrChecksumMismatch):
		return "Source changed or checksum verification failed"
	default:
		return "Copy failed; source retained. A partial destination may remain."
	}
}
