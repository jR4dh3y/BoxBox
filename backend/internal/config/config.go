package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
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
	v.SetDefault("port", 8080)
	v.SetDefault("host", "0.0.0.0")
	v.SetDefault("max_upload_mb", DefaultMaxUploadMB) // 10GB default
	v.SetDefault("chunk_size_mb", DefaultChunkSizeMB)
	v.SetDefault("rate_limit_rps", 2.0)
	v.SetDefault("data_dir", DefaultDataDir)
	v.SetDefault("nas.service_uid", DefaultNASServiceID)
	v.SetDefault("nas.service_gid", DefaultNASServiceID)

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
		"nas.service_uid",
		"nas.service_gid",
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
	cfg.MountPoints = model.ClassifyMountPoints(cfg.MountPoints)

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

	if err := applyNASIdentityEnv(&cfg); err != nil {
		return nil, err
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

	// Resolve once before filesystem adapters interpret persistence paths.
	if cfg.DataDir == "" {
		cfg.DataDir = DefaultDataDir
	}
	dataDir, err := filepath.Abs(cfg.DataDir)
	if err != nil {
		return nil, fmt.Errorf("resolve data directory: %w", err)
	}
	resolvedDataDir, err := filepath.EvalSymlinks(dataDir)
	if err != nil {
		if !os.IsNotExist(err) {
			return nil, fmt.Errorf("resolve data directory: %w", err)
		}
		resolvedDataDir = dataDir
	}
	if filepath.Dir(resolvedDataDir) == resolvedDataDir {
		return nil, fmt.Errorf("data_dir must not be a filesystem root; use a dedicated directory")
	}
	cfg.DataDir = dataDir

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

// applyNASIdentityEnv reads BOXBOX_NAS_IDENTITY_<user>=uid:gid[:group,group].
func applyNASIdentityEnv(cfg *model.ServerConfig) error {
	const prefix = "BOXBOX_NAS_IDENTITY_"
	for _, env := range os.Environ() {
		name, value, ok := strings.Cut(env, "=")
		if !ok || !strings.HasPrefix(name, prefix) {
			continue
		}
		username := strings.TrimPrefix(name, prefix)
		identity, err := parseNASIdentity(value)
		if username == "" || err != nil {
			return fmt.Errorf("%s must be uid:gid or uid:gid:group,group: %v", name, err)
		}
		if cfg.NAS.Identities == nil {
			cfg.NAS.Identities = make(map[string]model.NASIdentity)
		}
		cfg.NAS.Identities[username] = identity
	}
	return nil
}

func parseNASIdentity(value string) (model.NASIdentity, error) {
	parts := strings.Split(value, ":")
	if len(parts) < 2 || len(parts) > 3 {
		return model.NASIdentity{}, fmt.Errorf("got %q", value)
	}
	ids := make([]uint32, 0, 2)
	for _, part := range parts[:2] {
		id, err := strconv.ParseUint(part, 10, 32)
		if err != nil {
			return model.NASIdentity{}, err
		}
		ids = append(ids, uint32(id))
	}
	identity := model.NASIdentity{UID: ids[0], GID: ids[1]}
	if len(parts) == 3 && parts[2] != "" {
		for _, part := range strings.Split(parts[2], ",") {
			group, err := strconv.ParseUint(strings.TrimSpace(part), 10, 32)
			if err != nil {
				return model.NASIdentity{}, err
			}
			identity.Groups = append(identity.Groups, uint32(group))
		}
	}
	return identity, nil
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
