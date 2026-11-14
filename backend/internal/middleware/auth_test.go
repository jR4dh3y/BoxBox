// Package middleware provides HTTP middleware for the file manager.
package middleware

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// testAuthService is a minimal auth service implementation for testing
type testAuthService struct {
	jwtSecret []byte
}

func newTestAuthService(secret string) *testAuthService {
	return &testAuthService{jwtSecret: []byte(secret)}
}

func (s *testAuthService) VerifyPassword(ctx context.Context, username, password string) error {
	return service.ErrInvalidCredentials
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
		if errors.Is(err, jwt.ErrTokenExpired) {
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

func TestJWTAuthAcceptsQueryTokensOnlyForStreams(t *testing.T) {
	const secret = "test-secret-key-for-jwt-signing"
	token := generateValidToken(secret, "admin", time.Hour)
	handler := JWTAuth(newTestAuthService(secret))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}))

	for _, test := range []struct {
		path string
		want int
	}{
		{path: "/api/v1/stream/preview/home/photo.png?token=" + token, want: http.StatusNoContent},
		{path: "/api/v1/files/home/photo.png?token=" + token, want: http.StatusUnauthorized},
		{path: "/api/v1/jobs?token=" + token, want: http.StatusUnauthorized},
	} {
		request := httptest.NewRequest(http.MethodGet, test.path, nil)
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, request)
		if response.Code != test.want {
			t.Errorf("%s: status = %d, want %d", test.path, response.Code, test.want)
		}
	}
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
			Issuer:    "boxbox",
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
			Issuer:    "boxbox",
			Subject:   username,
		},
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	tokenString, _ := token.SignedString([]byte(secret))
	return tokenString
}

func TestAuthenticationEnforcement(t *testing.T) {
	const secret = "test-secret-key-for-jwt-signing"
	valid := generateValidToken(secret, "alice", time.Hour)
	expired := generateExpiredToken(secret, "alice")
	wrongSecret := generateValidToken("different-signing-secret", "alice", time.Hour)

	handler := JWTAuth(newTestAuthService(secret))(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	for _, test := range []struct {
		name     string
		header   string
		want     int
		wantBody string
	}{
		{name: "missing", want: http.StatusUnauthorized},
		{name: "wrong scheme", header: "Basic credentials", want: http.StatusUnauthorized},
		{name: "empty bearer", header: "Bearer ", want: http.StatusUnauthorized},
		{name: "malformed", header: "Bearer not.a.jwt", want: http.StatusUnauthorized},
		{name: "expired", header: "Bearer " + expired, want: http.StatusUnauthorized, wantBody: `{"error":"Token expired","code":"UNAUTHORIZED"}`},
		{name: "wrong secret", header: "Bearer " + wrongSecret, want: http.StatusUnauthorized},
		{name: "valid", header: "Bearer " + valid, want: http.StatusOK},
	} {
		t.Run(test.name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodGet, "/api/v1/files", nil)
			if test.header != "" {
				request.Header.Set("Authorization", test.header)
			}
			recorder := httptest.NewRecorder()
			handler.ServeHTTP(recorder, request)
			if recorder.Code != test.want {
				t.Fatalf("status = %d, want %d", recorder.Code, test.want)
			}
			if test.wantBody != "" && recorder.Body.String() != test.wantBody {
				t.Errorf("body = %q, want %q", recorder.Body.String(), test.wantBody)
			}
		})
	}
}
