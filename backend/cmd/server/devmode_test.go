package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
)

// clearConfigEnv unsets every variable the config loader reads, so a developer's own
// BoxBox settings cannot change the result. Unset, not empty: the loader treats a set
// FM_* variable as a legacy override even when it is empty.
func clearConfigEnv(t *testing.T) {
	t.Helper()
	for _, env := range os.Environ() {
		key, _, _ := strings.Cut(env, "=")
		if !strings.HasPrefix(key, "BOXBOX_") && !strings.HasPrefix(key, "FM_") && key != "CONFIG_PATH" {
			continue
		}
		value, existed := os.LookupEnv(key)
		if err := os.Unsetenv(key); err != nil {
			t.Fatalf("unset %s: %v", key, err)
		}
		t.Cleanup(func() {
			if existed {
				_ = os.Setenv(key, value)
			} else {
				_ = os.Unsetenv(key)
			}
		})
	}
}

// --dev must produce credentials that the real config loader accepts.
func TestDevCredentialsPassConfigValidation(t *testing.T) {
	clearConfigEnv(t)
	// setDevCredentials writes these two; t.Setenv makes the test restore them.
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
