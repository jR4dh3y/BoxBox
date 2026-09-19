// Package main provides the entry point for the BoxBox server.
package main

import (
	"context"
	"flag"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/go-chi/chi/v5"
	chimiddleware "github.com/go-chi/chi/v5/middleware"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"golang.org/x/crypto/bcrypt"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/handler"
	"github.com/jR4dh3y/BoxBox/backend/internal/middleware"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/jR4dh3y/BoxBox/backend/internal/static"
	"github.com/jR4dh3y/BoxBox/backend/internal/websocket"
)

func main() {
	// Parse command line flags
	configPath := flag.String("config", "", "Path to configuration file")
	devMode := flag.Bool("dev", false, "Run locally without authentication (forces host to 127.0.0.1)")
	flag.Parse()

	if *devMode {
		// These values exist only in this process and satisfy production-oriented
		// config validation. Authentication is bypassed below.
		os.Setenv("BOXBOX_JWT_SECRET", "boxbox-development-mode-not-for-production")
		// Authentication is bypassed, but configuration still requires the
		// bcrypt storage format used in production.
		devPasswordHash, hashErr := bcrypt.GenerateFromPassword([]byte("development-mode"), bcrypt.MinCost)
		if hashErr != nil {
			log.Fatal().Err(hashErr).Msg("Failed to initialize development credentials")
		}
		os.Setenv("BOXBOX_USERS_dev", string(devPasswordHash))
	}

	// Configure zerolog
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnix
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr, TimeFormat: time.RFC3339})

	// Load configuration
	loadResult, err := config.LoadWithReport(*configPath)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to load configuration")
	}
	cfg := loadResult.Config
	if *devMode {
		cfg.Host = "127.0.0.1"
		log.Warn().Msg("DEVELOPMENT MODE: authentication disabled; listening on loopback only")
	}

	for _, warning := range loadResult.Warnings {
		log.Warn().
			Str("legacy", warning.Legacy).
			Str("replacement", warning.Replacement).
			Msg(warning.Message)
	}

	log.Info().
		Int("port", cfg.Port).
		Str("host", cfg.Host).
		Int("mount_points", len(cfg.MountPoints)).
		Msg("Configuration loaded")

	// Create context that listens for shutdown signals
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Initialize components
	server, hub, jobService, authService, streamHandler, err := initializeServer(cfg, *devMode)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize server")
	}

	// Start WebSocket hub in background
	go hub.Run(ctx)
	log.Info().Msg("WebSocket hub started")

	// Start job service workers
	jobService.Start(ctx)
	log.Info().Msg("Job service started")

	// Start auth service cleanup
	authService.StartCleanup(ctx)
	log.Info().Msg("Auth service cleanup started")

	// Start upload session cleanup
	streamHandler.StartCleanup(ctx)
	log.Info().Msg("Upload session cleanup started")

	// Ensure data directory exists for settings storage
	if err := os.MkdirAll(cfg.DataDir, 0755); err != nil {
		log.Warn().Err(err).Str("path", cfg.DataDir).Msg("Could not create data directory, settings may not persist")
	} else {
		log.Info().Str("path", cfg.DataDir).Msg("Data directory created/verified")
	}

	// Start HTTP server in background
	go func() {
		addr := fmt.Sprintf("%s:%d", cfg.Host, cfg.Port)
		log.Info().Str("addr", addr).Msg("Starting HTTP server")
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatal().Err(err).Msg("HTTP server error")
		}
	}()

	// Wait for shutdown signal
	waitForShutdown(cancel, server, jobService, authService, streamHandler)
}

// initializeServer creates and configures all server components
func initializeServer(cfg *model.ServerConfig, devMode bool) (*http.Server, *websocket.Hub, service.JobService, service.AuthService, *handler.StreamHandler, error) {
	// Create filesystem abstraction (using real OS filesystem)
	fs := filesystem.NewOsFS()

	// Ensure mount point directories exist
	for _, mp := range cfg.MountPoints {
		exists, err := fs.Exists(mp.Path)
		if err != nil {
			log.Warn().Err(err).Str("path", mp.Path).Str("name", mp.Name).Msg("Error checking mount point")
			continue
		}
		if !exists {
			log.Warn().Str("path", mp.Path).Str("name", mp.Name).Msg("Mount point directory does not exist")
		} else {
			log.Info().Str("path", mp.Path).Str("name", mp.Name).Bool("read_only", mp.ReadOnly).Msg("Mount point configured")
		}
	}

	// Convert config mount points to model mount points
	mountPoints := make([]model.MountPoint, len(cfg.MountPoints))
	for i, mp := range cfg.MountPoints {
		mountPoints[i] = model.MountPoint{
			Name:     mp.Name,
			Path:     mp.Path,
			ReadOnly: mp.ReadOnly,
		}
	}

	// Create WebSocket hub
	hub := websocket.NewHub()

	// Create services
	authService := service.NewAuthService(service.AuthServiceConfig{
		JWTSecret: cfg.JWTSecret,
		Users:     cfg.Users,
	})

	fileService := service.NewFileService(fs, service.FileServiceConfig{
		MountPoints: mountPoints,
	})

	searchService := service.NewSearchService(fs, service.SearchServiceConfig{
		MountPoints: mountPoints,
	})

	jobService := service.NewJobService(fs, hub, service.JobServiceConfig{
		Workers:     config.DefaultJobWorkers,
		MountPoints: mountPoints,
	})

	systemService := service.NewSystemService()

	settingsService := service.NewSettingsService(fs, service.SettingsServiceConfig{
		DataDir: cfg.DataDir,
	})

	// Shares re-resolve their target against the live mount list on every
	// recipient access, so they receive a mounts provider instead of a snapshot.
	// Deliberately the configured list, not discovered sub-mounts: discovery
	// replaces auto-discover parents with sub-mount names, which would break
	// every share whose path is rooted at a configured name like "drives".
	shareService := service.NewShareService(fs, service.ShareServiceConfig{
		DataDir:        cfg.DataDir,
		MaxUploadBytes: int64(cfg.MaxUploadMB) * 1024 * 1024,
		Mounts:         func() []model.MountPoint { return mountPoints },
	})

	// Create handlers
	authHandler := handler.NewAuthHandler(authService)
	fileHandler := handler.NewFileHandler(fileService)
	streamHandler := handler.NewStreamHandler(fileService, cfg.ChunkSizeMB, cfg.MaxUploadMB)
	jobHandler := handler.NewJobHandler(jobService)
	searchHandler := handler.NewSearchHandler(searchService)
	wsHandler := handler.NewWebSocketHandler(hub, authService, cfg.AllowedOrigins)
	wsHandler.SetDevMode(devMode)
	systemHandler := handler.NewSystemHandler(systemService)
	settingsHandler := handler.NewSettingsHandler(settingsService)
	shareHandler := handler.NewShareHandler(shareService, cfg.MaxUploadMB)

	// Create router
	router := createRouter(cfg, devMode, authService, authHandler, fileHandler, streamHandler, jobHandler, searchHandler, wsHandler, systemHandler, settingsHandler, shareHandler, mountPoints)

	// Create HTTP server
	server := &http.Server{
		Addr:         fmt.Sprintf("%s:%d", cfg.Host, cfg.Port),
		Handler:      router,
		ReadTimeout:  config.HTTPReadTimeout,
		WriteTimeout: config.HTTPWriteTimeout,
		IdleTimeout:  config.HTTPIdleTimeout,
	}

	return server, hub, jobService, authService, streamHandler, nil
}

// shareRateLimitRPS bounds the public share-link endpoints. It is higher than
// the auth default because a single recipient page load fires info, preview,
// and media range requests in quick succession, which a 2 rps budget would
// reject with 429s.
const shareRateLimitRPS = 5.0

// createRouter sets up chi router with all routes and middleware
func createRouter(
	cfg *model.ServerConfig,
	devMode bool,
	authService service.AuthService,
	authHandler *handler.AuthHandler,
	fileHandler *handler.FileHandler,
	streamHandler *handler.StreamHandler,
	jobHandler *handler.JobHandler,
	searchHandler *handler.SearchHandler,
	wsHandler *handler.WebSocketHandler,
	systemHandler *handler.SystemHandler,
	settingsHandler *handler.SettingsHandler,
	shareHandler *handler.ShareHandler,
	mountPoints []model.MountPoint,
) chi.Router {
	r := chi.NewRouter()

	// Global middleware
	r.Use(chimiddleware.RequestID)
	r.Use(middleware.RequestLogger)
	r.Use(chimiddleware.Recoverer)
	r.Use(middleware.SecurityHeaders)

	// Health check endpoint (no auth required)
	r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ok"}`))
	})

	// API routes
	r.Route("/api/v1", func(r chi.Router) {
		// Health check also available under API path
		r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`{"status":"ok"}`))
		})
		// Public routes (no auth required)
		// Auth routes are rate-limited to prevent brute force attacks
		r.Route("/auth", func(r chi.Router) {
			r.Use(middleware.RateLimit(cfg.RateLimitRPS, cfg.TrustedProxies...))
			authHandler.RegisterRoutes(r)
		})

		// Public share-link routes (the share token is the only credential)
		r.Route("/share", func(r chi.Router) {
			r.Use(middleware.RateLimit(shareRateLimitRPS, cfg.TrustedProxies...))
			shareHandler.RegisterPublicRoutes(r)
		})

		// Protected routes (auth required)
		r.Group(func(r chi.Router) {
			if devMode {
				r.Use(middleware.DevelopmentAuth)
			} else {
				r.Use(middleware.JWTAuth(authService))
			}

			// File operations with mount point guard
			r.Route("/files", func(r chi.Router) {
				r.Use(middleware.MountPointGuard(mountPoints))
				fileHandler.RegisterRoutes(r)
			})

			// Streaming operations with mount point guard
			r.Route("/stream", func(r chi.Router) {
				r.Use(middleware.MountPointGuard(mountPoints))
				streamHandler.RegisterRoutes(r)
			})

			// Search operations
			r.Route("/search", func(r chi.Router) {
				searchHandler.RegisterRoutes(r)
			})

			// Job operations
			r.Route("/jobs", func(r chi.Router) {
				jobHandler.RegisterRoutes(r)
			})

			// System operations
			r.Route("/system", func(r chi.Router) {
				systemHandler.RegisterRoutes(r)
			})

			// Settings operations
			r.Route("/settings", func(r chi.Router) {
				settingsHandler.RegisterRoutes(r)
			})

			// Share link management
			r.Route("/shares", func(r chi.Router) {
				shareHandler.RegisterRoutes(r)
			})
		})

		// WebSocket endpoint (auth handled in handler)
		r.Get("/ws", wsHandler.ServeWS)
	})

	// Static file handler for SPA frontend (catch-all)
	// This must be after all API routes
	staticHandler, err := static.NewHandler(devMode)
	if err != nil {
		log.Warn().Err(err).Msg("Static handler not available, frontend will not be served")
	} else {
		r.NotFound(staticHandler.ServeHTTP)
		log.Info().Msg("Static file handler initialized for SPA frontend")
	}

	return r
}

// waitForShutdown handles graceful shutdown on interrupt signals
func waitForShutdown(cancel context.CancelFunc, server *http.Server, jobService service.JobService, authService service.AuthService, streamHandler *handler.StreamHandler) {
	// Create channel to receive OS signals
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)

	// Wait for signal
	sig := <-sigCh
	log.Info().Str("signal", sig.String()).Msg("Received shutdown signal")

	// Cancel context to stop background goroutines
	cancel()

	// Create shutdown context with timeout
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), config.ShutdownTimeout)
	defer shutdownCancel()

	// Stop job service
	log.Info().Msg("Stopping job service...")
	jobService.Stop()

	// Shutdown HTTP server
	log.Info().Msg("Shutting down HTTP server...")
	if err := server.Shutdown(shutdownCtx); err != nil {
		log.Error().Err(err).Msg("Error during server shutdown")
	}

	log.Info().Msg("Server shutdown complete")

	// Stop background cleanups
	authService.StopCleanup()
	streamHandler.StopCleanup()
}
