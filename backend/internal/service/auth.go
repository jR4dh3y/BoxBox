package service

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"encoding/hex"
	"errors"
	"sync"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/homelab/filemanager/internal/config"
)

// Auth-related errors
var (
	ErrInvalidCredentials = errors.New("invalid credentials")
	ErrInvalidToken       = errors.New("invalid token")
	ErrTokenExpired       = errors.New("token expired")
	ErrTokenRevoked       = errors.New("token revoked")
)

type TokenType string

const (
	TokenTypeAccess  TokenType = "access"
	TokenTypeRefresh TokenType = "refresh"
)

// Claims represents the JWT claims for access tokens
type Claims struct {
	UserID    string    `json:"user_id"`
	Username  string    `json:"username"`
	TokenType TokenType `json:"token_type"`
	jwt.RegisteredClaims
}

// TokenPair contains both access and refresh tokens
type TokenPair struct {
	AccessToken  string    `json:"accessToken"`
	RefreshToken string    `json:"refreshToken"`
	ExpiresAt    time.Time `json:"expiresAt"`
}

// AuthService defines the authentication service interface
type AuthService interface {
	Login(ctx context.Context, username, password string) (*TokenPair, error)
	Refresh(ctx context.Context, refreshToken string) (*TokenPair, error)
	ValidateToken(tokenString string) (*Claims, error)
	ValidateAccessToken(tokenString string) (*Claims, error)
	ValidateRefreshToken(tokenString string) (*Claims, error)
	Logout(ctx context.Context, refreshToken string) error
	StartCleanup(ctx context.Context)
	StopCleanup()
}

// authService implements AuthService
type authService struct {
	jwtSecret          []byte
	accessTokenExpiry  time.Duration
	refreshTokenExpiry time.Duration
	users              map[string]string // username -> password (in production, use proper storage)
	revokedTokens      map[string]time.Time
	mu                 sync.RWMutex
	stopCh             chan struct{}
	wg                 sync.WaitGroup
}

// AuthServiceConfig holds configuration for the auth service
type AuthServiceConfig struct {
	JWTSecret          string
	AccessTokenExpiry  time.Duration
	RefreshTokenExpiry time.Duration
	Users              map[string]string // username -> password
}

// NewAuthService creates a new authentication service
func NewAuthService(cfg AuthServiceConfig) AuthService {
	if cfg.AccessTokenExpiry == 0 {
		cfg.AccessTokenExpiry = config.DefaultAccessTokenExpiry
	}
	if cfg.RefreshTokenExpiry == 0 {
		cfg.RefreshTokenExpiry = config.DefaultRefreshTokenExpiry
	}
	if cfg.Users == nil {
		cfg.Users = make(map[string]string)
	}

	return &authService{
		jwtSecret:          []byte(cfg.JWTSecret),
		accessTokenExpiry:  cfg.AccessTokenExpiry,
		refreshTokenExpiry: cfg.RefreshTokenExpiry,
		users:              cfg.Users,
		revokedTokens:      make(map[string]time.Time),
		stopCh:             make(chan struct{}),
	}
}

// Login authenticates a user and returns a token pair
func (s *authService) Login(ctx context.Context, username, password string) (*TokenPair, error) {
	// Validate credentials
	storedPassword, exists := s.users[username]
	if !exists || subtle.ConstantTimeCompare([]byte(storedPassword), []byte(password)) != 1 {
		return nil, ErrInvalidCredentials
	}

	return s.generateTokenPair(username)
}

// Refresh generates a new token pair from a valid refresh token
func (s *authService) Refresh(ctx context.Context, refreshToken string) (*TokenPair, error) {
	// Check if token is revoked
	s.mu.RLock()
	if _, revoked := s.revokedTokens[refreshToken]; revoked {
		s.mu.RUnlock()
		return nil, ErrTokenRevoked
	}
	s.mu.RUnlock()

	// Parse and validate the refresh token
	claims, err := s.ValidateRefreshToken(refreshToken)
	if err != nil {
		return nil, err
	}

	// Revoke the old refresh token
	s.mu.Lock()
	s.revokedTokens[refreshToken] = time.Now()
	s.mu.Unlock()

	// Generate new token pair
	return s.generateTokenPair(claims.Username)
}

// ValidateToken validates a JWT token and returns the claims
func (s *authService) ValidateToken(tokenString string) (*Claims, error) {
	return s.validateToken(tokenString, "")
}

func (s *authService) ValidateAccessToken(tokenString string) (*Claims, error) {
	return s.validateToken(tokenString, TokenTypeAccess)
}

func (s *authService) ValidateRefreshToken(tokenString string) (*Claims, error) {
	s.mu.RLock()
	_, revoked := s.revokedTokens[tokenString]
	s.mu.RUnlock()
	if revoked {
		return nil, ErrTokenRevoked
	}

	return s.validateToken(tokenString, TokenTypeRefresh)
}

func (s *authService) validateToken(tokenString string, expectedType TokenType) (*Claims, error) {
	token, err := jwt.ParseWithClaims(tokenString, &Claims{}, func(token *jwt.Token) (interface{}, error) {
		// Validate signing method
		if _, ok := token.Method.(*jwt.SigningMethodHMAC); !ok {
			return nil, ErrInvalidToken
		}
		return s.jwtSecret, nil
	})

	if err != nil {
		if errors.Is(err, jwt.ErrTokenExpired) {
			return nil, ErrTokenExpired
		}
		return nil, ErrInvalidToken
	}

	claims, ok := token.Claims.(*Claims)
	if !ok || !token.Valid {
		return nil, ErrInvalidToken
	}
	if expectedType != "" && claims.TokenType != expectedType {
		return nil, ErrInvalidToken
	}

	return claims, nil
}

// Logout revokes a refresh token
func (s *authService) Logout(ctx context.Context, refreshToken string) error {
	if _, err := s.ValidateRefreshToken(refreshToken); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	s.revokedTokens[refreshToken] = time.Now()
	return nil
}

// generateTokenPair creates a new access and refresh token pair
func (s *authService) generateTokenPair(username string) (*TokenPair, error) {
	now := time.Now()
	userID := generateUserID(username)

	// Create access token
	accessExpiry := now.Add(s.accessTokenExpiry)
	accessClaims := &Claims{
		UserID:    userID,
		Username:  username,
		TokenType: TokenTypeAccess,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(accessExpiry),
			IssuedAt:  jwt.NewNumericDate(now),
			NotBefore: jwt.NewNumericDate(now),
			Issuer:    "homelab-filemanager",
			Subject:   username,
		},
	}

	accessToken := jwt.NewWithClaims(jwt.SigningMethodHS256, accessClaims)
	accessTokenString, err := accessToken.SignedString(s.jwtSecret)
	if err != nil {
		return nil, err
	}

	// Create refresh token
	refreshExpiry := now.Add(s.refreshTokenExpiry)
	refreshClaims := &Claims{
		UserID:    userID,
		Username:  username,
		TokenType: TokenTypeRefresh,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(refreshExpiry),
			IssuedAt:  jwt.NewNumericDate(now),
			NotBefore: jwt.NewNumericDate(now),
			Issuer:    "homelab-filemanager",
			Subject:   username,
		},
	}

	refreshToken := jwt.NewWithClaims(jwt.SigningMethodHS256, refreshClaims)
	refreshTokenString, err := refreshToken.SignedString(s.jwtSecret)
	if err != nil {
		return nil, err
	}

	return &TokenPair{
		AccessToken:  accessTokenString,
		RefreshToken: refreshTokenString,
		ExpiresAt:    accessExpiry,
	}, nil
}

// generateUserID creates a deterministic user ID from username
func generateUserID(username string) string {
	b := make([]byte, 8)
	rand.Read(b)
	return hex.EncodeToString(b)
}

// CleanupExpiredTokens removes expired tokens from the revoked list
// Should be called periodically to prevent memory growth
func (s *authService) CleanupExpiredTokens() {
	s.mu.Lock()
	defer s.mu.Unlock()

	cutoff := time.Now().Add(-s.refreshTokenExpiry)
	for token, revokedAt := range s.revokedTokens {
		if revokedAt.Before(cutoff) {
			delete(s.revokedTokens, token)
		}
	}
}

// StartCleanup starts the periodic cleanup of expired tokens
func (s *authService) StartCleanup(ctx context.Context) {
	s.wg.Add(1)
	go func() {
		defer s.wg.Done()
		ticker := time.NewTicker(config.TokenCleanupInterval)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				return
			case <-s.stopCh:
				return
			case <-ticker.C:
				s.CleanupExpiredTokens()
			}
		}
	}()
}

// StopCleanup stops the cleanup goroutine
func (s *authService) StopCleanup() {
	close(s.stopCh)
	s.wg.Wait()
}
