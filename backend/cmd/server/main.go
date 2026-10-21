// Package main provides the entry point for the BoxBox server.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/go-chi/chi/v5"
	chimiddleware "github.com/go-chi/chi/v5/middleware"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"

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
	desktopMode := flag.Bool("desktop", false, "Run a private loopback server for the desktop app")
	flag.Parse()

	var token string
	if *desktopMode {
		var err error
		token, err = desktopToken()
		if err != nil {
			log.Fatal().Err(err).Msg("Invalid desktop launch")
		}
	}
	localIdentity := *devMode || *desktopMode
	if localIdentity {
		if err := setDevCredentials(); err != nil {
			log.Fatal().Err(err).Msg("Failed to initialize development credentials")
		}
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
	if *desktopMode {
		cfg.Host = "127.0.0.1"
		cfg.Port = 0
		cfg.AllowedOrigins = nil
		cfg.TrustedProxies = nil
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
	server, hub, jobService, authService, streamHandler, err := initializeServer(cfg, localIdentity, *desktopMode)
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

	// Bind before announcing readiness so the desktop app never guesses a port.
	listener, err := net.Listen("tcp", server.Addr)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to listen")
	}
	if *desktopMode {
		server.Handler = desktopHandler(desktopRoutes(server.Handler, cfg), listener.Addr().String(), token)
	}
	// Start HTTP server in background
	go func() {
		addr := fmt.Sprintf("%s:%d", cfg.Host, cfg.Port)
		log.Info().Str("addr", addr).Msg("Starting HTTP server")
		if err := server.Serve(listener); err != nil && err != http.ErrServerClosed {
			log.Fatal().Err(err).Msg("HTTP server error")
		}
	}()
	if *desktopMode {
		if err := json.NewEncoder(os.Stdout).Encode(map[string]string{"url": "http://" + listener.Addr().String()}); err != nil {
			log.Fatal().Err(err).Msg("Failed to announce desktop server")
		}
	}

	// Wait for shutdown signal
	waitForShutdown(cancel, server, jobService, authService, streamHandler, *desktopMode)
}

// initializeServer creates and configures all server components
func initializeServer(cfg *model.ServerConfig, devMode, desktopMode bool) (*http.Server, *websocket.Hub, service.JobService, service.AuthService, *handler.StreamHandler, error) {
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

	configuredMountPoints := cfg.MountPoints
	mountPoints := service.DiscoverMountPoints(fs, configuredMountPoints)

	// Create WebSocket hub
	hub := websocket.NewHub()

	// Create services
	authService, err := service.NewAuthService(fs, service.AuthServiceConfig{
		DataDir:   cfg.DataDir,
		JWTSecret: cfg.JWTSecret,
		Users:     cfg.Users,
	})
	if err != nil {
		return nil, nil, nil, nil, nil, fmt.Errorf("initialize auth service: %w", err)
	}

	fileConfig := service.FileServiceConfig{MountPoints: mountPoints}
	if desktopMode {
		fileConfig.DirectoryEntryLimit = -1
	}
	fileService := service.NewFileService(fs, fileConfig)

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

	// Keep configured names for existing shares and discovered names for shares
	// created from the drives shown in the browser.
	shareMountPoints := make([]model.MountPoint, 0, len(configuredMountPoints)+len(mountPoints))
	shareMountPoints = append(shareMountPoints, configuredMountPoints...)
	shareMountPoints = append(shareMountPoints, mountPoints...)
	shareService := service.NewShareService(fs, service.ShareServiceConfig{
		DataDir:        cfg.DataDir,
		MaxUploadBytes: int64(cfg.MaxUploadMB) * 1024 * 1024,
		Mounts:         func() []model.MountPoint { return shareMountPoints },
	})

	// Create handlers
	authHandler := handler.NewAuthHandler(authService)
	fileHandler := handler.NewFileHandler(fileService)
	streamHandler := handler.NewStreamHandler(fileService, cfg.MaxUploadMB)
	jobHandler := handler.NewJobHandler(jobService)
	searchHandler := handler.NewSearchHandler(searchService)
	wsHandler := handler.NewWebSocketHandler(hub, authService, cfg.AllowedOrigins)
	wsHandler.SetDevMode(devMode)
	systemHandler := handler.NewSystemHandler(systemService, cfg.ChunkSizeMB)
	settingsHandler := handler.NewSettingsHandler(settingsService)
	shareHandler := handler.NewShareHandler(shareService, settingsService, cfg.MaxUploadMB)

	// Create router
	router := createRouter(cfg, devMode, authService, authHandler, fileHandler, streamHandler, jobHandler, searchHandler, wsHandler, systemHandler, settingsHandler, shareHandler, mountPoints, desktopMode)

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
	desktopMode bool,
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
	staticHandler, err := static.NewHandler(devMode, desktopMode)
	if err != nil {
		log.Warn().Err(err).Msg("Static handler not available, frontend will not be served")
	} else {
		r.NotFound(staticHandler.ServeHTTP)
		log.Info().Msg("Static file handler initialized for SPA frontend")
	}

	return r
}

// waitForShutdown handles graceful shutdown on interrupt signals
func waitForShutdown(cancel context.CancelFunc, server *http.Server, jobService service.JobService, authService service.AuthService, streamHandler *handler.StreamHandler, desktopMode bool) {
	// Create channel to receive OS signals
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)
	if desktopMode {
		// The desktop parent keeps stdin open; EOF also handles a crashed parent.
		go func() {
			_, _ = io.Copy(io.Discard, os.Stdin)
			sigCh <- syscall.SIGTERM
		}()
	}

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
