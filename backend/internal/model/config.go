package model

import "fmt"

// MountPoint represents a configured filesystem location accessible through the file manager
type MountPoint struct {
	Name         string `json:"name" mapstructure:"name"`
	Path         string `json:"path" mapstructure:"path"`
	ReadOnly     bool   `json:"readOnly" mapstructure:"read_only"`
	AutoDiscover bool   `json:"autoDiscover" mapstructure:"auto_discover"`
}

// ServerConfig contains all server configuration options
type ServerConfig struct {
	Port        int          `mapstructure:"port"`
	Host        string       `mapstructure:"host"`
	MountPoints []MountPoint `mapstructure:"mount_points"`
	JWTSecret   string       `mapstructure:"jwt_secret"`
	MaxUploadMB int          `mapstructure:"max_upload_mb"`
	ChunkSizeMB int          `mapstructure:"chunk_size_mb"`
	DataDir     string       `mapstructure:"data_dir"`

	// Security settings
	Users          map[string]string `mapstructure:"users"`           // username -> password
	AllowedOrigins []string          `mapstructure:"allowed_origins"` // WebSocket/CORS allowed origins
	RateLimitRPS   float64           `mapstructure:"rate_limit_rps"`  // Auth endpoint rate limit (requests per second)
}

// DefaultServerConfig returns sensible defaults for server configuration
func DefaultServerConfig() ServerConfig {
	return ServerConfig{
		Port:        8080,
		Host:        "0.0.0.0",
		MountPoints: []MountPoint{},
		JWTSecret:   "",
		MaxUploadMB: 10240, // 10GB
		ChunkSizeMB: 5,     // 5MB chunks
		DataDir:     "",
		// Security defaults
		Users:          nil,  // Must be configured
		AllowedOrigins: nil,  // nil = allow all (for homelab)
		RateLimitRPS:   10.0, // 10 requests per second per IP
	}
}

// Validate checks that the configuration is valid
func (c *ServerConfig) Validate() error {
	if c.JWTSecret == "" {
		return fmt.Errorf("jwt_secret is required")
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

	return nil
}

// IsMountPointReadOnly checks if a mount point is read-only by name
func (c *ServerConfig) IsMountPointReadOnly(name string) bool {
	for _, mp := range c.MountPoints {
		if mp.Name == name {
			return mp.ReadOnly
		}
	}
	return true // Default to read-only if not found
}

// GetMountPoint returns a mount point by name, or nil if not found
func (c *ServerConfig) GetMountPoint(name string) *MountPoint {
	for i := range c.MountPoints {
		if c.MountPoints[i].Name == name {
			return &c.MountPoints[i]
		}
	}
	return nil
}
