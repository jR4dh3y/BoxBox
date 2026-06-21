package websocket

import (
	"encoding/json"
	"time"

	"github.com/gorilla/websocket"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
)



// Client represents a WebSocket client connection
type Client struct {
	hub *Hub

	// The websocket connection
	conn *websocket.Conn

	// Buffered channel of outbound messages
	send chan []byte

	// User ID from JWT claims (if authenticated)
	userID string
}

// NewClient creates a new Client instance
func NewClient(hub *Hub, conn *websocket.Conn, userID string) *Client {
	return &Client{
		hub:    hub,
		conn:   conn,
		send:   make(chan []byte, config.WSSendBufferSize),
		userID: userID,
	}
}

// ReadPump pumps messages from the websocket connection to the hub
// This runs in a separate goroutine for each client
func (c *Client) ReadPump() {
	defer func() {
		c.hub.Unregister(c)
		c.conn.Close()
	}()

	c.conn.SetReadLimit(config.WSMaxMessageSize)
	c.conn.SetReadDeadline(time.Now().Add(config.WSPongWait))
	c.conn.SetPongHandler(func(string) error {
		c.conn.SetReadDeadline(time.Now().Add(config.WSPongWait))
		return nil
	})

	for {
		_, message, err := c.conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				// Log unexpected close errors if needed
			}
			break
		}

		c.handleMessage(message)
	}
}

// WritePump pumps messages from the hub to the websocket connection
// This runs in a separate goroutine for each client
func (c *Client) WritePump() {
	ticker := time.NewTicker(config.WSPingPeriod)
	defer func() {
		ticker.Stop()
		c.conn.Close()
	}()

	for {
		select {
		case message, ok := <-c.send:
			c.conn.SetWriteDeadline(time.Now().Add(config.WSWriteWait))
			if !ok {
				// The hub closed the channel
				c.conn.WriteMessage(websocket.CloseMessage, []byte{})
				return
			}

			w, err := c.conn.NextWriter(websocket.TextMessage)
			if err != nil {
				return
			}
			w.Write(message)

			// Add queued messages to the current websocket message
			n := len(c.send)
			for i := 0; i < n; i++ {
				w.Write([]byte{'\n'})
				w.Write(<-c.send)
			}

			if err := w.Close(); err != nil {
				return
			}
		case <-ticker.C:
			c.conn.SetWriteDeadline(time.Now().Add(config.WSWriteWait))
			if err := c.conn.WriteMessage(websocket.PingMessage, nil); err != nil {
				return
			}
		}
	}
}

// handleMessage processes incoming messages from the client
func (c *Client) handleMessage(message []byte) {
	var msg ClientMessage
	if err := json.Unmarshal(message, &msg); err != nil {
		c.hub.SendError(c, "Invalid message format")
		return
	}

	switch msg.Type {
	case MessageTypeSubscribe:
		c.handleSubscribe(msg)
	case MessageTypeUnsubscribe:
		c.handleUnsubscribe(msg)
	case MessageTypePing:
		c.handlePing()
	default:
		c.hub.SendError(c, "Unknown message type")
	}
}

// handleSubscribe handles job subscription requests
func (c *Client) handleSubscribe(msg ClientMessage) {
	if msg.JobID == "" {
		c.hub.SendError(c, "Job ID is required for subscription")
		return
	}
	c.hub.SubscribeToJob(c, msg.JobID)
}

// handleUnsubscribe handles job unsubscription requests
func (c *Client) handleUnsubscribe(msg ClientMessage) {
	if msg.JobID == "" {
		c.hub.SendError(c, "Job ID is required for unsubscription")
		return
	}
	c.hub.UnsubscribeFromJob(c, msg.JobID)
}

// handlePing handles ping messages from the client
func (c *Client) handlePing() {
	c.hub.SendPong(c)
}

// UserID returns the user ID associated with this client
func (c *Client) UserID() string {
	return c.userID
}

// Send sends a message to the client
func (c *Client) Send(message []byte) bool {
	select {
	case c.send <- message:
		return true
	default:
		return false
	}
}
