package config

import (
	"bytes"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

// The API decodes exactly what the supervisor loaded, so it never needs to
// read config.yaml itself.
func TestEncodedConfigurationRoundTrips(t *testing.T) {
	clearConfigEnv(t)
	configPath := filepath.Join(t.TempDir(), "config.yaml")
	content := []byte(`mount_points:
  - name: tank
    path: /tmp
    read_only: true
    auto_discover: true
allowed_origins: [https://files.example.com]
trusted_proxies: [10.0.0.0/8]
nas:
  identities:
    alice: { uid: 3000, gid: 3000, groups: [3001, 3002] }
`)
	if err := os.WriteFile(configPath, content, 0o600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("BOXBOX_JWT_SECRET", "unit-test-secret-at-least-thirty-two-bytes")
	t.Setenv("BOXBOX_USERS_alice", testHashA)
	t.Setenv("FM_PORT", "9090")

	loaded, err := LoadWithReport(configPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(loaded.Warnings) == 0 {
		t.Fatal("fixture should produce a migration warning")
	}
	var wire bytes.Buffer
	if err := loaded.Encode(&wire); err != nil {
		t.Fatal(err)
	}
	// The file may be unreadable to the API; decoding must not touch it.
	if err := os.Chmod(configPath, 0); err != nil {
		t.Fatal(err)
	}
	decoded, err := Decode(&wire)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, loaded) {
		t.Fatalf("decoded = %#v\nwant %#v", decoded.Config, loaded.Config)
	}
}

func TestDecodeRejectsAnEmptyHandoff(t *testing.T) {
	if _, err := Decode(bytes.NewReader(nil)); err == nil {
		t.Fatal("Decode of nothing succeeded")
	}
}
