package service

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"golang.org/x/crypto/bcrypt"
)

func TestAuthTokenTypesAreEnforced(t *testing.T) {
	svc := NewAuthService(AuthServiceConfig{
		JWTSecret: "test-secret",
		Users: map[string]string{
			"admin": testPasswordHash(t, "correct-password"),
		},
	})

	pair, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}

	if _, err := svc.ValidateAccessToken(pair.AccessToken); err != nil {
		t.Fatalf("access token should validate for access: %v", err)
	}
	if _, err := svc.ValidateRefreshToken(pair.RefreshToken); err != nil {
		t.Fatalf("refresh token should validate for refresh: %v", err)
	}

	if _, err := svc.ValidateAccessToken(pair.RefreshToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("refresh token accepted as access token: %v", err)
	}
	if _, err := svc.Refresh(context.Background(), pair.AccessToken); !errors.Is(err, ErrInvalidToken) {
		t.Fatalf("access token accepted as refresh token: %v", err)
	}
}

func TestRefreshRevokesOldRefreshToken(t *testing.T) {
	svc := NewAuthService(AuthServiceConfig{
		JWTSecret: "test-secret",
		Users: map[string]string{
			"admin": testPasswordHash(t, "correct-password"),
		},
	})

	pair, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatalf("login: %v", err)
	}

	if _, err := svc.Refresh(context.Background(), pair.RefreshToken); err != nil {
		t.Fatalf("refresh: %v", err)
	}
	if _, err := svc.Refresh(context.Background(), pair.RefreshToken); !errors.Is(err, ErrTokenRevoked) {
		t.Fatalf("old refresh token was not revoked: %v", err)
	}
}

func TestLogoutRevokesCurrentAccessToken(t *testing.T) {
	svc := NewAuthService(AuthServiceConfig{
		JWTSecret: "test-secret",
		Users: map[string]string{
			"admin": testPasswordHash(t, "correct-password"),
		},
	})
	pair, err := svc.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatal(err)
	}
	if err := svc.Logout(context.Background(), pair.RefreshToken, pair.AccessToken); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.ValidateAccessToken(pair.AccessToken); !errors.Is(err, ErrTokenRevoked) {
		t.Fatalf("access token remained valid after logout: %v", err)
	}
}

func TestLoginUsesAccountLockoutAfterRepeatedFailures(t *testing.T) {
	svc := NewAuthService(AuthServiceConfig{
		JWTSecret: "test-secret",
		Users: map[string]string{
			"admin": testPasswordHash(t, "correct-password"),
		},
	}).(*authService)

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

func testPasswordHash(t *testing.T, password string) string {
	t.Helper()
	hash, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.MinCost)
	if err != nil {
		t.Fatalf("hash password: %v", err)
	}
	return string(hash)
}
