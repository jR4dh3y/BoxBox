package service

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"fmt"
	"maps"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"golang.org/x/time/rate"
)

const (
	ClientCredentialPrefix = "boxbox_client_"
	pairingLifetime        = 5 * time.Minute
	clientLifetime         = 30 * 24 * time.Hour
	pairingMaxAttempts     = 10
	maxConnectClients      = 1000
	pairingAlphabet        = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ"
)

var (
	ErrPairingUnavailable    = errors.New("pairing code invalid or unavailable")
	ErrConnectRateLimited    = errors.New("connect request rate limited")
	ErrPairingInUse          = errors.New("pairing is already enabled by another account")
	ErrInvalidClientName     = errors.New("invalid client name")
	ErrConnectClientNotFound = errors.New("connected client not found")
	ErrConnectClientLimit    = errors.New("connected client limit reached")
	ErrConnectStorage        = errors.New("connected client storage unavailable")
)

type ConnectServiceConfig struct {
	DataDir string
	Users   map[string]string
}

type ConnectClient struct {
	ClientID   string    `json:"clientId"`
	ClientName string    `json:"clientName"`
	Username   string    `json:"username"`
	CreatedAt  time.Time `json:"createdAt"`
	ExpiresAt  time.Time `json:"expiresAt"`
}

type ConnectRedemption struct {
	ClientID   string    `json:"clientId"`
	Credential string    `json:"credential"`
	ExpiresAt  time.Time `json:"expiresAt"`
	Username   string    `json:"username"`
}

type PairingCode struct {
	Code      string    `json:"code"`
	ExpiresAt time.Time `json:"expiresAt"`
}

type PairingStatus struct {
	Enabled   bool       `json:"enabled"`
	ExpiresAt *time.Time `json:"expiresAt,omitempty"`
}

type connectRecord struct {
	ConnectClient
	CredentialHash string `json:"credentialHash"`
}

type pairingWindow struct {
	username  string
	hash      [sha256.Size]byte
	expiresAt time.Time
	attempts  int
}

// ConnectService owns one process-local pairing window and durable client digests.
// As with auth sessions, only one server process may own its storage directory.
type ConnectService struct {
	mu          sync.Mutex
	fs          filesystem.FS
	path        string
	users       map[string]string
	passwords   PasswordVerifier
	clients     map[string]connectRecord
	window      *pairingWindow
	redemptions *rate.Limiter
	storeFailed bool
	now         func() time.Time
}

func NewConnectService(fsys filesystem.FS, passwords PasswordVerifier, cfg ConnectServiceConfig) (*ConnectService, error) {
	if cfg.DataDir == "" {
		cfg.DataDir = config.DefaultDataDir
	}
	s := &ConnectService{
		fs: fsys, path: filepath.Join(cfg.DataDir, "connect", "clients.json"),
		users: maps.Clone(cfg.Users), passwords: passwords,
		clients: make(map[string]connectRecord), now: time.Now,
		redemptions: rate.NewLimiter(1, 5),
	}
	if err := s.loadClients(); err != nil {
		return nil, fmt.Errorf("initialize connected clients: %w", err)
	}
	return s, nil
}

func (s *ConnectService) EnablePairing(ctx context.Context, username, password string) (*PairingCode, error) {
	if !s.accountExists(username) {
		return nil, ErrInvalidCredentials
	}
	if err := s.passwords.VerifyPassword(ctx, username, password); err != nil {
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.storeFailed {
		return nil, ErrConnectStorage
	}
	now := s.now()
	if s.activeWindow(now) && s.window.username != username {
		return nil, ErrPairingInUse
	}
	var random [8]byte
	if _, err := rand.Read(random[:]); err != nil {
		return nil, err
	}
	var code strings.Builder
	for _, value := range random {
		code.WriteByte(pairingAlphabet[int(value)%len(pairingAlphabet)])
	}
	plain := code.String()
	expires := now.Add(pairingLifetime)
	s.window = &pairingWindow{username: username, hash: sha256.Sum256([]byte(plain)), expiresAt: expires}
	return &PairingCode{Code: plain[:4] + "-" + plain[4:], ExpiresAt: expires}, nil
}

func (s *ConnectService) PairingStatus(username string) PairingStatus {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.storeFailed || !s.activeWindow(s.now()) || s.window.username != username {
		return PairingStatus{}
	}
	expires := s.window.expiresAt
	return PairingStatus{Enabled: true, ExpiresAt: &expires}
}

func (s *ConnectService) DisablePairing(username string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.window != nil && s.window.username == username {
		s.window = nil
	}
}

func (s *ConnectService) activeWindow(now time.Time) bool {
	if s.window != nil && (!now.Before(s.window.expiresAt) || s.window.attempts >= pairingMaxAttempts) {
		s.window = nil
	}
	return s.window != nil
}

func normalizePairingCode(code string) string {
	code = strings.ToUpper(strings.TrimSpace(code))
	if len(code) == 9 && code[4] == '-' {
		code = code[:4] + code[5:]
	}
	return code
}

func validClientName(name string) bool {
	return utf8.ValidString(name) && utf8.RuneCountInString(name) >= 1 && utf8.RuneCountInString(name) <= 80 &&
		!strings.ContainsFunc(name, unicode.IsControl)
}

func (s *ConnectService) Redeem(code, clientName string) (*ConnectRedemption, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	now := s.now()
	if !s.redemptions.AllowN(now, 1) {
		return nil, ErrConnectRateLimited
	}
	if s.storeFailed {
		return nil, ErrConnectStorage
	}
	if !s.activeWindow(now) {
		return nil, ErrPairingUnavailable
	}
	s.window.attempts++
	hash := sha256.Sum256([]byte(normalizePairingCode(code)))
	if subtle.ConstantTimeCompare(hash[:], s.window.hash[:]) != 1 {
		if s.window.attempts >= pairingMaxAttempts {
			s.window = nil
		}
		return nil, ErrPairingUnavailable
	}
	clientName = strings.TrimSpace(clientName)
	if !validClientName(clientName) {
		return nil, ErrInvalidClientName
	}
	username := s.window.username
	if !s.accountExists(username) {
		s.window = nil
		return nil, ErrPairingUnavailable
	}
	previous := maps.Clone(s.clients)
	for id, record := range s.clients {
		if !now.Before(record.ExpiresAt) || !s.accountExists(record.Username) {
			delete(s.clients, id)
		}
	}
	if len(s.clients) >= maxConnectClients {
		return nil, ErrConnectClientLimit
	}
	var random [32]byte
	if _, err := rand.Read(random[:]); err != nil {
		return nil, err
	}
	credential := ClientCredentialPrefix + base64.RawURLEncoding.EncodeToString(random[:])
	id := rand.Text()
	client := ConnectClient{ClientID: id, ClientName: clientName, Username: username, CreatedAt: now, ExpiresAt: now.Add(clientLifetime)}
	s.clients[id] = connectRecord{ConnectClient: client, CredentialHash: connectCredentialHash(credential)}
	// Consume before storage publication. A failed request can never mint twice.
	s.window = nil
	if err := s.saveClients(previous); err != nil {
		return nil, err
	}
	return &ConnectRedemption{ClientID: id, Credential: credential, ExpiresAt: client.ExpiresAt, Username: username}, nil
}

func (s *ConnectService) ValidateClientCredential(credential string) (*ConnectClient, error) {
	if !strings.HasPrefix(credential, ClientCredentialPrefix) {
		return nil, ErrInvalidToken
	}
	random, err := base64.RawURLEncoding.DecodeString(strings.TrimPrefix(credential, ClientCredentialPrefix))
	if err != nil || len(random) != 32 || ClientCredentialPrefix+base64.RawURLEncoding.EncodeToString(random) != credential {
		return nil, ErrInvalidToken
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.storeFailed {
		return nil, ErrConnectStorage
	}
	hash := connectCredentialHash(credential)
	for _, record := range s.clients {
		if subtle.ConstantTimeCompare([]byte(hash), []byte(record.CredentialHash)) == 1 && s.accountExists(record.Username) && s.now().Before(record.ExpiresAt) {
			client := record.ConnectClient
			return &client, nil
		}
	}
	return nil, ErrInvalidToken
}

func (s *ConnectService) ListClients(username string) ([]ConnectClient, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.storeFailed {
		return nil, ErrConnectStorage
	}
	clients := make([]ConnectClient, 0)
	for _, record := range s.clients {
		if record.Username == username && s.accountExists(username) && s.now().Before(record.ExpiresAt) {
			clients = append(clients, record.ConnectClient)
		}
	}
	slices.SortFunc(clients, func(a, b ConnectClient) int { return strings.Compare(a.ClientID, b.ClientID) })
	return clients, nil
}

func (s *ConnectService) RevokeClient(username, id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.storeFailed {
		return ErrConnectStorage
	}
	record, ok := s.clients[id]
	if !ok || record.Username != username {
		return ErrConnectClientNotFound
	}
	previous := maps.Clone(s.clients)
	delete(s.clients, id)
	return s.saveClients(previous)
}

func (s *ConnectService) accountExists(username string) bool {
	_, ok := s.users[username]
	return ok && strings.TrimSpace(username) != ""
}

func connectCredentialHash(credential string) string {
	hash := sha256.Sum256([]byte(credential))
	return hex.EncodeToString(hash[:])
}
