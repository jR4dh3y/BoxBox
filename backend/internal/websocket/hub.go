package websocket

import (
	"context"
	"encoding/json"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// Hub maintains the set of active clients and broadcasts messages to them
type Hub struct {
	// Registered clients
	clients map[*Client]bool

	// Job subscriptions - maps job ID to subscribed clients
	jobSubscriptions map[string]map[*Client]bool

	// Inbound messages from clients
	broadcast chan []byte

	// Register requests from clients
	register chan *Client

	// Unregister requests from clients
	unregister chan *Client

	// Mutex for concurrent access
	mu sync.RWMutex
}

// NewHub creates a new Hub instance
func NewHub() *Hub {
	return &Hub{
		clients:          make(map[*Client]bool),
		jobSubscriptions: make(map[string]map[*Client]bool),
		broadcast:        make(chan []byte, 256),
		register:         make(chan *Client),
		unregister:       make(chan *Client),
	}
}

// Run starts the hub's main event loop
func (h *Hub) Run(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			h.shutdown()
			return
		case client := <-h.register:
			h.registerClient(client)
		case client := <-h.unregister:
			h.unregisterClient(client)
		case message := <-h.broadcast:
			h.broadcastMessage(message)
		}
	}
}

// registerClient adds a client to the hub
func (h *Hub) registerClient(client *Client) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.clients[client] = true
}

// unregisterClient removes a client from the hub and all subscriptions
func (h *Hub) unregisterClient(client *Client) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if _, ok := h.clients[client]; ok {
		delete(h.clients, client)

		// Remove from all job subscriptions
		for jobID, subscribers := range h.jobSubscriptions {
			delete(subscribers, client)
			if len(subscribers) == 0 {
				delete(h.jobSubscriptions, jobID)
			}
		}

		close(client.send)
	}
}

// broadcastMessage sends a message to all connected clients
func (h *Hub) broadcastMessage(message []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	for client := range h.clients {
		select {
		case client.send <- message:
		default:
			// Client buffer full, skip this message
		}
	}
}

// shutdown closes all client connections
func (h *Hub) shutdown() {
	h.mu.Lock()
	defer h.mu.Unlock()

	for client := range h.clients {
		close(client.send)
		delete(h.clients, client)
	}
}

// Register adds a client to the hub
func (h *Hub) Register(client *Client) {
	h.register <- client
}

// Unregister removes a client from the hub
func (h *Hub) Unregister(client *Client) {
	h.unregister <- client
}

// SubscribeToJob subscribes a client to job updates
func (h *Hub) SubscribeToJob(client *Client, jobID string) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if h.jobSubscriptions[jobID] == nil {
		h.jobSubscriptions[jobID] = make(map[*Client]bool)
	}
	h.jobSubscriptions[jobID][client] = true
}

// UnsubscribeFromJob unsubscribes a client from job updates
func (h *Hub) UnsubscribeFromJob(client *Client, jobID string) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if subscribers, ok := h.jobSubscriptions[jobID]; ok {
		delete(subscribers, client)
		if len(subscribers) == 0 {
			delete(h.jobSubscriptions, jobID)
		}
	}
}

// BroadcastJobUpdate sends a job update to all connected clients
func (h *Hub) BroadcastJobUpdate(job *model.Job) {
	update := model.JobUpdate{
		JobID:    job.ID,
		State:    job.State,
		Progress: job.Progress,
		Error:    job.Error,
	}

	msg := ServerMessage{
		Type:    MessageTypeJobUpdate,
		Payload: update,
	}

	data, err := json.Marshal(msg)
	if err != nil {
		return
	}

	h.broadcast <- data
}

// SendJobUpdateToSubscribers sends a job update only to subscribed clients
func (h *Hub) SendJobUpdateToSubscribers(job *model.Job) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	subscribers, ok := h.jobSubscriptions[job.ID]
	if !ok || len(subscribers) == 0 {
		// No subscribers, broadcast to all
		h.mu.RUnlock()
		h.BroadcastJobUpdate(job)
		h.mu.RLock()
		return
	}

	update := model.JobUpdate{
		JobID:    job.ID,
		State:    job.State,
		Progress: job.Progress,
		Error:    job.Error,
	}

	msgType := MessageTypeJobUpdate
	if job.State.IsTerminal() {
		msgType = MessageTypeJobComplete
	}

	msg := ServerMessage{
		Type:    msgType,
		Payload: update,
	}

	data, err := json.Marshal(msg)
	if err != nil {
		return
	}

	for client := range subscribers {
		select {
		case client.send <- data:
		default:
			// Client buffer full, skip
		}
	}
}

// SendError sends an error message to a specific client
func (h *Hub) SendError(client *Client, message string) {
	msg := ServerMessage{
		Type: MessageTypeError,
		Payload: map[string]string{
			"message": message,
		},
	}

	data, err := json.Marshal(msg)
	if err != nil {
		return
	}

	select {
	case client.send <- data:
	default:
		// Client buffer full
	}
}

// SendPong sends a pong response to a client
func (h *Hub) SendPong(client *Client) {
	msg := ServerMessage{
		Type:    MessageTypePong,
		Payload: nil,
	}

	data, err := json.Marshal(msg)
	if err != nil {
		return
	}

	select {
	case client.send <- data:
	default:
		// Client buffer full
	}
}

// ClientCount returns the number of connected clients
func (h *Hub) ClientCount() int {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return len(h.clients)
}
