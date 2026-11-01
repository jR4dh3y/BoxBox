package service

import (
	"context"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
)

// JobEvents publishes immutable, owner-scoped snapshots. Slow readers reconnect and reconcile.
type JobEvents struct {
	mu      sync.Mutex
	readers map[chan model.Job]string
}

func (e *JobEvents) Watch(ctx context.Context) (<-chan model.Job, func()) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.readers == nil {
		e.readers = make(map[chan model.Job]string)
	}
	updates := make(chan model.Job, 128)
	e.readers[updates] = authcontext.Username(ctx)
	return updates, func() {
		e.mu.Lock()
		defer e.mu.Unlock()
		if _, ok := e.readers[updates]; ok {
			delete(e.readers, updates)
			close(updates)
		}
	}
}

func (e *JobEvents) Publish(job model.Job) {
	e.mu.Lock()
	defer e.mu.Unlock()
	for updates, owner := range e.readers {
		if owner != job.Owner {
			continue
		}
		select {
		case updates <- job:
		default:
			delete(e.readers, updates)
			close(updates)
		}
	}
}
