package service

import (
	"errors"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestConnectLargeAcceptedSnapshotSurvivesRestart(t *testing.T) {
	fsys := filesystem.NewOsFS()
	_, auth, cfg := connectFixture(t, fsys, t.TempDir())
	username := strings.Repeat("u", 512)
	cfg.Users[username] = cfg.Users["alice"]
	s, err := NewConnectService(fsys, auth, cfg)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now()
	s.now = func() time.Time { return now }
	name := strings.Repeat("<", 80)
	clients := make([]*ConnectRedemption, 0, maxConnectClients)
	for range maxConnectClients {
		code := enableTestPairing(t, s, username)
		client, err := s.Redeem(code.Code, name)
		if err != nil {
			t.Fatal("redeem accepted client", err)
		}
		clients = append(clients, client)
		now = now.Add(time.Second)
	}
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	if len(data) <= 1<<20 || !strings.Contains(string(data), strings.Repeat(`\u003c`, 80)) {
		t.Fatal("fixture must exceed the former read limit with JSON-escaped names")
	}
	t.Logf("persisted %d clients in %d bytes", len(clients), len(data))
	code := enableTestPairing(t, s, username)
	if _, err := s.Redeem(code.Code, name); !errors.Is(err, ErrConnectClientLimit) {
		t.Fatal("client count limit was not enforced")
	}
	s, err = NewConnectService(fsys, auth, cfg)
	if err != nil {
		t.Fatal("restart with successfully persisted clients", err)
	}
	listed, err := s.ListClients(username)
	if err != nil || len(listed) != len(clients) {
		t.Fatal("restart lost clients")
	}
	for _, client := range clients {
		restored, err := s.ValidateClientCredential(client.Credential)
		if err != nil {
			t.Fatal("persisted credential rejected after restart", err)
		}
		if restored.ClientID != client.ClientID || restored.ClientName != name || restored.Username != username {
			t.Fatal("restart changed client metadata")
		}
	}
}

func TestConnectRejectsTrailingDataBeyondFormerReadLimit(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	s, auth, cfg := connectFixture(t, fsys, "/pairing")
	data := `{"version":1,"clients":{}}` + strings.Repeat(" ", 1<<20) + `{}`
	if err := fsys.WriteFile(s.path, []byte(data), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := NewConnectService(fsys, auth, cfg); err == nil {
		t.Fatal("trailing data beyond the former read limit accepted")
	}
}
