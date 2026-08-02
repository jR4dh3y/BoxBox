package handler

import (
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
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
	AccessToken string `json:"accessToken"`
	ExpiresAt   string `json:"expiresAt"`
}

const refreshCookieName = "boxbox_refresh"

// Login handles user login requests
// POST /api/v1/auth/login
func (h *AuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	if !sameOriginRequest(r) {
		writeError(w, "Cross-origin authentication request rejected", "FORBIDDEN", http.StatusForbidden)
		return
	}
	var req LoginRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
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

	h.setRefreshCookie(w, r, tokenPair.RefreshToken, tokenPair.RefreshExpiresAt)
	resp := LoginResponse{
		AccessToken: tokenPair.AccessToken,
		ExpiresAt:   tokenPair.ExpiresAt.Format(time.RFC3339),
	}

	writeJSON(w, resp, http.StatusOK)
}

// Refresh handles token refresh requests
// POST /api/v1/auth/refresh
func (h *AuthHandler) Refresh(w http.ResponseWriter, r *http.Request) {
	if !sameOriginRequest(r) {
		writeError(w, "Cross-origin authentication request rejected", "FORBIDDEN", http.StatusForbidden)
		return
	}
	refreshToken, err := h.refreshTokenFromCookie(r)
	if err != nil {
		writeError(w, "Invalid refresh token", "TOKEN_INVALID", http.StatusUnauthorized)
		return
	}

	// Attempt refresh
	tokenPair, err := h.authService.Refresh(r.Context(), refreshToken)
	if err != nil {
		h.clearRefreshCookie(w, r)
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

	h.setRefreshCookie(w, r, tokenPair.RefreshToken, tokenPair.RefreshExpiresAt)
	resp := LoginResponse{
		AccessToken: tokenPair.AccessToken,
		ExpiresAt:   tokenPair.ExpiresAt.Format(time.RFC3339),
	}

	writeJSON(w, resp, http.StatusOK)
}

// Logout handles user logout requests
// POST /api/v1/auth/logout
func (h *AuthHandler) Logout(w http.ResponseWriter, r *http.Request) {
	if !sameOriginRequest(r) {
		writeError(w, "Cross-origin authentication request rejected", "FORBIDDEN", http.StatusForbidden)
		return
	}
	refreshToken, err := h.refreshTokenFromCookie(r)
	if err != nil {
		h.clearRefreshCookie(w, r)
		writeError(w, "Invalid refresh token", "TOKEN_INVALID", http.StatusUnauthorized)
		return
	}
	h.clearRefreshCookie(w, r)

	accessToken := strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer ")
	if err := h.authService.Logout(r.Context(), refreshToken, accessToken); err != nil {
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

func (h *AuthHandler) refreshTokenFromCookie(r *http.Request) (string, error) {
	cookie, err := r.Cookie(refreshCookieName)
	if err != nil || cookie.Value == "" {
		return "", http.ErrNoCookie
	}
	return cookie.Value, nil
}

func (h *AuthHandler) setRefreshCookie(w http.ResponseWriter, r *http.Request, token string, expiresAt time.Time) {
	http.SetCookie(w, &http.Cookie{
		Name:     refreshCookieName,
		Value:    token,
		Path:     "/api/v1/auth",
		Expires:  expiresAt,
		MaxAge:   max(1, int(time.Until(expiresAt).Seconds())),
		HttpOnly: true,
		Secure:   requestIsHTTPS(r),
		SameSite: http.SameSiteStrictMode,
	})
}

func (h *AuthHandler) clearRefreshCookie(w http.ResponseWriter, r *http.Request) {
	http.SetCookie(w, &http.Cookie{
		Name:     refreshCookieName,
		Value:    "",
		Path:     "/api/v1/auth",
		Expires:  time.Unix(1, 0),
		MaxAge:   -1,
		HttpOnly: true,
		Secure:   requestIsHTTPS(r),
		SameSite: http.SameSiteStrictMode,
	})
}

func requestIsHTTPS(r *http.Request) bool {
	return r.TLS != nil || strings.EqualFold(r.Header.Get("X-Forwarded-Proto"), "https")
}

func sameOriginRequest(r *http.Request) bool {
	origin := r.Header.Get("Origin")
	if origin == "" {
		return true
	}
	parsed, err := url.Parse(origin)
	return err == nil && parsed.Host != "" && strings.EqualFold(parsed.Host, r.Host)
}
