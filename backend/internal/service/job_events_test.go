package service

import (
	"context"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"testing"
)

func TestJobEventsOwnerIsolationAndOverflow(t *testing.T) {
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
	for i := 0; i < 129; i++ {
		events.Publish(job)
	}
	for range alice {
	}
	stopAlice()
}
