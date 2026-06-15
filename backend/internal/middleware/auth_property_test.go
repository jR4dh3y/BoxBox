// Package middleware provides HTTP middleware for the file manager.
// This file contains property-based tests for authentication enforcement.
package middleware

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/homelab/filemanager/internal/service"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// **Feature: homelab-file-manager, Property 12: Authentication Enforcement**
// **Validates: Requirements 7.1, 7.5**
//
// Property: For any API request without a valid JWT token, the response SHALL be HTTP 401 status code.

// testAuthService is a minimal auth service implementation for testing
type testAuthService struct {
	jwtSecret []byte
}

func newTestAuthService(secret string) *testAuthService {
	return &testAuthService{jwtSecret: []byte(secret)}
}

func (s *testAuthService) Login(ctx context.Context, username, password string) (*service.TokenPair, error) {
	return nil, nil
}

func (s *testAuthService) Refresh(ctx context.Context, refreshToken string) (*service.TokenPair, error) {
	return nil, nil
}

func (s *testAuthService) Logout(ctx context.Context, refreshToken string) error {
	return nil
}

func (s *testAuthService) ValidateToken(tokenString string) (*service.Claims, error) {
	return s.validateToken(tokenString, "")
}

func (s *testAuthService) ValidateAccessToken(tokenString string) (*service.Claims, error) {
	return s.validateToken(tokenString, service.TokenTypeAccess)
}

func (s *testAuthService) ValidateRefreshToken(tokenString string) (*service.Claims, error) {
	return s.validateToken(tokenString, service.TokenTypeRefresh)
}

func (s *testAuthService) validateToken(tokenString string, expectedType service.TokenType) (*service.Claims, error) {
	token, err := jwt.ParseWithClaims(tokenString, &service.Claims{}, func(token *jwt.Token) (interface{}, error) {
		if _, ok := token.Method.(*jwt.SigningMethodHMAC); !ok {
			return nil, service.ErrInvalidToken
		}
		return s.jwtSecret, nil
	})

	if err != nil {
		if err == jwt.ErrTokenExpired {
			return nil, service.ErrTokenExpired
		}
		return nil, service.ErrInvalidToken
	}

	claims, ok := token.Claims.(*service.Claims)
	if !ok || !token.Valid {
		return nil, service.ErrInvalidToken
	}
	if expectedType != "" && claims.TokenType != expectedType {
		return nil, service.ErrInvalidToken
	}

	return claims, nil
}

func (s *testAuthService) StartCleanup(ctx context.Context) {
	// No-op for testing
}

func (s *testAuthService) StopCleanup() {
	// No-op for testing
}

// generateValidToken creates a valid JWT token for testing
func generateValidToken(secret string, username string, expiry time.Duration) string {
	claims := &service.Claims{
		UserID:    "test-user-id",
		Username:  username,
		TokenType: service.TokenTypeAccess,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(time.Now().Add(expiry)),
			IssuedAt:  jwt.NewNumericDate(time.Now()),
			NotBefore: jwt.NewNumericDate(time.Now()),
			Issuer:    "homelab-filemanager",
			Subject:   username,
		},
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	tokenString, _ := token.SignedString([]byte(secret))
	return tokenString
}

// generateExpiredToken creates an expired JWT token for testing
func generateExpiredToken(secret string, username string) string {
	claims := &service.Claims{
		UserID:    "test-user-id",
		Username:  username,
		TokenType: service.TokenTypeAccess,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(time.Now().Add(-1 * time.Hour)),
			IssuedAt:  jwt.NewNumericDate(time.Now().Add(-2 * time.Hour)),
			NotBefore: jwt.NewNumericDate(time.Now().Add(-2 * time.Hour)),
			Issuer:    "homelab-filemanager",
			Subject:   username,
		},
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	tokenString, _ := token.SignedString([]byte(secret))
	return tokenString
}

// TestAuthenticationEnforcement is a property-based test that verifies
// requests without valid JWT tokens always receive HTTP 401 status.
func TestAuthenticationEnforcement(t *testing.T) {
	const testSecret = "test-secret-key-for-jwt-signing"
	authService := newTestAuthService(testSecret)

	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Create a protected handler that should only be reached with valid auth
	protectedHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("protected content"))
	})

	// Wrap with auth middleware
	handler := JWTAuth(authService)(protectedHandler)

	// Generator for HTTP methods
	methodGen := gen.OneConstOf("GET", "POST", "PUT", "DELETE", "PATCH")

	// Generator for request paths
	pathGen := gen.OneConstOf(
		"/api/v1/files",
		"/api/v1/files/documents",
		"/api/v1/jobs",
		"/api/v1/jobs/123",
		"/api/v1/search",
	)

	// Generator for invalid authorization header formats
	invalidAuthHeaderGen := gen.OneConstOf(
		"",                   // Empty
		"Basic dXNlcjpwYXNz", // Wrong scheme
		"Bearer",             // Missing token
		"Bearer ",            // Empty token
		"bearer valid-token", // Wrong case
		"BEARER valid-token", // Wrong case
		"Token valid-token",  // Wrong scheme
	)

	// Generator for random strings (invalid tokens)
	randomStringGen := gen.Identifier().Map(func(s string) string {
		if len(s) > 50 {
			return s[:50]
		}
		return s
	})

	// Property 1: Requests without Authorization header should return 401
	properties.Property("requests without Authorization header return 401", prop.ForAll(
		func(method string, path string) bool {
			req := httptest.NewRequest(method, path, nil)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusUnauthorized
		},
		methodGen,
		pathGen,
	))

	// Property 2: Requests with invalid Authorization header format should return 401
	properties.Property("requests with invalid auth header format return 401", prop.ForAll(
		func(method string, path string, authHeader string) bool {
			req := httptest.NewRequest(method, path, nil)
			if authHeader != "" {
				req.Header.Set("Authorization", authHeader)
			}
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusUnauthorized
		},
		methodGen,
		pathGen,
		invalidAuthHeaderGen,
	))

	// Property 3: Requests with random/invalid tokens should return 401
	properties.Property("requests with invalid tokens return 401", prop.ForAll(
		func(method string, path string, randomToken string) bool {
			req := httptest.NewRequest(method, path, nil)
			req.Header.Set("Authorization", "Bearer "+randomToken)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusUnauthorized
		},
		methodGen,
		pathGen,
		randomStringGen,
	))

	// Property 4: Requests with expired tokens should return 401
	properties.Property("requests with expired tokens return 401", prop.ForAll(
		func(method string, path string, username string) bool {
			expiredToken := generateExpiredToken(testSecret, username)

			req := httptest.NewRequest(method, path, nil)
			req.Header.Set("Authorization", "Bearer "+expiredToken)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusUnauthorized
		},
		methodGen,
		pathGen,
		gen.Identifier(),
	))

	// Property 5: Requests with tokens signed with wrong secret should return 401
	properties.Property("requests with wrong-secret tokens return 401", prop.ForAll(
		func(method string, path string, username string, wrongSecret string) bool {
			// Generate token with a different secret
			tokenWithWrongSecret := generateValidToken(wrongSecret, username, time.Hour)

			req := httptest.NewRequest(method, path, nil)
			req.Header.Set("Authorization", "Bearer "+tokenWithWrongSecret)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusUnauthorized
		},
		methodGen,
		pathGen,
		gen.Identifier(),
		gen.Identifier().SuchThat(func(s string) bool {
			return s != testSecret && len(s) > 0
		}),
	))

	// Property 6: Valid tokens should allow access (returns 200)
	properties.Property("requests with valid tokens return 200", prop.ForAll(
		func(method string, path string, username string) bool {
			validToken := generateValidToken(testSecret, username, time.Hour)

			req := httptest.NewRequest(method, path, nil)
			req.Header.Set("Authorization", "Bearer "+validToken)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusOK
		},
		methodGen,
		pathGen,
		gen.Identifier().SuchThat(func(s string) bool {
			return len(s) > 0
		}),
	))

	properties.TestingRun(t)
}

// TestMalformedTokensRejected tests that various malformed tokens are rejected
func TestMalformedTokensRejected(t *testing.T) {
	const testSecret = "test-secret-key-for-jwt-signing"
	authService := newTestAuthService(testSecret)

	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100

	properties := gopter.NewProperties(parameters)

	protectedHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	handler := JWTAuth(authService)(protectedHandler)

	// Generator for malformed JWT-like strings
	malformedTokenGen := gen.OneConstOf(
		"not.a.jwt",
		"header.payload",
		"a.b.c.d.e",
		"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9", // Only header
		"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0", // Missing signature
		"eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiIxMjM0NTY3ODkwIn0.", // Algorithm none
		".....",
		"",
		" ",
		"Bearer",
	)

	properties.Property("malformed tokens are rejected with 401", prop.ForAll(
		func(malformedToken string) bool {
			req := httptest.NewRequest("GET", "/api/v1/files", nil)
			req.Header.Set("Authorization", "Bearer "+malformedToken)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			return rec.Code == http.StatusUnauthorized
		},
		malformedTokenGen,
	))

	properties.TestingRun(t)
}
