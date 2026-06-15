package config

import (
	"fmt"
	"os"
	"strings"

	"github.com/homelab/filemanager/internal/model"
	"github.com/spf13/viper"
)

// Load reads configuration from file and environment variables
func Load(configPath string) (*model.ServerConfig, error) {
	v := viper.New()

	// Set defaults
	v.SetDefault("port", 80)
	v.SetDefault("host", "0.0.0.0")
	v.SetDefault("max_upload_mb", DefaultMaxUploadMB) // 10GB default
	v.SetDefault("chunk_size_mb", 5)                  // 5MB chunks
	v.SetDefault("rate_limit_rps", 10.0)
	v.SetDefault("data_dir", DefaultDataDir)

	if configPath == "" {
		configPath = os.Getenv("CONFIG_PATH")
	}

	// Config file settings
	if configPath != "" {
		v.SetConfigFile(configPath)
	} else {
		v.SetConfigName("config")
		v.SetConfigType("yaml")
		v.AddConfigPath(".")
		v.AddConfigPath("./config")
		v.AddConfigPath("/etc/filemanager")
	}

	// Environment variable settings
	v.SetEnvPrefix("FM")
	v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	v.AutomaticEnv()

	// Read config file
	if err := v.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			return nil, fmt.Errorf("error reading config file: %w", err)
		}
		// Config file not found is okay, we'll use defaults and env vars
	}

	var cfg model.ServerConfig
	if err := v.Unmarshal(&cfg); err != nil {
		return nil, fmt.Errorf("error unmarshaling config: %w", err)
	}

	// Parse FM_USERS_* environment variables into the Users map
	// Viper's AutomaticEnv doesn't handle map types from env vars like FM_USERS_username=password
	if cfg.Users == nil {
		cfg.Users = make(map[string]string)
	}
	for _, env := range os.Environ() {
		if strings.HasPrefix(env, "FM_USERS_") {
			parts := strings.SplitN(env, "=", 2)
			if len(parts) == 2 {
				username := strings.TrimPrefix(parts[0], "FM_USERS_")
				password := parts[1]
				if username != "" && password != "" {
					cfg.Users[username] = password
				}
			}
		}
	}

	if origins := os.Getenv("FM_ALLOWED_ORIGINS"); origins != "" {
		cfg.AllowedOrigins = splitEnvList(origins)
	}

	if err := cfg.Validate(); err != nil {
		return nil, err
	}

	return &cfg, nil
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
