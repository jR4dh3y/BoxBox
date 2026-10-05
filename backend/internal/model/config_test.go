package model

import (
	"strings"
	"testing"
)

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

func TestClassifyMountPoints(t *testing.T) {
	kinds := func(mounts []MountPoint) map[string]string {
		result := make(map[string]string, len(mounts))
		for _, mount := range ClassifyMountPoints(mounts) {
			result[mount.Name] = mount.Kind
		}
		return result
	}

	docker := kinds([]MountPoint{
		{Name: "drives", Path: "/media/devmon", AutoDiscover: true},
		{Name: "home", Path: "/home/user"},
		{Name: "downloads", Path: "/home/user/Downloads"},
		{Name: "pictures", Path: "/home/user/Pictures/"},
	})
	want := map[string]string{"drives": MountKindDrive, "home": MountKindDrive, "downloads": MountKindPlace, "pictures": MountKindPlace}
	for name, kind := range want {
		if docker[name] != kind {
			t.Errorf("docker defaults: %s kind = %q, want %q", name, docker[name], kind)
		}
	}

	// Everything is inside "/", so a root mount must not turn every mount into a place.
	withRoot := kinds([]MountPoint{
		{Name: "root", Path: "/"},
		{Name: "home", Path: "/home/user"},
		{Name: "downloads", Path: "/home/user/Downloads"},
		{Name: "homey", Path: "/home/username"},
	})
	if withRoot["root"] != MountKindDrive || withRoot["home"] != MountKindDrive || withRoot["downloads"] != MountKindPlace || withRoot["homey"] != MountKindDrive {
		t.Errorf("root mount classification = %v", withRoot)
	}

	explicit := kinds([]MountPoint{
		{Name: "home", Path: "/home/user", Kind: MountKindPlace},
		{Name: "downloads", Path: "/home/user/Downloads", Kind: MountKindDrive},
	})
	if explicit["home"] != MountKindPlace || explicit["downloads"] != MountKindDrive {
		t.Errorf("explicit kinds were overridden: %v", explicit)
	}
}

func TestValidateRejectsUnknownMountKind(t *testing.T) {
	cfg := ServerConfig{
		JWTSecret:   "0123456789abcdef0123456789abcdef",
		Users:       map[string]string{"admin": validTestBcryptHash},
		MountPoints: []MountPoint{{Name: "data", Path: "/data", Kind: "shortcut"}},
		Port:        80,
		MaxUploadMB: 1,
		ChunkSizeMB: 1,
	}
	if err := cfg.Validate(); err == nil || !strings.Contains(err.Error(), "kind") {
		t.Fatalf("Validate() = %v, want a kind error", err)
	}
}
