package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/spf13/viper"
)

// MigrationWarning describes a deprecated deployment input detected while
// loading configuration.
type MigrationWarning struct {
	Legacy      string
	Replacement string
	Message     string
}

// LoadResult contains the loaded config and any non-fatal migration warnings.
type LoadResult struct {
	Config   *model.ServerConfig
	Warnings []MigrationWarning
}

type envAlias struct {
	Key         string
	Legacy      string
	Replacement string
}

type configSearchPath struct {
	Path        string
	Legacy      bool
	Replacement string
}

var scalarEnvAliases = []envAlias{
	{Key: "jwt_secret", Legacy: "FM_JWT_SECRET", Replacement: "BOXBOX_JWT_SECRET"},
	{Key: "port", Legacy: "FM_PORT", Replacement: "BOXBOX_PORT"},
	{Key: "host", Legacy: "FM_HOST", Replacement: "BOXBOX_HOST"},
	{Key: "rate_limit_rps", Legacy: "FM_RATE_LIMIT_RPS", Replacement: "BOXBOX_RATE_LIMIT_RPS"},
	{Key: "max_upload_mb", Legacy: "FM_MAX_UPLOAD_MB", Replacement: "BOXBOX_MAX_UPLOAD_MB"},
	{Key: "chunk_size_mb", Legacy: "FM_CHUNK_SIZE_MB", Replacement: "BOXBOX_CHUNK_SIZE_MB"},
}

var defaultConfigSearchPaths = []configSearchPath{
	{Path: "."},
	{Path: "./config"},
	{Path: "/etc/boxbox"},
	{Path: "/etc/filemanager", Legacy: true, Replacement: "/etc/boxbox/config.yaml"},
}

// Load reads configuration from file and environment variables
func Load(configPath string) (*model.ServerConfig, error) {
	result, err := LoadWithReport(configPath)
	if err != nil {
		return nil, err
	}
	return result.Config, nil
}

// LoadWithReport reads configuration from file and environment variables,
// returning deprecated migration inputs that should be surfaced to operators.
func LoadWithReport(configPath string) (*LoadResult, error) {
	return loadWithReport(configPath, defaultConfigSearchPaths)
}

func loadWithReport(configPath string, searchPaths []configSearchPath) (*LoadResult, error) {
	v := viper.New()
	warnings := make([]MigrationWarning, 0)

	// Set defaults
	v.SetDefault("port", 80)
	v.SetDefault("host", "0.0.0.0")
	v.SetDefault("max_upload_mb", DefaultMaxUploadMB) // 10GB default
	v.SetDefault("chunk_size_mb", 5)                  // 5MB chunks
	v.SetDefault("rate_limit_rps", 2.0)
	v.SetDefault("data_dir", DefaultDataDir)

	if configPath == "" {
		configPath = os.Getenv("CONFIG_PATH")
	}

	// Config file settings
	if configPath != "" {
		v.SetConfigFile(configPath)
		if isDefaultLegacyConfigFile(configPath) {
			warnings = append(warnings, configPathWarning(configPath, "/etc/boxbox/config.yaml"))
		}
	} else {
		v.SetConfigName("config")
		v.SetConfigType("yaml")
		for _, searchPath := range searchPaths {
			v.AddConfigPath(searchPath.Path)
		}
	}

	// Environment variable settings
	v.SetEnvPrefix("BOXBOX")
	v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	v.AutomaticEnv()

	for _, key := range []string{
		"jwt_secret",
		"port",
		"host",
		"rate_limit_rps",
		"max_upload_mb",
		"chunk_size_mb",
		"data_dir",
		"allow_root_mount",
	} {
		if err := v.BindEnv(key); err != nil {
			return nil, fmt.Errorf("bind environment variable %q: %w", key, err)
		}
	}

	for _, alias := range scalarEnvAliases {
		if legacyValue, ok := os.LookupEnv(alias.Legacy); ok {
			warnings = append(warnings, envWarning(alias.Legacy, alias.Replacement))
			if _, replacementSet := os.LookupEnv(alias.Replacement); !replacementSet {
				v.Set(alias.Key, legacyValue)
			}
		}
	}

	// Read config file
	if err := v.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			return nil, fmt.Errorf("error reading config file: %w", err)
		}
		// Config file not found is okay, we'll use defaults and env vars
	} else if configPath == "" {
		warnings = append(warnings, configSearchWarnings(v.ConfigFileUsed(), searchPaths)...)
	}

	var cfg model.ServerConfig
	if err := v.Unmarshal(&cfg); err != nil {
		return nil, fmt.Errorf("error unmarshaling config: %w", err)
	}

	// Parse BOXBOX_USERS_* environment variables into the Users map.
	// Viper's AutomaticEnv doesn't handle map types from env vars like BOXBOX_USERS_username=password.
	if cfg.Users == nil {
		cfg.Users = make(map[string]string)
	}
	applyLegacyUserEnv(&cfg, &warnings)
	for _, env := range os.Environ() {
		if strings.HasPrefix(env, "BOXBOX_USERS_") {
			parts := strings.SplitN(env, "=", 2)
			if len(parts) == 2 {
				username := strings.TrimPrefix(parts[0], "BOXBOX_USERS_")
				password := parts[1]
				if username != "" && password != "" {
					cfg.Users[username] = password
				}
			}
		}
	}

	if origins, ok := os.LookupEnv("FM_ALLOWED_ORIGINS"); ok {
		warnings = append(warnings, envWarning("FM_ALLOWED_ORIGINS", "BOXBOX_ALLOWED_ORIGINS"))
		if _, replacementSet := os.LookupEnv("BOXBOX_ALLOWED_ORIGINS"); !replacementSet {
			cfg.AllowedOrigins = splitEnvList(origins)
		}
	}

	if origins := os.Getenv("BOXBOX_ALLOWED_ORIGINS"); origins != "" {
		cfg.AllowedOrigins = splitEnvList(origins)
	}
	if proxies := os.Getenv("BOXBOX_TRUSTED_PROXIES"); proxies != "" {
		cfg.TrustedProxies = splitEnvList(proxies)
	}

	if err := cfg.Validate(); err != nil {
		return nil, err
	}

	return &LoadResult{
		Config:   &cfg,
		Warnings: warnings,
	}, nil
}

func splitEnvList(value string) []string {
	parts := strings.Split(value, ",")
	result := make([]string, 0, len(parts))
	for _, part := range parts {
		trimmed := strings.TrimSpace(part)
		if trimmed != "" {
			result = append(result, trimmed)
		}
	}
	return result
}

func applyLegacyUserEnv(cfg *model.ServerConfig, warnings *[]MigrationWarning) {
	replacementUsers := make(map[string]bool)
	for _, env := range os.Environ() {
		if strings.HasPrefix(env, "BOXBOX_USERS_") {
			parts := strings.SplitN(env, "=", 2)
			if len(parts) == 2 {
				username := strings.TrimPrefix(parts[0], "BOXBOX_USERS_")
				replacementUsers[username] = true
			}
		}
	}

	for _, env := range os.Environ() {
		if strings.HasPrefix(env, "FM_USERS_") {
			parts := strings.SplitN(env, "=", 2)
			if len(parts) != 2 {
				continue
			}
			username := strings.TrimPrefix(parts[0], "FM_USERS_")
			legacy := "FM_USERS_" + username
			replacement := "BOXBOX_USERS_" + username
			*warnings = append(*warnings, envWarning(legacy, replacement))

			password := parts[1]
			if username != "" && password != "" && !replacementUsers[username] {
				cfg.Users[username] = password
			}
		}
	}
}

func configSearchWarnings(usedConfigPath string, searchPaths []configSearchPath) []MigrationWarning {
	if usedConfigPath == "" {
		return nil
	}

	usedDir := filepath.Clean(filepath.Dir(usedConfigPath))
	warnings := make([]MigrationWarning, 0, 1)
	for _, searchPath := range searchPaths {
		if !searchPath.Legacy {
			continue
		}
		if filepath.Clean(searchPath.Path) == usedDir {
			legacy := filepath.Join(searchPath.Path, "config.yaml")
			warnings = append(warnings, configPathWarning(legacy, searchPath.Replacement))
		}
	}
	return warnings
}

func isDefaultLegacyConfigFile(path string) bool {
	return filepath.Clean(path) == filepath.Clean("/etc/filemanager/config.yaml")
}

func envWarning(legacy, replacement string) MigrationWarning {
	return MigrationWarning{
		Legacy:      legacy,
		Replacement: replacement,
		Message: fmt.Sprintf(
			"Detected deprecated FileManager migration input: %s. Rename it to %s. %s takes precedence when both are set.",
			legacy,
			replacement,
			replacement,
		),
	}
}

func configPathWarning(legacy, replacement string) MigrationWarning {
	return MigrationWarning{
		Legacy:      legacy,
		Replacement: replacement,
		Message: fmt.Sprintf(
			"Detected deprecated FileManager config path: %s. Move it to %s.",
			legacy,
			replacement,
		),
	}
}
