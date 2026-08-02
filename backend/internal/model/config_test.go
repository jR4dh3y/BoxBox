package model

import "testing"

const validTestBcryptHash = "$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy"

func TestServerConfigRejectsUnsafeAuthDefaults(t *testing.T) {
	validMounts := []MountPoint{{Name: "home", Path: "/home/user"}}

	tests := []struct {
		name string
		cfg  ServerConfig
	}{
		{
			name: "missing users",
			cfg: ServerConfig{
				JWTSecret:   "test-secret",
				MountPoints: validMounts,
				Port:        80,
				MaxUploadMB: 1,
				ChunkSizeMB: 1,
			},
		},
		{
			name: "placeholder jwt secret",
			cfg: ServerConfig{
				JWTSecret:   "change-me-in-production",
				Users:       map[string]string{"admin": "correct-password"},
				MountPoints: validMounts,
				Port:        80,
				MaxUploadMB: 1,
				ChunkSizeMB: 1,
			},
		},
		{
			name: "placeholder password",
			cfg: ServerConfig{
				JWTSecret:   "test-secret",
				Users:       map[string]string{"admin": "change-me-in-production"},
				MountPoints: validMounts,
				Port:        80,
				MaxUploadMB: 1,
				ChunkSizeMB: 1,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := tt.cfg.Validate(); err == nil {
				t.Fatal("expected validation error")
			}
		})
	}
}

func TestServerConfigRequiresStrongHashedCredentials(t *testing.T) {
	base := ServerConfig{
		JWTSecret:   "0123456789abcdef0123456789abcdef",
		Users:       map[string]string{"admin": validTestBcryptHash},
		MountPoints: []MountPoint{{Name: "home", Path: "/tmp"}},
		Port:        80,
		MaxUploadMB: 1,
		ChunkSizeMB: 1,
	}
	if err := base.Validate(); err != nil {
		t.Fatalf("valid security config rejected: %v", err)
	}

	plaintext := base
	plaintext.Users = map[string]string{"admin": "correct-password"}
	if err := plaintext.Validate(); err == nil {
		t.Fatal("plaintext password was accepted")
	}

	shortSecret := base
	shortSecret.JWTSecret = "too-short"
	if err := shortSecret.Validate(); err == nil {
		t.Fatal("short JWT secret was accepted")
	}
}

func TestServerConfigRejectsFilesystemRootMountByDefault(t *testing.T) {
	cfg := ServerConfig{
		JWTSecret:   "0123456789abcdef0123456789abcdef",
		Users:       map[string]string{"admin": validTestBcryptHash},
		MountPoints: []MountPoint{{Name: "root", Path: "/"}},
		Port:        80,
		MaxUploadMB: 1,
		ChunkSizeMB: 1,
	}
	if err := cfg.Validate(); err == nil {
		t.Fatal("root filesystem mount was accepted without override")
	}
	cfg.AllowRootMount = true
	if err := cfg.Validate(); err != nil {
		t.Fatalf("explicit root mount override rejected: %v", err)
	}
}
