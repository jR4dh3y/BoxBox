// Package main provides the entry point for the homelab file manager server.
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

	"github.com/homelab/filemanager/internal/config"
	"github.com/homelab/filemanager/internal/handler"
	"github.com/homelab/filemanager/internal/middleware"
	"github.com/homelab/filemanager/internal/model"
	"github.com/homelab/filemanager/internal/pkg/filesystem"
	"github.com/homelab/filemanager/internal/service"
	"github.com/homelab/filemanager/internal/static"
	"github.com/homelab/filemanager/internal/websocket"
)

func main() {
	// Parse command line flags
	configPath := flag.String("config", "", "Path to configuration file")
	flag.Parse()

	// Configure zerolog
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnix
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr, TimeFormat: time.RFC3339})

	// Load configuration
	cfg, err := config.Load(*configPath)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to load configuration")
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
	server, hub, jobService, authService, streamHandler, err := initializeServer(cfg)
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
func initializeServer(cfg *model.ServerConfig) (*http.Server, *websocket.Hub, service.JobService, service.AuthService, *handler.StreamHandler, error) {
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

	// Create handlers
	authHandler := handler.NewAuthHandler(authService)
	fileHandler := handler.NewFileHandler(fileService)
	streamHandler := handler.NewStreamHandler(fileService, cfg.ChunkSizeMB, cfg.MaxUploadMB)
	jobHandler := handler.NewJobHandler(jobService)
	searchHandler := handler.NewSearchHandler(searchService)
	wsHandler := handler.NewWebSocketHandler(hub, authService, cfg.AllowedOrigins)
	systemHandler := handler.NewSystemHandler(systemService)
	settingsHandler := handler.NewSettingsHandler(settingsService)

	// Create router
	router := createRouter(cfg, authService, authHandler, fileHandler, streamHandler, jobHandler, searchHandler, wsHandler, systemHandler, settingsHandler, mountPoints)

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

// createRouter sets up chi router with all routes and middleware
func createRouter(
	cfg *model.ServerConfig,
	authService service.AuthService,
	authHandler *handler.AuthHandler,
	fileHandler *handler.FileHandler,
	streamHandler *handler.StreamHandler,
	jobHandler *handler.JobHandler,
	searchHandler *handler.SearchHandler,
	wsHandler *handler.WebSocketHandler,
	systemHandler *handler.SystemHandler,
	settingsHandler *handler.SettingsHandler,
	mountPoints []model.MountPoint,
) chi.Router {
	r := chi.NewRouter()

	// Global middleware
	r.Use(chimiddleware.RequestID)
	r.Use(chimiddleware.Logger)
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
			r.Use(middleware.RateLimit(cfg.RateLimitRPS))
			authHandler.RegisterRoutes(r)
		})

		// Protected routes (auth required)
		r.Group(func(r chi.Router) {
			r.Use(middleware.JWTAuth(authService))

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
		})

		// WebSocket endpoint (auth handled in handler)
		r.Get("/ws", wsHandler.ServeWS)
	})

	// Static file handler for SPA frontend (catch-all)
	// This must be after all API routes
	staticHandler, err := static.NewHandler()
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
