package service

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"maps"
	"path/filepath"
	"strings"
	"sync"
	"time"
	"uuid"

	"github.com/golang-jwt/jwt/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/rs/zerolog/log"
	"golang.org/x/crypto/bcrypt"
)

// Auth-related errors
var (
	ErrInvalidCredentials      = errors.New("invalid credentials")
	ErrInvalidToken            = errors.New("invalid token")
	ErrTokenExpired            = errors.New("token expired")
	ErrTokenRevoked            = errors.New("token revoked")
	ErrRefreshSuperseded       = errors.New("refresh token superseded")
	ErrSessionRecoveryRequired = errors.New("session recovery required")
)

type TokenType string

const (
	TokenTypeAccess  TokenType = "access"
	TokenTypeRefresh TokenType = "refresh"
)

// Claims represents the JWT claims for access and refresh tokens.
type Claims struct {
	UserID    string    `json:"user_id"`
	Username  string    `json:"username"`
	TokenType TokenType `json:"token_type"`
	SessionID string    `json:"sid"`
	jwt.RegisteredClaims
}

// TokenPair contains both access and refresh tokens
type TokenPair struct {
	AccessToken      string    `json:"accessToken"`
	RefreshToken     string    `json:"-"`
	ExpiresAt        time.Time `json:"expiresAt"`
	RefreshExpiresAt time.Time `json:"-"`
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

// authService implements AuthService.
type authService struct {
	jwtSecret          []byte
	accessTokenExpiry  time.Duration
	refreshTokenExpiry time.Duration
	users              map[string]string // username -> bcrypt password hash
	dummyPasswordHash  []byte
	fs                 filesystem.FS
	sessionsPath       string
	sessions           map[string]sessionRecord
	sessionsDirty      bool
	loginFailures      map[string]loginFailure
	mu                 sync.RWMutex
	stopCh             chan struct{}
	wg                 sync.WaitGroup
}

type loginFailure struct {
	count       int
	lockedUntil time.Time
}

// AuthServiceConfig holds configuration for the auth service.
type AuthServiceConfig struct {
	DataDir            string
	JWTSecret          string
	AccessTokenExpiry  time.Duration
	RefreshTokenExpiry time.Duration
	Users              map[string]string // username -> bcrypt password hash
}

// NewAuthService creates a new authentication service and loads its sessions.
func NewAuthService(fsys filesystem.FS, cfg AuthServiceConfig) (AuthService, error) {
	if cfg.DataDir == "" {
		cfg.DataDir = config.DefaultDataDir
	}
	if cfg.AccessTokenExpiry == 0 {
		cfg.AccessTokenExpiry = config.DefaultAccessTokenExpiry
	}
	if cfg.RefreshTokenExpiry == 0 {
		cfg.RefreshTokenExpiry = config.DefaultRefreshTokenExpiry
	}
	if cfg.Users == nil {
		cfg.Users = make(map[string]string)
	}

	var dummyHash []byte
	for _, configuredHash := range cfg.Users {
		dummyHash = []byte(configuredHash)
		break
	}
	if len(dummyHash) == 0 {
		dummyHash, _ = bcrypt.GenerateFromPassword([]byte("boxbox-invalid-user-password"), bcrypt.DefaultCost)
	}

	svc := &authService{
		jwtSecret:          []byte(cfg.JWTSecret),
		accessTokenExpiry:  cfg.AccessTokenExpiry,
		refreshTokenExpiry: cfg.RefreshTokenExpiry,
		users:              cfg.Users,
		dummyPasswordHash:  dummyHash,
		fs:                 fsys,
		sessionsPath:       filepath.Join(cfg.DataDir, config.AuthSessionsFileName),
		sessions:           make(map[string]sessionRecord),
		loginFailures:      make(map[string]loginFailure),
		stopCh:             make(chan struct{}),
	}
	if err := svc.loadSessions(); err != nil {
		return nil, err
	}
	return svc, nil
}

// Login authenticates a user and persists a new session before returning tokens.
func (s *authService) Login(ctx context.Context, username, password string) (*TokenPair, error) {
	if err := s.VerifyPassword(ctx, username, password); err != nil {
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.loginLocked(username)
}

// VerifyPassword rechecks an account without issuing a session, sharing login lockout.
func (s *authService) VerifyPassword(ctx context.Context, username, password string) error {
	storedHash, exists := s.users[username]
	configured := exists && strings.TrimSpace(username) != ""
	hash := []byte(storedHash)
	if !configured {
		hash = s.dummyPasswordHash
	}

	passwordMatches := bcrypt.CompareHashAndPassword(hash, []byte(password)) == nil
	if !configured {
		return ErrInvalidCredentials
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	failure := s.loginFailures[username]
	if time.Now().Before(failure.lockedUntil) {
		return ErrInvalidCredentials
	}
	if !passwordMatches {
		failure.count++
		if failure.count >= config.LoginLockoutThreshold {
			exponent := min(failure.count-config.LoginLockoutThreshold, 9)
			delay := config.LoginLockoutBaseDelay * time.Duration(1<<exponent)
			if delay > config.LoginLockoutMaxDelay {
				delay = config.LoginLockoutMaxDelay
			}
			failure.lockedUntil = time.Now().Add(delay)
		}
		s.loginFailures[username] = failure
		return ErrInvalidCredentials
	}
	delete(s.loginFailures, username)
	return nil
}

func (s *authService) loginLocked(username string) (*TokenPair, error) {
	sessionID := uuid.New().String()
	for {
		if _, exists := s.sessions[sessionID]; !exists {
			break
		}
		sessionID = uuid.New().String()
	}
	pair, err := s.generateTokenPair(username, sessionID)
	if err != nil {
		return nil, err
	}
	record := sessionRecord{
		Username:    username,
		RefreshHash: sessionRefreshHash(pair.RefreshToken),
		ExpiresAt:   later(pair.ExpiresAt, pair.RefreshExpiresAt),
	}
	s.sessions[sessionID] = record
	result, persistErr := s.persistSessionsLocked()
	if persistErr != nil && !result.published {
		delete(s.sessions, sessionID)
		return nil, persistErr
	}
	if persistErr != nil {
		s.sessionsDirty = true
		log.Error().Err(persistErr).Msg("Could not durably persist auth session login")
		return nil, errors.Join(ErrSessionRecoveryRequired, persistErr)
	}
	s.sessionsDirty = false
	return pair, nil
}

// Refresh rotates the current refresh token without changing its session.
func (s *authService) Refresh(ctx context.Context, refreshToken string) (*TokenPair, error) {
	claims, err := s.parseToken(refreshToken)
	if err != nil {
		return nil, err
	}
	if err := validateClaims(claims, TokenTypeRefresh, s.users); err != nil {
		return nil, err
	}

	tokenHash := sessionRefreshHash(refreshToken)
	s.mu.Lock()
	defer s.mu.Unlock()

	previous := s.sessions[claims.SessionID]
	if err := s.validateSessionLocked(claims, TokenTypeRefresh, tokenHash); err != nil {
		return nil, err
	}
	pair, err := s.generateTokenPair(claims.Username, claims.SessionID)
	if err != nil {
		return nil, err
	}
	record := sessionRecord{
		Username:    previous.Username,
		RefreshHash: sessionRefreshHash(pair.RefreshToken),
		ExpiresAt:   later(previous.ExpiresAt, later(pair.ExpiresAt, pair.RefreshExpiresAt)),
	}
	s.sessions[claims.SessionID] = record
	result, persistErr := s.persistSessionsLocked()
	if persistErr != nil && !result.published {
		s.sessions[claims.SessionID] = previous
		return nil, persistErr
	}
	if persistErr != nil {
		s.sessionsDirty = true
		log.Error().Err(persistErr).Msg("Could not durably persist auth session refresh")
		return nil, errors.Join(ErrSessionRecoveryRequired, persistErr)
	}
	s.sessionsDirty = false
	return pair, nil
}

// ValidateToken accepts either token type only when its session remains active.
func (s *authService) ValidateToken(tokenString string) (*Claims, error) {
	return s.validateToken(tokenString, "")
}

func (s *authService) ValidateAccessToken(tokenString string) (*Claims, error) {
	return s.validateToken(tokenString, TokenTypeAccess)
}

func (s *authService) ValidateRefreshToken(tokenString string) (*Claims, error) {
	return s.validateToken(tokenString, TokenTypeRefresh)
}

func (s *authService) validateToken(tokenString string, expectedType TokenType) (*Claims, error) {
	claims, err := s.parseToken(tokenString)
	if err != nil {
		return nil, err
	}
	if err := validateClaims(claims, expectedType, s.users); err != nil {
		return nil, err
	}

	s.mu.RLock()
	var tokenHash string
	if claims.TokenType == TokenTypeRefresh {
		tokenHash = sessionRefreshHash(tokenString)
	}
	err = s.validateSessionLocked(claims, claims.TokenType, tokenHash)
	s.mu.RUnlock()
	if err != nil {
		return nil, err
	}
	return claims, nil
}

func (s *authService) parseToken(tokenString string) (*Claims, error) {
	token, err := jwt.ParseWithClaims(tokenString, &Claims{}, func(token *jwt.Token) (interface{}, error) {
		if token.Method != jwt.SigningMethodHS256 {
			return nil, ErrInvalidToken
		}
		return s.jwtSecret, nil
	},
		jwt.WithValidMethods([]string{jwt.SigningMethodHS256.Alg()}),
		jwt.WithIssuer("boxbox"),
		jwt.WithExpirationRequired(),
	)
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
	return claims, nil
}

func validateClaims(claims *Claims, expectedType TokenType, users map[string]string) error {
	if claims.TokenType != TokenTypeAccess && claims.TokenType != TokenTypeRefresh {
		return ErrInvalidToken
	}
	if expectedType != "" && claims.TokenType != expectedType {
		return ErrInvalidToken
	}
	if !validUUID(claims.SessionID) || strings.TrimSpace(claims.ID) == "" ||
		strings.TrimSpace(claims.Username) == "" || claims.Subject != claims.Username ||
		claims.UserID != generateUserID(claims.Username) {
		return ErrInvalidToken
	}
	if _, configured := users[claims.Username]; !configured {
		return ErrInvalidToken
	}
	return nil
}

func (s *authService) validateSessionLocked(claims *Claims, tokenType TokenType, tokenHash string) error {
	record, exists := s.sessions[claims.SessionID]
	if !exists || !record.ExpiresAt.After(time.Now()) {
		return ErrTokenRevoked
	}
	if record.Username != claims.Username {
		return ErrInvalidToken
	}
	if tokenType == TokenTypeRefresh && record.RefreshHash != tokenHash {
		return ErrRefreshSuperseded
	}
	return nil
}

func validUUID(value string) bool {
	if len(value) != 36 {
		return false
	}
	for index, char := range value {
		switch index {
		case 8, 13, 18, 23:
			if char != '-' {
				return false
			}
		default:
			if char < '0' || char > '9' {
				if char < 'a' || char > 'f' {
					return false
				}
			}
		}
	}
	return true
}

func later(first, second time.Time) time.Time {
	if first.After(second) {
		return first
	}
	return second
}

// Logout removes the session only when presented its current refresh token.
func (s *authService) Logout(ctx context.Context, refreshToken string) error {
	claims, err := s.parseToken(refreshToken)
	if err != nil {
		return err
	}
	if err := validateClaims(claims, TokenTypeRefresh, s.users); err != nil {
		return err
	}

	tokenHash := sessionRefreshHash(refreshToken)
	s.mu.Lock()
	defer s.mu.Unlock()

	previous := s.sessions[claims.SessionID]
	if err := s.validateSessionLocked(claims, TokenTypeRefresh, tokenHash); err != nil {
		return err
	}
	delete(s.sessions, claims.SessionID)
	result, persistErr := s.persistSessionsLocked()
	if persistErr != nil && !result.published {
		s.sessions[claims.SessionID] = previous
		return persistErr
	}
	if persistErr != nil {
		s.sessionsDirty = true
		log.Error().Err(persistErr).Msg("Could not durably persist auth session logout")
		return errors.Join(ErrSessionRecoveryRequired, persistErr)
	}
	s.sessionsDirty = false
	return nil
}

// generateTokenPair creates a pair of distinct tokens bound to one session.
func (s *authService) generateTokenPair(username, sessionID string) (*TokenPair, error) {
	now := time.Now()
	userID := generateUserID(username)

	accessExpiry := now.Add(s.accessTokenExpiry)
	accessClaims := &Claims{
		UserID:    userID,
		Username:  username,
		TokenType: TokenTypeAccess,
		SessionID: sessionID,
		RegisteredClaims: jwt.RegisteredClaims{
			ID:        generateTokenID(),
			ExpiresAt: jwt.NewNumericDate(accessExpiry),
			IssuedAt:  jwt.NewNumericDate(now),
			NotBefore: jwt.NewNumericDate(now),
			Issuer:    "boxbox",
			Subject:   username,
		},
	}
	accessToken := jwt.NewWithClaims(jwt.SigningMethodHS256, accessClaims)
	accessTokenString, err := accessToken.SignedString(s.jwtSecret)
	if err != nil {
		return nil, err
	}

	refreshExpiry := now.Add(s.refreshTokenExpiry)
	refreshClaims := &Claims{
		UserID:    userID,
		Username:  username,
		TokenType: TokenTypeRefresh,
		SessionID: sessionID,
		RegisteredClaims: jwt.RegisteredClaims{
			ID:        generateTokenID(),
			ExpiresAt: jwt.NewNumericDate(refreshExpiry),
			IssuedAt:  jwt.NewNumericDate(now),
			NotBefore: jwt.NewNumericDate(now),
			Issuer:    "boxbox",
			Subject:   username,
		},
	}
	refreshToken := jwt.NewWithClaims(jwt.SigningMethodHS256, refreshClaims)
	refreshTokenString, err := refreshToken.SignedString(s.jwtSecret)
	if err != nil {
		return nil, err
	}

	return &TokenPair{
		AccessToken:      accessTokenString,
		RefreshToken:     refreshTokenString,
		ExpiresAt:        accessExpiry,
		RefreshExpiresAt: refreshExpiry,
	}, nil
}

// cleanupExpiredSessions removes sessions after their longest issued token expires.
func (s *authService) cleanupExpiredSessions() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	now := time.Now()
	before := len(s.sessions)
	maps.DeleteFunc(s.sessions, func(_ string, session sessionRecord) bool {
		return !session.ExpiresAt.After(now)
	})
	maps.DeleteFunc(s.loginFailures, func(_ string, failure loginFailure) bool {
		return failure.count == 0 || now.Sub(failure.lockedUntil) > config.LoginLockoutMaxDelay
	})
	if before == len(s.sessions) && !s.sessionsDirty {
		return nil
	}

	result, err := s.persistSessionsLocked()
	if err != nil {
		s.sessionsDirty = true
		if result.published {
			log.Error().Err(err).Msg("Could not durably persist auth session cleanup")
			return errors.Join(ErrSessionRecoveryRequired, err)
		}
		return err
	}
	s.sessionsDirty = false
	return nil
}

// generateUserID creates a stable, non-plaintext user ID for token claims.
func generateUserID(username string) string {
	sum := sha256.Sum256([]byte(username))
	return hex.EncodeToString(sum[:8])
}

func generateTokenID() string {
	return uuid.New().String()
}

// StartCleanup starts the periodic cleanup of expired sessions.
func (s *authService) StartCleanup(ctx context.Context) {
	s.wg.Go(func() {
		ticker := time.NewTicker(config.TokenCleanupInterval)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				return
			case <-s.stopCh:
				return
			case <-ticker.C:
				if err := s.cleanupExpiredSessions(); err != nil {
					log.Warn().Err(err).Msg("Could not persist auth session cleanup")
				}
			}
		}
	})
}

// StopCleanup stops the cleanup goroutine
func (s *authService) StopCleanup() {
	close(s.stopCh)
	s.wg.Wait()
}
