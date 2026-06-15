package handler

import (
	"net/http"
	"strings"

	"github.com/gorilla/websocket"
	"github.com/homelab/filemanager/internal/config"
	"github.com/homelab/filemanager/internal/service"
	ws "github.com/homelab/filemanager/internal/websocket"
)

// WebSocketHandler handles WebSocket connections
type WebSocketHandler struct {
	hub            *ws.Hub
	authService    service.AuthService
	allowedOrigins []string
	upgrader       websocket.Upgrader
}

// NewWebSocketHandler creates a new WebSocket handler with optional origin restrictions.
// If allowedOrigins is nil or empty, all origins are allowed (homelab mode).
// Otherwise, only origins in the list are permitted.
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

// checkOrigin validates the request origin against allowed origins.
// Returns true if:
// - No allowed origins are configured (allow all - homelab mode)
// - The origin matches one of the allowed origins
func (h *WebSocketHandler) checkOrigin(r *http.Request) bool {
	// If no restrictions configured, allow all (homelab mode)
	if len(h.allowedOrigins) == 0 {
		return true
	}

	origin := r.Header.Get("Origin")
	if origin == "" {
		// No origin header - could be same-origin or curl/etc
		// Be permissive for non-browser clients
		return true
	}

	// Check against allowed origins
	for _, allowed := range h.allowedOrigins {
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
		suffix := allowed[1:] // Keep the dot: ".example.com"
		// Origin format: "https://sub.example.com" or "http://example.com:3000"
		// Extract host from origin
		host := origin
		if idx := strings.Index(origin, "://"); idx != -1 {
			host = origin[idx+3:]
		}
		// Remove port if present
		if idx := strings.LastIndex(host, ":"); idx != -1 {
			host = host[:idx]
		}
		// Check if host ends with the suffix or equals the domain (without leading dot)
		return strings.HasSuffix(host, suffix) || host == allowed[2:]
	}

	return false
}

// ServeWS handles WebSocket upgrade requests with authentication
func (h *WebSocketHandler) ServeWS(w http.ResponseWriter, r *http.Request) {
	// Extract token from query parameter or Authorization header
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

	// Upgrade the HTTP connection to WebSocket
	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		// Upgrade already sends an error response
		return
	}

	// Create a new client and register with the hub
	client := ws.NewClient(h.hub, conn, claims.UserID)
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
