package model

import (
	"fmt"
	"net/netip"
	"path/filepath"
	"strings"

	"golang.org/x/crypto/bcrypt"
)

// MountPoint represents a configured filesystem location accessible through the file manager
type MountPoint struct {
	Name         string `json:"name" mapstructure:"name"`
	Path         string `json:"path" mapstructure:"path"`
	ReadOnly     bool   `json:"readOnly" mapstructure:"read_only"`
	AutoDiscover bool   `json:"autoDiscover" mapstructure:"auto_discover"`
}

// ServerConfig contains all server configuration options
type ServerConfig struct {
	Port           int          `mapstructure:"port"`
	Host           string       `mapstructure:"host"`
	MountPoints    []MountPoint `mapstructure:"mount_points"`
	JWTSecret      string       `mapstructure:"jwt_secret"`
	MaxUploadMB    int          `mapstructure:"max_upload_mb"`
	ChunkSizeMB    int          `mapstructure:"chunk_size_mb"`
	DataDir        string       `mapstructure:"data_dir"`
	AllowRootMount bool         `mapstructure:"allow_root_mount"`

	// Security settings
	Users          map[string]string `mapstructure:"users"`           // username -> bcrypt password hash
	AllowedOrigins []string          `mapstructure:"allowed_origins"` // WebSocket/CORS allowed origins
	TrustedProxies []string          `mapstructure:"trusted_proxies"` // CIDRs allowed to supply forwarding headers
	RateLimitRPS   float64           `mapstructure:"rate_limit_rps"`  // Auth endpoint rate limit (requests per second)
}

// Validate checks that the configuration is valid
func (c *ServerConfig) Validate() error {
	if c.JWTSecret == "" {
		return fmt.Errorf("jwt_secret is required")
	}
	if len(c.JWTSecret) < 32 {
		return fmt.Errorf("jwt_secret must be at least 32 bytes")
	}
	if strings.Contains(strings.ToLower(c.JWTSecret), "change-me") {
		return fmt.Errorf("jwt_secret must be changed from the placeholder value")
	}

	if len(c.Users) == 0 {
		return fmt.Errorf("at least one user is required")
	}
	for username, password := range c.Users {
		if username == "" {
			return fmt.Errorf("usernames cannot be empty")
		}
		if password == "" {
			return fmt.Errorf("password for user %q is required", username)
		}
		if !isBcryptHash(password) {
			return fmt.Errorf("password for user %q must be a bcrypt hash ($2a$, $2b$, or $2y$)", username)
		}
	}

	if len(c.MountPoints) == 0 {
		return fmt.Errorf("at least one mount_point is required")
	}

	for i, mp := range c.MountPoints {
		if mp.Name == "" {
			return fmt.Errorf("mount_point[%d].name is required", i)
		}
		if mp.Path == "" {
			return fmt.Errorf("mount_point[%d].path is required", i)
		}
		if !c.AllowRootMount && resolvesToFilesystemRoot(mp.Path) {
			return fmt.Errorf("mount_point[%d] resolves to filesystem root; set allow_root_mount only after reviewing the security risk", i)
		}
	}

	if c.Port < 1 || c.Port > 65535 {
		return fmt.Errorf("port must be between 1 and 65535")
	}

	if c.MaxUploadMB < 1 {
		return fmt.Errorf("max_upload_mb must be at least 1")
	}

	if c.ChunkSizeMB < 1 {
		return fmt.Errorf("chunk_size_mb must be at least 1")
	}
	for _, trustedProxy := range c.TrustedProxies {
		if _, err := netip.ParsePrefix(trustedProxy); err != nil {
			if _, addressErr := netip.ParseAddr(trustedProxy); addressErr != nil {
				return fmt.Errorf("trusted_proxy %q must be an IP address or CIDR", trustedProxy)
			}
		}
	}

	return nil
}

func isBcryptHash(value string) bool {
	if len(value) != 60 ||
		(!strings.HasPrefix(value, "$2a$") &&
			!strings.HasPrefix(value, "$2b$") &&
			!strings.HasPrefix(value, "$2y$")) {
		return false
	}
	cost, err := bcrypt.Cost([]byte(value))
	return err == nil && cost >= bcrypt.DefaultCost
}

func resolvesToFilesystemRoot(path string) bool {
	absolute, err := filepath.Abs(path)
	if err != nil {
		return false
	}
	resolved, err := filepath.EvalSymlinks(absolute)
	if err == nil {
		absolute = resolved
	}
	volume := filepath.VolumeName(absolute)
	return filepath.Clean(absolute) == filepath.Clean(volume+string(filepath.Separator))
}
