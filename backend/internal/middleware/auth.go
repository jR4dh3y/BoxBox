package middleware

import (
	"context"
	"net/http"
	"path"
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
type ClientCredentialValidator interface {
	ValidateClientCredential(string) (*service.ConnectClient, error)
}

func JWTAuth(authService service.AuthService, clients ...ClientCredentialValidator) func(next http.Handler) http.Handler {
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
				if strings.HasPrefix(tokenString, service.ClientCredentialPrefix) {
					writeAuthError(w, "Client credentials require an Authorization header", http.StatusUnauthorized)
					return
				}
			}

			// No token found
			if tokenString == "" {
				writeAuthError(w, "Missing authorization", http.StatusUnauthorized)
				return
			}

			if strings.HasPrefix(tokenString, service.ClientCredentialPrefix) {
				if len(clients) == 0 || clients[0] == nil {
					writeAuthError(w, "Client authentication unavailable", http.StatusUnauthorized)
					return
				}
				client, err := clients[0].ValidateClientCredential(tokenString)
				if err != nil || client == nil {
					writeAuthError(w, "Invalid client credential", http.StatusUnauthorized)
					return
				}
				if !clientRequestAllowed(r) {
					writeAuthError(w, "Client credential cannot access this endpoint", http.StatusForbidden)
					return
				}
				ctx := authcontext.WithUsername(r.Context(), client.Username)
				ctx = authcontext.WithClientID(ctx, client.ClientID)
				if r.URL.Path == "/api/v1/jobs/events" {
					ctx = authcontext.WithAuthorization(ctx, func() bool {
						current, err := clients[0].ValidateClientCredential(tokenString)
						return err == nil && current != nil && current.Username == client.Username
					})
				}
				next.ServeHTTP(w, r.WithContext(ctx))
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
			ctx = authcontext.WithAccountSession(ctx)
			if r.URL.Path == "/api/v1/jobs/events" {
				ctx = authcontext.WithAuthorization(ctx, func() bool {
					_, err := authService.ValidateAccessToken(tokenString)
					return err == nil
				})
			}
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

func clientRequestAllowed(r *http.Request) bool {
	requestPath := r.URL.Path
	if path.Clean(requestPath) != strings.TrimSuffix(requestPath, "/") {
		return false
	}
	if requestPath == "/api/v1/connect/client" {
		return r.Method == http.MethodDelete
	}
	if !strings.HasPrefix(requestPath, "/api/v1/") {
		return false
	}
	return service.ClientRouteAllowed(r.Method, strings.TrimPrefix(requestPath, "/api/v1"))
}

// RejectClientCredentials also guards routes outside JWTAuth, including dev WS.
func RejectClientCredentials(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		header := strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer ")
		if strings.HasPrefix(header, service.ClientCredentialPrefix) || strings.HasPrefix(r.URL.Query().Get("token"), service.ClientCredentialPrefix) {
			writeAuthError(w, "Client credentials are unsupported on this endpoint", http.StatusForbidden)
			return
		}
		next.ServeHTTP(w, r)
	})
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
