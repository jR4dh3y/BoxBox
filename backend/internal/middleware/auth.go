package middleware

import (
	"context"
	"net/http"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// ContextKey is a type for context keys to avoid collisions
type ContextKey string

const (
	// UserClaimsKey is the context key for user claims
	UserClaimsKey ContextKey = "userClaims"
)

// JWTAuth creates a middleware that validates JWT tokens
func JWTAuth(authService service.AuthService) func(next http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			var tokenString string

			// First, try Authorization header
			authHeader := r.Header.Get("Authorization")
			if authHeader != "" && strings.HasPrefix(authHeader, "Bearer ") {
				tokenString = strings.TrimPrefix(authHeader, "Bearer ")
			}

			// Browser media elements cannot attach Authorization headers. Query
			// tokens are therefore accepted only by the streaming route.
			if tokenString == "" && strings.HasPrefix(r.URL.Path, "/api/v1/stream/") {
				tokenString = r.URL.Query().Get("token")
			}

			// No token found
			if tokenString == "" {
				writeAuthError(w, "Missing authorization", http.StatusUnauthorized)
				return
			}

			// Validate token
			claims, err := authService.ValidateAccessToken(tokenString)
			if err != nil {
				switch err {
				case service.ErrTokenExpired:
					writeAuthError(w, "Token expired", http.StatusUnauthorized)
				case service.ErrInvalidToken:
					writeAuthError(w, "Invalid token", http.StatusUnauthorized)
				default:
					writeAuthError(w, "Authentication failed", http.StatusUnauthorized)
				}
				return
			}

			// Add claims to context
			ctx := context.WithValue(r.Context(), UserClaimsKey, claims)
			ctx = authcontext.WithUsername(ctx, claims.Username)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

// DevelopmentAuth adds the same identity context as JWTAuth while --dev
// bypasses token verification.
func DevelopmentAuth(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		claims := &service.Claims{UserID: "dev", Username: "dev", TokenType: service.TokenTypeAccess}
		ctx := context.WithValue(r.Context(), UserClaimsKey, claims)
		ctx = authcontext.WithUsername(ctx, "dev")
		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// writeAuthError writes an authentication error response
func writeAuthError(w http.ResponseWriter, message string, status int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	w.Write([]byte(`{"error":"` + message + `","code":"UNAUTHORIZED"}`))
}
