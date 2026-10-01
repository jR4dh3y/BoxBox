package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
)

// --dev must produce credentials that the real config loader accepts.
func TestDevCredentialsPassConfigValidation(t *testing.T) {
	// t.Setenv restores both variables after the test.
	t.Setenv("BOXBOX_JWT_SECRET", "")
	t.Setenv("BOXBOX_USERS_dev", "")
	if err := setDevCredentials(); err != nil {
		t.Fatal(err)
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "config.yaml")
	yaml := "data_dir: " + dir + "\nmount_points:\n  - name: home\n    path: " + dir + "\n"
	if err := os.WriteFile(path, []byte(yaml), 0o600); err != nil {
		t.Fatal(err)
	}

	cfg, err := config.Load(path)
	if err != nil {
		t.Fatalf("config.Load with dev credentials: %v", err)
	}
	if _, ok := cfg.Users["dev"]; !ok {
		t.Fatalf("users = %v, want the dev user", cfg.Users)
	}
}
