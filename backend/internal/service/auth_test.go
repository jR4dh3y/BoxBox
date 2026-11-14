package service

import (
	"context"
	"errors"
	"path/filepath"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"golang.org/x/crypto/bcrypt"
)

func TestAuthTokenTypesAndSessionBinding(t *testing.T) {
	svc := newTestAuthService(t)
	pair, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}

	access, err := svc.ValidateAccessToken(pair.AccessToken)
	if err != nil {
		t.Fatalf("validate access: %v", err)
	}
	refresh, err := svc.ValidateRefreshToken(pair.RefreshToken)
	if err != nil {
		t.Fatalf("validate refresh: %v", err)
	}
	if access.SessionID == "" || access.SessionID != refresh.SessionID {
		t.Fatalf("access and refresh session IDs differ or are empty: %q, %q", access.SessionID, refresh.SessionID)
	}
	if access.ID == "" || refresh.ID == "" || access.ID == refresh.ID {
		t.Fatalf("access and refresh token IDs are not distinct: %q, %q", access.ID, refresh.ID)
	}
	if _, err := svc.ValidateToken(pair.AccessToken); err != nil {
		t.Fatalf("generic validation rejected active access token: %v", err)
	}
	if _, err := svc.ValidateToken(pair.RefreshToken); err != nil {
		t.Fatalf("generic validation rejected current refresh token: %v", err)
	}
	if _, err := svc.ValidateAccessToken(pair.RefreshToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("refresh token accepted as access: %v", err)
	}
	if _, err := svc.ValidateRefreshToken(pair.AccessToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("access token accepted as refresh: %v", err)
	}
	if _, err := svc.Refresh(context.Background(), pair.AccessToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("access token accepted for refresh: %v", err)
	}
}

func TestRefreshKeepsSessionAndOldAccessUntilLogout(t *testing.T) {
	svc := newTestAuthService(t)
	original, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}
	replacement, err := svc.Refresh(context.Background(), original.RefreshToken)
	if err != nil {
		t.Fatalf("refresh: %v", err)
	}

	oldAccess, err := svc.ValidateAccessToken(original.AccessToken)
	if err != nil {
		t.Fatalf("old access token was invalidated by rotation: %v", err)
	}
	newAccess, err := svc.ValidateAccessToken(replacement.AccessToken)
	if err != nil {
		t.Fatalf("new access token: %v", err)
	}
	newRefresh, err := svc.ValidateRefreshToken(replacement.RefreshToken)
	if err != nil {
		t.Fatalf("new refresh token: %v", err)
	}
	if oldAccess.SessionID != newAccess.SessionID || newAccess.SessionID != newRefresh.SessionID {
		t.Fatal("refresh rotation changed the active session ID")
	}
	if _, err := svc.ValidateRefreshToken(original.RefreshToken); !errors.Is(err, ErrRefreshSuperseded) {
		t.Fatalf("rotated refresh token validation = %v, want superseded replay", err)
	}
	if _, err := svc.ValidateToken(original.RefreshToken); !errors.Is(err, ErrRefreshSuperseded) {
		t.Fatalf("generic validation admitted a rotated refresh token: %v", err)
	}
}

func TestLogoutIsSessionWideAndReplayCannotTerminateWinner(t *testing.T) {
	svc := newTestAuthService(t)
	original, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}
	independent, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("independent login: %v", err)
	}
	winner, err := svc.Refresh(context.Background(), original.RefreshToken)
	if err != nil {
		t.Fatalf("refresh: %v", err)
	}

	if err := svc.Logout(context.Background(), original.RefreshToken); !errors.Is(err, ErrRefreshSuperseded) {
		t.Fatalf("replayed logout error = %v, want superseded stale refresh", err)
	}
	for _, token := range []string{winner.AccessToken, winner.RefreshToken} {
		if _, err := svc.ValidateToken(token); err != nil {
			t.Fatalf("stale logout terminated winning session: %v", err)
		}
	}
	if err := svc.Logout(context.Background(), winner.RefreshToken); err != nil {
		t.Fatalf("logout current refresh token: %v", err)
	}
	for _, token := range []string{original.AccessToken, winner.AccessToken, winner.RefreshToken} {
		if _, err := svc.ValidateToken(token); !errors.Is(err, ErrTokenRevoked) {
			t.Fatalf("token from logged-out session validation = %v, want inactive session", err)
		}
	}
	for _, token := range []string{independent.AccessToken, independent.RefreshToken} {
		if _, err := svc.ValidateToken(token); err != nil {
			t.Fatalf("logout affected independent session: %v", err)
		}
	}
}

func TestValidateTokenRejectsUnboundAndLegacyClaims(t *testing.T) {
	svc := newTestAuthService(t)
	pair, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}
	claims, err := svc.ValidateAccessToken(pair.AccessToken)
	if err != nil {
		t.Fatalf("parse issued claims: %v", err)
	}

	cases := []struct {
		name    string
		mutate  func(*Claims)
		wantErr error
	}{
		{
			name: "missing legacy session ID",
			mutate: func(claims *Claims) {
				claims.SessionID = ""
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "missing expiration",
			mutate: func(claims *Claims) {
				claims.ExpiresAt = nil
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "empty token ID",
			mutate: func(claims *Claims) {
				claims.ID = ""
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "malformed session ID",
			mutate: func(claims *Claims) {
				claims.SessionID = "not-a-uuid"
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "wrong issuer",
			mutate: func(claims *Claims) {
				claims.Issuer = "different"
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "unknown token type",
			mutate: func(claims *Claims) {
				claims.TokenType = TokenType("other")
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "username subject mismatch",
			mutate: func(claims *Claims) {
				claims.Subject = "different"
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "user ID mismatch",
			mutate: func(claims *Claims) {
				claims.UserID = "different"
			},
			wantErr: ErrInvalidToken,
		},
		{
			name: "unknown but syntactically valid session",
			mutate: func(claims *Claims) {
				claims.SessionID = "a88b7c22-6ddd-4f3d-8b61-4f1368dca9f1"
			},
			wantErr: ErrTokenRevoked,
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			unbound := *claims
			testCase.mutate(&unbound)
			if _, err := svc.ValidateToken(issueTestToken(t, unbound)); !errors.Is(err, testCase.wantErr) {
				t.Fatalf("generic validation error = %v, want %v", err, testCase.wantErr)
			}
		})
	}

	otherPasswordHash := testPasswordHash(t, "other-password")
	svc.users["other"] = otherPasswordHash
	misbound := *claims
	misbound.Username = "other"
	misbound.UserID = generateUserID("other")
	misbound.Subject = "other"
	if _, err := svc.ValidateToken(issueTestToken(t, misbound)); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("session accepted a token bound to a different configured user: %v", err)
	}
}

func TestValidationRejectsUserRemovedFromConfiguration(t *testing.T) {
	dataDir := filepath.Join(t.TempDir(), "data")
	first := newTestAuthServiceAt(t, dataDir)
	pair, err := first.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}

	cfg := testAuthConfig(t, dataDir)
	cfg.Users = map[string]string{"other": testPasswordHash(t, "other-password")}
	second, err := NewAuthService(filesystem.NewOsFS(), cfg)
	if err != nil {
		t.Fatalf("restart with removed user: %v", err)
	}
	if _, err := second.ValidateAccessToken(pair.AccessToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("removed user's access token validation = %v, want invalid", err)
	}
	if _, err := second.ValidateRefreshToken(pair.RefreshToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("removed user's refresh token validation = %v, want invalid", err)
	}
}

func TestLoginUsesAccountLockoutAfterRepeatedFailures(t *testing.T) {
	svc := newTestAuthService(t)
	for range config.LoginLockoutThreshold {
		_, _ = svc.Login(context.Background(), "admin", "wrong-password")
	}
	if _, err := svc.Login(context.Background(), "admin", "correct-password"); !errors.Is(err, ErrInvalidCredentials) {
		t.Fatalf("locked account accepted credentials: %v", err)
	}

	svc.mu.Lock()
	failure := svc.loginFailures["admin"]
	failure.lockedUntil = time.Now().Add(-time.Second)
	svc.loginFailures["admin"] = failure
	svc.mu.Unlock()
	if _, err := svc.Login(context.Background(), "admin", "correct-password"); err != nil {
		t.Fatalf("account did not recover after lockout: %v", err)
	}
}

func issueTestToken(t *testing.T, claims Claims) string {
	t.Helper()
	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	signed, err := token.SignedString([]byte("test-secret"))
	if err != nil {
		t.Fatalf("sign test token: %v", err)
	}
	return signed
}

func newTestAuthService(t *testing.T) *authService {
	t.Helper()
	return newTestAuthServiceAt(t, filepath.Join(t.TempDir(), "data"))
}

func newTestAuthServiceAt(t *testing.T, dataDir string) *authService {
	t.Helper()
	return newTestAuthServiceWithConfig(t, testAuthConfig(t, dataDir))
}

func newTestAuthServiceWithConfig(t *testing.T, cfg AuthServiceConfig) *authService {
	t.Helper()
	svc, err := NewAuthService(filesystem.NewOsFS(), cfg)
	if err != nil {
		t.Fatalf("create auth service: %v", err)
	}
	auth, ok := svc.(*authService)
	if !ok {
		t.Fatal("auth service has an unexpected implementation")
	}
	return auth
}

func testAuthConfig(t *testing.T, dataDir string) AuthServiceConfig {
	t.Helper()
	return AuthServiceConfig{
		DataDir:   dataDir,
		JWTSecret: "test-secret",
		Users:     map[string]string{"admin": testPasswordHash(t, "correct-password")},
	}
}

func testPasswordHash(t *testing.T, password string) string {
	t.Helper()
	hash, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.MinCost)
	if err != nil {
		t.Fatalf("hash password: %v", err)
	}
	return string(hash)
}

func TestExpiredTokensAreRejectedByRealAuthService(t *testing.T) {
	svc := newTestAuthService(t)
	pair, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatal(err)
	}
	for _, token := range []string{pair.AccessToken, pair.RefreshToken} {
		claims, err := svc.ValidateToken(token)
		if err != nil {
			t.Fatal(err)
		}
		claims.ExpiresAt = jwt.NewNumericDate(time.Now().Add(-time.Hour))
		expired := issueTestToken(t, *claims)
		if _, err := svc.ValidateToken(expired); !errors.Is(err, ErrTokenExpired) {
			t.Fatalf("expired %s token: got %v, want ErrTokenExpired", claims.TokenType, err)
		}
	}
}
