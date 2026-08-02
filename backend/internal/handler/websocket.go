package handler

import (
	"net/http"
	"net/url"
	"strings"

	"github.com/gorilla/websocket"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	ws "github.com/jR4dh3y/BoxBox/backend/internal/websocket"
)

// WebSocketHandler handles WebSocket connections
type WebSocketHandler struct {
	hub            *ws.Hub
	authService    service.AuthService
	allowedOrigins []string
	devMode        bool
	upgrader       websocket.Upgrader
}

// NewWebSocketHandler creates a new WebSocket handler with optional origin restrictions.
// If allowedOrigins is nil or empty, browser connections must be same-origin.
func NewWebSocketHandler(hub *ws.Hub, authService service.AuthService, allowedOrigins []string) *WebSocketHandler {
	h := &WebSocketHandler{
		hub:            hub,
		authService:    authService,
		allowedOrigins: allowedOrigins,
	}

	h.upgrader = websocket.Upgrader{
		ReadBufferSize:  config.WSReadBufferSize,
		WriteBufferSize: config.WSWriteBufferSize,
		CheckOrigin:     h.checkOrigin,
	}

	return h
}

// SetDevMode disables WebSocket authentication for loopback-only development.
func (h *WebSocketHandler) SetDevMode(enabled bool) {
	h.devMode = enabled
}

// checkOrigin validates the request origin against allowed origins.
func (h *WebSocketHandler) checkOrigin(r *http.Request) bool {
	origin := r.Header.Get("Origin")
	if origin == "" {
		// No origin header - could be same-origin or curl/etc
		// Be permissive for non-browser clients
		return true
	}

	if len(h.allowedOrigins) == 0 {
		parsed, err := url.Parse(origin)
		return err == nil && strings.EqualFold(parsed.Host, r.Host)
	}

	// Check against allowed origins
	for _, allowed := range h.allowedOrigins {
		if allowed == "*" {
			return true
		}
		if matchOrigin(origin, allowed) {
			return true
		}
	}

	return false
}

// matchOrigin checks if the origin matches the allowed pattern.
// Supports exact match and wildcard subdomain matching (e.g., "*.example.com").
func matchOrigin(origin, allowed string) bool {
	// Exact match
	if origin == allowed {
		return true
	}

	// Wildcard subdomain match (e.g., "*.example.com")
	if strings.HasPrefix(allowed, "*.") {
		parsed, err := url.Parse(origin)
		if err != nil || parsed.Hostname() == "" {
			return false
		}
		domain := strings.ToLower(allowed[2:])
		host := strings.ToLower(parsed.Hostname())
		return host == domain || strings.HasSuffix(host, "."+domain)
	}

	return false
}

// ServeWS handles WebSocket upgrade requests with authentication
func (h *WebSocketHandler) ServeWS(w http.ResponseWriter, r *http.Request) {
	userID := "dev"
	username := "dev"

	// Extract token from query parameter or Authorization header
	if !h.devMode {
		token := h.extractToken(r)
		if token == "" {
			http.Error(w, "Missing authentication token", http.StatusUnauthorized)
			return
		}

		// Validate the token
		claims, err := h.authService.ValidateAccessToken(token)
		if err != nil {
			http.Error(w, "Invalid authentication token", http.StatusUnauthorized)
			return
		}
		userID = claims.UserID
		username = claims.Username
	}

	// Upgrade the HTTP connection to WebSocket
	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		// Upgrade already sends an error response
		return
	}

	// Create a new client and register with the hub
	client := ws.NewClient(h.hub, conn, userID, username)
	h.hub.Register(client)

	// Start the client's read and write pumps in separate goroutines
	go client.WritePump()
	go client.ReadPump()
}

// extractToken extracts the JWT token from the request
// It checks the query parameter 'token' first, then the Authorization header
func (h *WebSocketHandler) extractToken(r *http.Request) string {
	// Check query parameter first (useful for WebSocket connections)
	token := r.URL.Query().Get("token")
	if token != "" {
		return token
	}

	// Check Authorization header
	authHeader := r.Header.Get("Authorization")
	if authHeader != "" {
		// Remove "Bearer " prefix if present
		return strings.TrimPrefix(authHeader, "Bearer ")
	}

	return ""
}
