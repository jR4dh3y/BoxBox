package websocket

import (
	"encoding/json"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func TestJobDeliveryIsScopedToOwner(t *testing.T) {
	for _, mode := range []string{"broadcast", "subscribers", "no subscribers"} {
		t.Run(mode, func(t *testing.T) {
			hub := NewHub()
			alice := NewClient(hub, nil, "alice-id", "alice")
			bob := NewClient(hub, nil, "bob-id", "bob")
			hub.registerClient(alice)
			hub.registerClient(bob)
			defer hub.shutdown()
			if mode == "subscribers" {
				// Knowing another user's job ID must not grant access to its updates.
				hub.SubscribeToJob(alice, "private-job")
				hub.SubscribeToJob(bob, "private-job")
			}
			job := &model.Job{ID: "private-job", Owner: "alice", State: model.JobStateRunning, Progress: 25}
			for _, state := range []model.JobState{model.JobStateRunning, model.JobStateCompleted} {
				job.State = state
				if mode == "broadcast" {
					hub.BroadcastJobUpdate(job)
				} else {
					hub.SendJobUpdateToSubscribers(job)
				}
				select {
				case data := <-alice.send:
					var message struct {
						Type    MessageType     `json:"type"`
						Payload model.JobUpdate `json:"payload"`
					}
					if err := json.Unmarshal(data, &message); err != nil {
						t.Fatal(err)
					}
					wantType := MessageTypeJobUpdate
					if mode == "subscribers" && state.IsTerminal() {
						wantType = MessageTypeJobComplete
					}
					if message.Type != wantType || message.Payload.JobID != job.ID || message.Payload.State != state || message.Payload.Progress != job.Progress {
						t.Fatalf("owner received unexpected update: %+v", message)
					}
				default:
					t.Fatal("owner did not receive update")
				}
				select {
				case data := <-bob.send:
					t.Fatalf("another user received private job update: %s", data)
				default:
				}
			}
		})
	}
}
