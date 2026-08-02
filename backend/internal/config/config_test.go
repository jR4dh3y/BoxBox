package config

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

const (
	testHashA = "$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy"
	testHashB = "$2b$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWx"
)

func TestLoadUsesBoxBoxEnvironmentPrefix(t *testing.T) {
	clearConfigEnv(t)
	configPath := writeMinimalConfig(t)

	t.Setenv("BOXBOX_JWT_SECRET", "unit-test-secret-at-least-thirty-two-bytes")
	t.Setenv("BOXBOX_USERS_admin", testHashA)
	t.Setenv("BOXBOX_ALLOWED_ORIGINS", "https://boxbox.example.com, *.internal.example.com ")
	t.Setenv("BOXBOX_PORT", "9090")

	result, err := LoadWithReport(configPath)
	if err != nil {
		t.Fatalf("LoadWithReport() error = %v", err)
	}
	cfg := result.Config

	if cfg.JWTSecret != "unit-test-secret-at-least-thirty-two-bytes" {
		t.Fatalf("JWTSecret = %q, want strong unit-test secret", cfg.JWTSecret)
	}
	if cfg.Users["admin"] != testHashA {
		t.Fatalf("Users[admin] = %q, want test hash", cfg.Users["admin"])
	}
	if cfg.Port != 9090 {
		t.Fatalf("Port = %d, want 9090", cfg.Port)
	}
	wantOrigins := []string{"https://boxbox.example.com", "*.internal.example.com"}
	if !reflect.DeepEqual(cfg.AllowedOrigins, wantOrigins) {
		t.Fatalf("AllowedOrigins = %#v, want %#v", cfg.AllowedOrigins, wantOrigins)
	}
	if len(result.Warnings) != 0 {
		t.Fatalf("Warnings = %#v, want none", result.Warnings)
	}
}

func TestLoadSupportsLegacyFMEnvironmentAliases(t *testing.T) {
	clearConfigEnv(t)
	configPath := writeMinimalConfig(t)

	t.Setenv("FM_JWT_SECRET", "legacy-secret-at-least-thirty-two-bytes")
	t.Setenv("FM_USERS_admin", testHashA)
	t.Setenv("FM_ALLOWED_ORIGINS", "https://legacy.example.com, *.legacy.example.com ")
	t.Setenv("FM_PORT", "9091")
	t.Setenv("FM_HOST", "127.0.0.1")
	t.Setenv("FM_RATE_LIMIT_RPS", "3.5")
	t.Setenv("FM_MAX_UPLOAD_MB", "2048")
	t.Setenv("FM_CHUNK_SIZE_MB", "11")

	result, err := LoadWithReport(configPath)
	if err != nil {
		t.Fatalf("LoadWithReport() error = %v", err)
	}
	cfg := result.Config

	if cfg.JWTSecret != "legacy-secret-at-least-thirty-two-bytes" {
		t.Fatalf("JWTSecret = %q, want strong legacy secret", cfg.JWTSecret)
	}
	if cfg.Users["admin"] != testHashA {
		t.Fatalf("Users[admin] = %q, want legacy test hash", cfg.Users["admin"])
	}
	if cfg.Port != 9091 {
		t.Fatalf("Port = %d, want 9091", cfg.Port)
	}
	if cfg.Host != "127.0.0.1" {
		t.Fatalf("Host = %q, want 127.0.0.1", cfg.Host)
	}
	if cfg.RateLimitRPS != 3.5 {
		t.Fatalf("RateLimitRPS = %v, want 3.5", cfg.RateLimitRPS)
	}
	if cfg.MaxUploadMB != 2048 {
		t.Fatalf("MaxUploadMB = %d, want 2048", cfg.MaxUploadMB)
	}
	if cfg.ChunkSizeMB != 11 {
		t.Fatalf("ChunkSizeMB = %d, want 11", cfg.ChunkSizeMB)
	}
	wantOrigins := []string{"https://legacy.example.com", "*.legacy.example.com"}
	if !reflect.DeepEqual(cfg.AllowedOrigins, wantOrigins) {
		t.Fatalf("AllowedOrigins = %#v, want %#v", cfg.AllowedOrigins, wantOrigins)
	}

	for _, legacy := range []string{
		"FM_JWT_SECRET",
		"FM_USERS_admin",
		"FM_ALLOWED_ORIGINS",
		"FM_PORT",
		"FM_HOST",
		"FM_RATE_LIMIT_RPS",
		"FM_MAX_UPLOAD_MB",
		"FM_CHUNK_SIZE_MB",
	} {
		if !hasWarning(result.Warnings, legacy) {
			t.Fatalf("Warnings = %#v, want warning for %s", result.Warnings, legacy)
		}
	}
}

func TestLoadPrefersBoxBoxEnvironmentOverLegacyAliases(t *testing.T) {
	clearConfigEnv(t)
	configPath := writeMinimalConfig(t)

	t.Setenv("FM_JWT_SECRET", "legacy-secret-at-least-thirty-two-bytes")
	t.Setenv("BOXBOX_JWT_SECRET", "boxbox-secret-at-least-thirty-two-bytes")
	t.Setenv("FM_USERS_admin", testHashA)
	t.Setenv("BOXBOX_USERS_admin", testHashB)
	t.Setenv("FM_ALLOWED_ORIGINS", "https://legacy.example.com")
	t.Setenv("BOXBOX_ALLOWED_ORIGINS", "https://boxbox.example.com")
	t.Setenv("FM_PORT", "9091")
	t.Setenv("BOXBOX_PORT", "9092")

	result, err := LoadWithReport(configPath)
	if err != nil {
		t.Fatalf("LoadWithReport() error = %v", err)
	}
	cfg := result.Config

	if cfg.JWTSecret != "boxbox-secret-at-least-thirty-two-bytes" {
		t.Fatalf("JWTSecret = %q, want strong boxbox secret", cfg.JWTSecret)
	}
	if cfg.Users["admin"] != testHashB {
		t.Fatalf("Users[admin] = %q, want replacement test hash", cfg.Users["admin"])
	}
	if cfg.Port != 9092 {
		t.Fatalf("Port = %d, want 9092", cfg.Port)
	}
	wantOrigins := []string{"https://boxbox.example.com"}
	if !reflect.DeepEqual(cfg.AllowedOrigins, wantOrigins) {
		t.Fatalf("AllowedOrigins = %#v, want %#v", cfg.AllowedOrigins, wantOrigins)
	}

	for _, legacy := range []string{"FM_JWT_SECRET", "FM_USERS_admin", "FM_ALLOWED_ORIGINS", "FM_PORT"} {
		if !hasWarning(result.Warnings, legacy) {
			t.Fatalf("Warnings = %#v, want warning for %s", result.Warnings, legacy)
		}
	}
}

func TestLoadStillReturnsConfigOnly(t *testing.T) {
	clearConfigEnv(t)
	configPath := writeMinimalConfig(t)

	t.Setenv("BOXBOX_JWT_SECRET", "unit-test-secret-at-least-thirty-two-bytes")
	t.Setenv("BOXBOX_USERS_admin", testHashA)

	cfg, err := Load(configPath)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if cfg.JWTSecret != "unit-test-secret-at-least-thirty-two-bytes" {
		t.Fatalf("JWTSecret = %q, want strong unit-test secret", cfg.JWTSecret)
	}
}

func TestLoadWarnsForDeprecatedConfigSearchPath(t *testing.T) {
	clearConfigEnv(t)
	tempDir := t.TempDir()
	newConfigDir := filepath.Join(tempDir, "boxbox")
	legacyConfigDir := filepath.Join(tempDir, "filemanager")
	if err := os.MkdirAll(legacyConfigDir, 0o755); err != nil {
		t.Fatalf("create legacy config dir: %v", err)
	}
	configPath := filepath.Join(legacyConfigDir, "config.yaml")
	content := []byte(`jwt_secret: unit-test-secret-at-least-thirty-two-bytes
users:
  admin: "$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy"
mount_points:
  - name: home
    path: /tmp
    read_only: true
`)
	if err := os.WriteFile(configPath, content, 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	result, err := loadWithReport("", []configSearchPath{
		{Path: newConfigDir},
		{Path: legacyConfigDir, Legacy: true, Replacement: filepath.Join(newConfigDir, "config.yaml")},
	})
	if err != nil {
		t.Fatalf("loadWithReport() error = %v", err)
	}

	if result.Config.JWTSecret != "unit-test-secret-at-least-thirty-two-bytes" {
		t.Fatalf("JWTSecret = %q, want strong unit-test secret", result.Config.JWTSecret)
	}
	if !hasWarning(result.Warnings, filepath.Join(legacyConfigDir, "config.yaml")) {
		t.Fatalf("Warnings = %#v, want warning for legacy config path", result.Warnings)
	}
}

func writeMinimalConfig(t *testing.T) string {
	t.Helper()
	configPath := filepath.Join(t.TempDir(), "config.yaml")
	content := []byte(`mount_points:
  - name: home
    path: /tmp
    read_only: true
`)
	if err := os.WriteFile(configPath, content, 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}
	return configPath
}

func hasWarning(warnings []MigrationWarning, legacy string) bool {
	for _, warning := range warnings {
		if warning.Legacy == legacy {
			return true
		}
	}
	return false
}

func clearConfigEnv(t *testing.T) {
	t.Helper()

	for _, env := range os.Environ() {
		key := strings.SplitN(env, "=", 2)[0]
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
