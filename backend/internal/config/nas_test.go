package config

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func TestLoadReadsNASIdentitiesFromFileAndEnvironment(t *testing.T) {
	clearConfigEnv(t)
	configPath := filepath.Join(t.TempDir(), "config.yaml")
	content := []byte(`mount_points:
  - name: tank
    path: /tmp
nas:
  identities:
    alice:
      uid: 3000
      gid: 3000
      groups: [3001]
`)
	if err := os.WriteFile(configPath, content, 0o600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("BOXBOX_JWT_SECRET", "unit-test-secret-at-least-thirty-two-bytes")
	t.Setenv("BOXBOX_USERS_alice", testHashA)
	t.Setenv("BOXBOX_USERS_bob", testHashB)
	t.Setenv("BOXBOX_NAS_IDENTITY_bob", "3001:3001:3000,3002")

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]model.NASIdentity{
		"alice": {UID: 3000, GID: 3000, Groups: []uint32{3001}},
		"bob":   {UID: 3001, GID: 3001, Groups: []uint32{3000, 3002}},
	}
	if !reflect.DeepEqual(cfg.NAS.Identities, want) {
		t.Fatalf("Identities = %#v", cfg.NAS.Identities)
	}
	if cfg.NAS.ServiceUID != DefaultNASServiceID || cfg.NAS.ServiceGID != DefaultNASServiceID {
		t.Fatalf("service identity = %d:%d", cfg.NAS.ServiceUID, cfg.NAS.ServiceGID)
	}
}

func TestLoadRejectsUnsafeNASIdentities(t *testing.T) {
	for _, test := range []struct {
		name  string
		env   map[string]string
		error string
	}{
		{"root uid", map[string]string{"BOXBOX_NAS_IDENTITY_admin": "0:3000"}, "must not be 0"},
		{"root group", map[string]string{"BOXBOX_NAS_IDENTITY_admin": "3000:3000:0"}, "groups must not include 0"},
		{"unknown user", map[string]string{"BOXBOX_NAS_IDENTITY_mallory": "3000:3000"}, "does not match a configured user"},
		{"malformed", map[string]string{"BOXBOX_NAS_IDENTITY_admin": "alice"}, "uid:gid"},
		{"root service", map[string]string{"BOXBOX_NAS_IDENTITY_admin": "3000:3000", "BOXBOX_NAS_SERVICE_UID": "0"}, "service_uid"},
	} {
		t.Run(test.name, func(t *testing.T) {
			clearConfigEnv(t)
			configPath := writeMinimalConfig(t)
			t.Setenv("BOXBOX_JWT_SECRET", "unit-test-secret-at-least-thirty-two-bytes")
			t.Setenv("BOXBOX_USERS_admin", testHashA)
			for key, value := range test.env {
				t.Setenv(key, value)
			}
			if _, err := Load(configPath); err == nil || !strings.Contains(err.Error(), test.error) {
				t.Fatalf("Load() error = %v, want %q", err, test.error)
			}
		})
	}
}
