package config

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestLoadUsesBoxBoxEnvironmentPrefix(t *testing.T) {
	configPath := filepath.Join(t.TempDir(), "config.yaml")
	content := []byte(`mount_points:
  - name: home
    path: /tmp
    read_only: true
`)
	if err := os.WriteFile(configPath, content, 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	t.Setenv("BOXBOX_JWT_SECRET", "unit-test-secret")
	t.Setenv("BOXBOX_USERS_admin", "unit-test-password")
	t.Setenv("BOXBOX_ALLOWED_ORIGINS", "https://boxbox.example.com, *.internal.example.com ")
	t.Setenv("BOXBOX_PORT", "9090")

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	if cfg.JWTSecret != "unit-test-secret" {
		t.Fatalf("JWTSecret = %q, want unit-test-secret", cfg.JWTSecret)
	}
	if cfg.Users["admin"] != "unit-test-password" {
		t.Fatalf("Users[admin] = %q, want unit-test-password", cfg.Users["admin"])
	}
	if cfg.Port != 9090 {
		t.Fatalf("Port = %d, want 9090", cfg.Port)
	}
	wantOrigins := []string{"https://boxbox.example.com", "*.internal.example.com"}
	if !reflect.DeepEqual(cfg.AllowedOrigins, wantOrigins) {
		t.Fatalf("AllowedOrigins = %#v, want %#v", cfg.AllowedOrigins, wantOrigins)
	}
}
