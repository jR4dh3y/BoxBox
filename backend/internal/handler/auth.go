package handler

import (
	"encoding/json"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/homelab/filemanager/internal/service"
)

// AuthHandler handles authentication-related HTTP requests
type AuthHandler struct {
	authService service.AuthService
}

// NewAuthHandler creates a new auth handler
func NewAuthHandler(authService service.AuthService) *AuthHandler {
	return &AuthHandler{
		authService: authService,
	}
}

// RegisterRoutes registers auth routes on the given router
func (h *AuthHandler) RegisterRoutes(r chi.Router) {
	r.Post("/login", h.Login)
	r.Post("/refresh", h.Refresh)
	r.Post("/logout", h.Logout)
}

// LoginRequest represents the login request body
type LoginRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
}

// LoginResponse represents the login response body
type LoginResponse struct {
	AccessToken  string `json:"accessToken"`
	RefreshToken string `json:"refreshToken"`
	ExpiresAt    string `json:"expiresAt"`
}

// RefreshRequest represents the refresh token request body
type RefreshRequest struct {
	RefreshToken string `json:"refreshToken"`
}

// LogoutRequest represents the logout request body
type LogoutRequest struct {
	RefreshToken string `json:"refreshToken"`
}

// Login handles user login requests
// POST /api/v1/auth/login
func (h *AuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	var req LoginRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, "Invalid request body", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}

	// Validate required fields
	if req.Username == "" || req.Password == "" {
		writeError(w, "Username and password are required", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}

	// Attempt login
	tokenPair, err := h.authService.Login(r.Context(), req.Username, req.Password)
	if err != nil {
		switch err {
		case service.ErrInvalidCredentials:
			writeError(w, "Invalid username or password", "UNAUTHORIZED", http.StatusUnauthorized)
		default:
			writeError(w, "Authentication failed", "INTERNAL_ERROR", http.StatusInternalServerError)
		}
		return
	}

	// Return token pair
	resp := LoginResponse{
		AccessToken:  tokenPair.AccessToken,
		RefreshToken: tokenPair.RefreshToken,
		ExpiresAt:    tokenPair.ExpiresAt.Format("2006-01-02T15:04:05Z07:00"),
	}

	writeJSON(w, resp, http.StatusOK)
}

// Refresh handles token refresh requests
// POST /api/v1/auth/refresh
func (h *AuthHandler) Refresh(w http.ResponseWriter, r *http.Request) {
	var req RefreshRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, "Invalid request body", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}

	// Validate required fields
	if req.RefreshToken == "" {
		writeError(w, "Refresh token is required", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}

	// Attempt refresh
	tokenPair, err := h.authService.Refresh(r.Context(), req.RefreshToken)
	if err != nil {
		switch err {
		case service.ErrTokenExpired:
			writeError(w, "Refresh token expired", "TOKEN_INVALID", http.StatusUnauthorized)
		case service.ErrTokenRevoked:
			writeError(w, "Refresh token has been revoked", "TOKEN_INVALID", http.StatusUnauthorized)
		case service.ErrInvalidToken:
			writeError(w, "Invalid refresh token", "TOKEN_INVALID", http.StatusUnauthorized)
		default:
			writeError(w, "Token refresh failed", "INTERNAL_ERROR", http.StatusInternalServerError)
		}
		return
	}

	// Return new token pair
	resp := LoginResponse{
		AccessToken:  tokenPair.AccessToken,
		RefreshToken: tokenPair.RefreshToken,
		ExpiresAt:    tokenPair.ExpiresAt.Format("2006-01-02T15:04:05Z07:00"),
	}

	writeJSON(w, resp, http.StatusOK)
}

// Logout handles user logout requests
// POST /api/v1/auth/logout
func (h *AuthHandler) Logout(w http.ResponseWriter, r *http.Request) {
	var req LogoutRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, "Invalid request body", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}

	// Validate required fields
	if req.RefreshToken == "" {
		writeError(w, "Refresh token is required", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}

	// Revoke the refresh token
	if err := h.authService.Logout(r.Context(), req.RefreshToken); err != nil {
		switch err {
		case service.ErrTokenExpired, service.ErrTokenRevoked, service.ErrInvalidToken:
			writeError(w, "Invalid refresh token", "TOKEN_INVALID", http.StatusUnauthorized)
		default:
			writeError(w, "Logout failed", "INTERNAL_ERROR", http.StatusInternalServerError)
		}
		return
	}

	writeJSON(w, map[string]string{"message": "Logged out successfully"}, http.StatusOK)
}
