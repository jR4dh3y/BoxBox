package service

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"golang.org/x/crypto/bcrypt"
)

func connectFixture(t *testing.T, fsys filesystem.FS, dir string) (*ConnectService, AuthService, ConnectServiceConfig) {
	t.Helper()
	hash, err := bcrypt.GenerateFromPassword([]byte("pairing-test-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal("hash password", err)
	}
	cfg := ConnectServiceConfig{DataDir: dir, Users: map[string]string{"alice": string(hash), "bob": string(hash)}}
	auth, err := NewAuthService(fsys, AuthServiceConfig{DataDir: dir, JWTSecret: "pairing-test-secret", Users: cfg.Users})
	if err != nil {
		t.Fatal("create auth", err)
	}
	s, err := NewConnectService(fsys, auth, cfg)
	if err != nil {
		t.Fatal("create connect", err)
	}
	return s, auth, cfg
}

func enableTestPairing(t *testing.T, s *ConnectService, username string) *PairingCode {
	t.Helper()
	code, err := s.EnablePairing(context.Background(), username, "pairing-test-password")
	if err != nil {
		t.Fatal("enable pairing", err)
	}
	return code
}

func redeemTestClient(t *testing.T, s *ConnectService, username string) *ConnectRedemption {
	t.Helper()
	code := enableTestPairing(t, s, username)
	client, err := s.Redeem(code.Code, "Test device")
	if err != nil {
		t.Fatal("redeem pairing", err)
	}
	return client
}

func TestConnectConcurrentRedemptionHasOneWinner(t *testing.T) {
	s, _, _ := connectFixture(t, filesystem.NewMemMapFS(), "/pairing")
	code := enableTestPairing(t, s, "alice")
	if len(pairingAlphabet) != 32 {
		t.Fatal("code alphabet must evenly divide random bytes")
	}
	if len(code.Code) != 9 || code.Code[4] != '-' {
		t.Fatal("invalid display format")
	}
	var wins atomic.Int32
	var wg sync.WaitGroup
	for range 40 {
		wg.Go(func() {
			client, err := s.Redeem(code.Code, "Laptop")
			if err == nil {
				wins.Add(1)
				if _, err := s.ValidateClientCredential(client.Credential); err != nil {
					t.Error("winning credential rejected")
				}
			} else if !errors.Is(err, ErrPairingUnavailable) && !errors.Is(err, ErrConnectRateLimited) {
				t.Error("unexpected redemption error")
			}
		})
	}
	wg.Wait()
	if wins.Load() != 1 {
		t.Fatalf("redemption winners: %d", wins.Load())
	}
	if s.PairingStatus("alice").Enabled {
		t.Fatal("successful redemption left pairing enabled")
	}
}

func TestConnectPasswordWindowOwnershipAndExpiry(t *testing.T) {
	s, _, _ := connectFixture(t, filesystem.NewMemMapFS(), "/pairing")
	if _, err := s.EnablePairing(context.Background(), "alice", "wrong"); !errors.Is(err, ErrInvalidCredentials) {
		t.Fatal("wrong password accepted")
	}
	if _, err := s.EnablePairing(context.Background(), "missing", "pairing-test-password"); !errors.Is(err, ErrInvalidCredentials) {
		t.Fatal("unknown user accepted")
	}
	code := enableTestPairing(t, s, "alice")
	if s.PairingStatus("bob").Enabled {
		t.Fatal("another account can inspect window")
	}
	if _, err := s.EnablePairing(context.Background(), "bob", "pairing-test-password"); !errors.Is(err, ErrPairingInUse) {
		t.Fatal("another account replaced window")
	}
	s.DisablePairing("bob")
	if !s.PairingStatus("alice").Enabled {
		t.Fatal("another account disabled window")
	}
	s.now = func() time.Time { return code.ExpiresAt }
	if _, err := s.Redeem(code.Code, "Laptop"); !errors.Is(err, ErrPairingUnavailable) {
		t.Fatal("expired code accepted")
	}
	if s.PairingStatus("alice").Enabled {
		t.Fatal("expired window enabled")
	}
	code = enableTestPairing(t, s, "alice")
	s.DisablePairing("alice")
	if _, err := s.Redeem(code.Code, "Laptop"); !errors.Is(err, ErrPairingUnavailable) {
		t.Fatal("disabled code accepted")
	}
}

func TestConnectGlobalAttemptBudgetAndRateLimit(t *testing.T) {
	s, _, _ := connectFixture(t, filesystem.NewMemMapFS(), "/pairing")
	code := enableTestPairing(t, s, "alice")
	now := time.Now()
	s.now = func() time.Time { return now }
	for range pairingMaxAttempts {
		if _, err := s.Redeem("wrong", "Laptop"); !errors.Is(err, ErrPairingUnavailable) {
			t.Fatal("unexpected attempt result")
		}
		now = now.Add(time.Second)
	}
	if _, err := s.Redeem(code.Code, "Laptop"); !errors.Is(err, ErrPairingUnavailable) {
		t.Fatal("exhausted code accepted")
	}
	code = enableTestPairing(t, s, "alice")
	for range 4 {
		_, _ = s.Redeem("wrong", "Laptop")
	}
	if _, err := s.Redeem(code.Code, "Laptop"); !errors.Is(err, ErrConnectRateLimited) {
		t.Fatal("global rate limit bypassed")
	}
	// Reopening cannot reset the global rate bucket.
	code = enableTestPairing(t, s, "alice")
	if _, err := s.Redeem(code.Code, "Laptop"); !errors.Is(err, ErrConnectRateLimited) {
		t.Fatal("window reset rate limiter")
	}
	now = now.Add(time.Second)
	if _, err := s.Redeem(strings.ToLower(code.Code), "Laptop"); err != nil {
		t.Fatal("code rejected after rate recovery", err)
	}
}

func TestConnectPersistenceRestartRevokeAndAccountRemoval(t *testing.T) {
	dir := t.TempDir()
	fsys := filesystem.NewOsFS()
	s, auth, cfg := connectFixture(t, fsys, dir)
	alice := redeemTestClient(t, s, "alice")
	bob := redeemTestClient(t, s, "bob")
	pending := enableTestPairing(t, s, "alice")
	for path, want := range map[string]os.FileMode{filepath.Dir(s.path): 0o700, s.path: 0o600} {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != want {
			t.Fatal("store permissions incorrect")
		}
	}
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(data), alice.Credential) || strings.Contains(string(data), bob.Credential) || strings.Contains(string(data), pending.Code) {
		t.Fatal("store contains a plaintext secret")
	}
	s, err = NewConnectService(fsys, auth, cfg)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.Redeem(pending.Code, "New device"); !errors.Is(err, ErrPairingUnavailable) {
		t.Fatal("window survived restart")
	}
	if _, err := s.ValidateClientCredential(alice.Credential); err != nil {
		t.Fatal("credential did not survive restart")
	}
	clients, err := s.ListClients("alice")
	if err != nil || len(clients) != 1 || clients[0].ClientID != alice.ClientID {
		t.Fatal("list crosses account boundary")
	}
	if err := s.RevokeClient("bob", alice.ClientID); !errors.Is(err, ErrConnectClientNotFound) {
		t.Fatal("cross-account revoke accepted")
	}
	if err := s.RevokeClient("alice", alice.ClientID); err != nil {
		t.Fatal(err)
	}
	s, err = NewConnectService(fsys, auth, cfg)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.ValidateClientCredential(alice.Credential); !errors.Is(err, ErrInvalidToken) {
		t.Fatal("revocation lost after restart")
	}
	if _, err := s.ValidateClientCredential(bob.Credential); err != nil {
		t.Fatal("unrelated credential revoked")
	}
	s.now = func() time.Time { return bob.ExpiresAt }
	if _, err := s.ValidateClientCredential(bob.Credential); !errors.Is(err, ErrInvalidToken) {
		t.Fatal("expired credential accepted")
	}
	delete(cfg.Users, "bob")
	s, err = NewConnectService(fsys, auth, cfg)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.ValidateClientCredential(bob.Credential); !errors.Is(err, ErrInvalidToken) {
		t.Fatal("removed account accepted")
	}
}

func TestConnectPasswordVerificationSharesLoginLockoutWithoutSession(t *testing.T) {
	s, auth, _ := connectFixture(t, filesystem.NewMemMapFS(), "/pairing")
	actual := auth.(*authService)
	for range config.LoginLockoutThreshold {
		_, _ = s.EnablePairing(context.Background(), "alice", "wrong")
	}
	if _, err := auth.Login(context.Background(), "alice", "pairing-test-password"); !errors.Is(err, ErrInvalidCredentials) {
		t.Fatal("pairing bypassed login lockout")
	}
	if len(actual.sessions) != 0 {
		t.Fatal("password re-entry created sessions")
	}
}

func TestConnectRejectsCorruptAndUnsafeStore(t *testing.T) {
	for _, value := range []string{`{`, `{"version":2,"clients":{}}`, `{"version":1,"clients":null}`, `{"version":1,"clients":{}} {}`, `{"version":1,"clients":{},"unknown":true}`, `{"version":1,"clients":{"x":{}}}`} {
		t.Run(value, func(t *testing.T) {
			fsys := filesystem.NewMemMapFS()
			s, auth, cfg := connectFixture(t, fsys, "/pairing")
			if err := fsys.WriteFile(s.path, []byte(value), 0o600); err != nil {
				t.Fatal(err)
			}
			if _, err := NewConnectService(fsys, auth, cfg); err == nil {
				t.Fatal("corrupt store accepted")
			}
		})
	}
	for _, target := range []string{"file", "directory"} {
		t.Run("symlink-"+target, func(t *testing.T) {
			dir := t.TempDir()
			fsys := filesystem.NewOsFS()
			s, auth, cfg := connectFixture(t, fsys, dir)
			link := s.path
			if target == "directory" {
				link = filepath.Dir(s.path)
			}
			moved := link + ".original"
			if err := os.Rename(link, moved); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(moved, link); err != nil {
				t.Fatal(err)
			}
			if _, err := NewConnectService(fsys, auth, cfg); err == nil {
				t.Fatal("symlink store accepted")
			}
		})
	}
}

func TestConnectCodeAndCredentialNeverAppearInStatusOrList(t *testing.T) {
	s, _, _ := connectFixture(t, filesystem.NewMemMapFS(), "/pairing")
	code := enableTestPairing(t, s, "alice")
	status, _ := json.Marshal(s.PairingStatus("alice"))
	if strings.Contains(string(status), "code") || strings.Contains(string(status), code.Code) {
		t.Fatal("status discloses code")
	}
	for _, name := range []string{"", "\n", strings.Repeat("a", 81), "device\x00", string([]byte{0xff})} {
		if validClientName(strings.TrimSpace(name)) {
			t.Fatal("invalid name accepted")
		}
	}
	client, err := s.Redeem(code.Code, "  Laptop  ")
	if err != nil {
		t.Fatal(err)
	}
	clients, err := s.ListClients("alice")
	if err != nil || len(clients) != 1 || clients[0].ClientName != "Laptop" {
		t.Fatal("invalid client metadata")
	}
	list, _ := json.Marshal(clients)
	if strings.Contains(string(list), "credential") || strings.Contains(string(list), client.Credential) {
		t.Fatal("list discloses credential")
	}
}

func TestConnectPersistenceFailures(t *testing.T) {
	stages := []authPersistenceFailureStage{
		authPersistenceFailCreate, authPersistenceFailChmod, authPersistenceFailWrite,
		authPersistenceFailFileSync, authPersistenceFailFileClose, authPersistenceFailRename,
		authPersistenceFailDirectoryOpen, authPersistenceFailDirectorySync, authPersistenceFailDirectoryClose,
	}
	for _, operation := range []string{"redeem", "revoke"} {
		for _, stage := range stages {
			t.Run(operation+"/"+string(stage), func(t *testing.T) {
				fsys := &authPersistenceFaultFS{FS: filesystem.NewOsFS()}
				s, auth, cfg := connectFixture(t, fsys, t.TempDir())
				existing := redeemTestClient(t, s, "alice")
				code := enableTestPairing(t, s, "alice")
				fsys.storePath = s.path
				fsys.stage = stage
				var err error
				if operation == "redeem" {
					var result *ConnectRedemption
					result, err = s.Redeem(code.Code, "New device")
					if result != nil {
						t.Fatal("credential returned without durable storage")
					}
					if s.PairingStatus("alice").Enabled {
						t.Fatal("failed redemption left code reusable")
					}
				} else {
					err = s.RevokeClient("alice", existing.ClientID)
				}
				if !errors.Is(err, ErrConnectStorage) {
					t.Fatal("storage failure not reported")
				}
				published := stage == authPersistenceFailDirectoryOpen || stage == authPersistenceFailDirectorySync || stage == authPersistenceFailDirectoryClose
				_, validationErr := s.ValidateClientCredential(existing.Credential)
				if published {
					if !errors.Is(validationErr, ErrConnectStorage) {
						t.Fatal("uncertain publication did not fail closed")
					}
					if _, err := s.ListClients("alice"); !errors.Is(err, ErrConnectStorage) {
						t.Fatal("uncertain publication allowed client administration")
					}
				} else if validationErr != nil {
					t.Fatal("unpublished mutation was not rolled back")
				}
				fsys.stage = ""
				restarted, err := NewConnectService(fsys, auth, cfg)
				if err != nil {
					t.Fatal("restart after persistence failure", err)
				}
				_, validationErr = restarted.ValidateClientCredential(existing.Credential)
				if operation == "revoke" && published {
					if !errors.Is(validationErr, ErrInvalidToken) {
						t.Fatal("published revocation lost after restart")
					}
				} else if validationErr != nil {
					t.Fatal("unrelated credential lost after restart")
				}
				entries, err := fsys.ReadDir(filepath.Dir(s.path))
				if err != nil || len(entries) != 1 || entries[0].Name() != "clients.json" {
					t.Fatal("failed publication left temporary snapshots")
				}
			})
		}
	}
}

func TestConnectRejectsMalformedCredentials(t *testing.T) {
	s, _, _ := connectFixture(t, filesystem.NewMemMapFS(), "/pairing")
	client := redeemTestClient(t, s, "alice")
	for _, value := range []string{"", "unrelated", ClientCredentialPrefix, client.Credential + "=", client.Credential + "x", strings.ToUpper(client.Credential)} {
		if _, err := s.ValidateClientCredential(value); !errors.Is(err, ErrInvalidToken) {
			t.Fatal("malformed credential accepted")
		}
	}
}
