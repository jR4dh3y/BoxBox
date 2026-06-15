package service

import (
	"context"
	"errors"
	"testing"
)

func TestAuthTokenTypesAreEnforced(t *testing.T) {
	svc := NewAuthService(AuthServiceConfig{
		JWTSecret: "test-secret",
		Users: map[string]string{
			"admin": "correct-password",
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
			"admin": "correct-password",
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
