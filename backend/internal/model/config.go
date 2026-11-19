package model

import (
	"fmt"
	"net/netip"
	"path/filepath"
	"slices"
	"strings"

	"golang.org/x/crypto/bcrypt"
)

// Mount point kinds. Drives are storage roots shown on "This Server"; places are
// folder shortcuts (Downloads, Pictures, ...) listed separately in the sidebar.
const (
	MountKindDrive = "drive"
	MountKindPlace = "place"
)

// MountPoint represents a configured filesystem location accessible through the file manager
type MountPoint struct {
	Name         string `json:"name" mapstructure:"name"`
	Path         string `json:"path" mapstructure:"path"`
	ReadOnly     bool   `json:"readOnly" mapstructure:"read_only"`
	AutoDiscover bool   `json:"autoDiscover" mapstructure:"auto_discover"`
	Kind         string `json:"kind" mapstructure:"kind"`
}

// ClassifyMountPoints fills in Kind for mount points that do not set it.
// Auto-discovered mounts are drives. A mount inside another configured mount
// (such as /home/user/Downloads inside /home/user) is a place; nesting under a
// filesystem-root mount does not count, since everything is inside "/".
// Everything else is a drive.
func ClassifyMountPoints(mounts []MountPoint) []MountPoint {
	classified := make([]MountPoint, len(mounts))
	copy(classified, mounts)
	for i := range classified {
		if classified[i].Kind != "" {
			continue
		}
		classified[i].Kind = MountKindDrive
		if classified[i].AutoDiscover {
			continue
		}
		for j, other := range mounts {
			if i != j && !resolvesToFilesystemRoot(other.Path) && isNestedPath(other.Path, classified[i].Path) {
				classified[i].Kind = MountKindPlace
				break
			}
		}
	}
	return classified
}

func isNestedPath(parent, child string) bool {
	relative, err := filepath.Rel(filepath.Clean(parent), filepath.Clean(child))
	return err == nil && relative != "." && relative != ".." &&
		!strings.HasPrefix(relative, ".."+string(filepath.Separator)) && !filepath.IsAbs(relative)
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

	NAS NASConfig `mapstructure:"nas"`
}

// NASConfig maps BoxBox accounts to filesystem identities. With any identity
// set, BoxBox runs each account's file access in a worker process running as
// that identity, and accounts without one get no file access.
type NASConfig struct {
	// The API process runs as this unprivileged identity.
	ServiceUID uint32                 `mapstructure:"service_uid"`
	ServiceGID uint32                 `mapstructure:"service_gid"`
	Identities map[string]NASIdentity `mapstructure:"identities"`
}

// NASIdentity is numeric because the container does not share the NAS's
// account database.
type NASIdentity struct {
	UID    uint32   `mapstructure:"uid"`
	GID    uint32   `mapstructure:"gid"`
	Groups []uint32 `mapstructure:"groups"`
}

func (n NASConfig) Enabled() bool {
	return len(n.Identities) > 0
}

func (n NASConfig) validate(users map[string]string) error {
	if !n.Enabled() {
		return nil
	}
	if n.ServiceUID == 0 || n.ServiceGID == 0 {
		return fmt.Errorf("nas.service_uid and nas.service_gid must not be 0")
	}
	for username, identity := range n.Identities {
		if _, ok := users[username]; !ok {
			return fmt.Errorf("nas.identities.%s does not match a configured user", username)
		}
		if identity.UID == 0 || identity.GID == 0 {
			return fmt.Errorf("nas.identities.%s: uid and gid are required and must not be 0", username)
		}
		if slices.Contains(identity.Groups, 0) {
			return fmt.Errorf("nas.identities.%s: groups must not include 0", username)
		}
	}
	return nil
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
		if mp.Kind != "" && mp.Kind != MountKindDrive && mp.Kind != MountKindPlace {
			return fmt.Errorf("mount_point[%d].kind must be %q or %q", i, MountKindDrive, MountKindPlace)
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
	if err := c.NAS.validate(c.Users); err != nil {
		return err
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
