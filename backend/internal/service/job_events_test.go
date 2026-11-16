package service

import (
	"context"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
)

func TestJobEventsOwnerIsolationAndSnapshot(t *testing.T) {
	var events JobEvents
	alice, stopAlice := events.Watch(authcontext.WithUsername(context.Background(), "alice"))
	defer stopAlice()
	bob, stopBob := events.Watch(authcontext.WithUsername(context.Background(), "bob"))
	defer stopBob()
	job := model.Job{ID: "one", Owner: "alice", Progress: 12}
	events.Publish(job)
	job.Progress = 99
	if got := <-alice; got.Progress != 12 {
		t.Fatal("published snapshot mutated")
	}
	select {
	case <-bob:
		t.Fatal("owner leak")
	default:
	}
}

// A full watcher is dropped and closed so slow readers reconnect instead of blocking Publish.
func TestJobEventsPublishClosesOverflowedWatcher(t *testing.T) {
	var events JobEvents
	updates, stop := events.Watch(authcontext.WithUsername(context.Background(), "alice"))
	defer stop()
	for i := 0; i < 129; i++ {
		events.Publish(model.Job{ID: "one", Owner: "alice"})
	}
	timeout := time.NewTimer(time.Second)
	defer timeout.Stop()
	received := 0
	for {
		select {
		case _, ok := <-updates:
			if !ok {
				if received != 128 {
					t.Fatalf("received %d buffered updates before close, want 128", received)
				}
				return
			}
			received++
		case <-timeout.C:
			t.Fatal("overflowed watcher channel did not close")
		}
	}
}
