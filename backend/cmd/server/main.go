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
	"github.com/jR4dh3y/BoxBox/backend/internal/nas"
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
	nasAPI := flag.Bool(nas.APIFlag, false, "Internal: run as the NAS-mode API under the supervisor")
	nasWorker := flag.Bool(nas.WorkerFlag, false, "Internal: run as a NAS-mode file worker")
	flag.Parse()

	// A worker reads no config and no environment; it only serves files.
	if *nasWorker {
		if err := nas.RunWorker(); err != nil {
			fmt.Fprintln(os.Stderr, "NAS worker:", err)
			os.Exit(1)
		}
		return
	}

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

	var workers *nas.Workers
	switch {
	case cfg.NAS.Enabled() && (*devMode || *desktopMode):
		log.Fatal().Msg("NAS identities cannot be used with --dev or --desktop")
	case cfg.NAS.Enabled() && !*nasAPI:
		os.Exit(nas.RunSupervisor(cfg.NAS, os.Args[1:], config.ShutdownTimeout+5*time.Second))
	case cfg.NAS.Enabled():
		workers, err = nas.ConnectToSupervisor(cfg.NAS)
		if err != nil {
			log.Fatal().Err(err).Msg("Failed to start NAS mode")
		}
		log.Info().Strs("accounts", workers.Accounts()).Msg("NAS mode: file access runs as each account's identity")
	case *nasAPI:
		log.Fatal().Msg("--nas-api needs NAS identities in the configuration")
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
	app, err := initializeServer(cfg, localIdentity, *desktopMode, workers)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize server")
	}
	server := app.server
	app.start(ctx)
	log.Info().Msg("WebSocket hub, jobs, and auth and upload cleanup started")

	// Ensure data directory exists for settings storage
	if err := os.MkdirAll(cfg.DataDir, 0755); err != nil {
		log.Warn().Err(err).Str("path", cfg.DataDir).Msg("Could not create data directory, settings may not persist")
	} else {
		log.Info().Str("path", cfg.DataDir).Msg("Data directory created/verified")
	}

	// Cancel long-lived event streams before waiting for HTTP shutdown.
	server.BaseContext = func(net.Listener) context.Context { return ctx }
	// Bind before announcing readiness so the desktop app never guesses a port.
	listener, err := net.Listen("tcp", server.Addr)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to listen")
	}
	if *desktopMode {
		chooser, closeChooser := startDesktopChooser(ctx, cfg)
		defer closeChooser()
		server.Handler = desktopHandler(desktopRoutesWithChooser(server.Handler, cfg, chooser), listener.Addr().String(), token)
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
	waitForShutdown(cancel, app, *desktopMode, workers != nil)
}

// app is the assembled server and the background work it owns.
type app struct {
	server *http.Server
	hub    *websocket.Hub
	auth   service.AuthService
	stacks fileStacks
}

func (a *app) start(ctx context.Context) {
	go a.hub.Run(ctx)
	for _, stack := range a.stacks.all() {
		stack.start(ctx)
	}
	a.auth.StartCleanup(ctx)
}

// stopJobs stops the job workers, which must end before the HTTP server.
func (a *app) stopJobs() {
	for _, stack := range a.stacks.all() {
		stack.jobs.Stop()
	}
}

func (a *app) stopCleanups() {
	a.auth.StopCleanup()
	for _, stack := range a.stacks.all() {
		stack.streams.StopCleanup()
	}
}

// initializeServer creates and configures all server components. With
// workers, each mapped account reaches user files only through its worker.
func initializeServer(cfg *model.ServerConfig, devMode, desktopMode bool, workers *nas.Workers) (*app, error) {
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
		return nil, fmt.Errorf("initialize auth service: %w", err)
	}
	var connectService *service.ConnectService
	if !devMode {
		connectService, err = service.NewConnectService(fs, authService, service.ConnectServiceConfig{DataDir: cfg.DataDir, Users: cfg.Users})
		if err != nil {
			return nil, err
		}
	}

	var stacks fileStacks
	thumbnailLimit := service.NewThumbnailLimit()
	var userFiles func(account string) (filesystem.FS, error)
	if workers == nil {
		stacks.shared, err = newFileStack(fs, mountPoints, hub, cfg, desktopMode, thumbnailLimit)
		if err != nil {
			return nil, err
		}
	} else {
		stacks.byAccount = make(map[string]*fileStack)
		for _, account := range workers.Accounts() {
			accountFS, _ := workers.FS(account)
			stacks.byAccount[account], err = newFileStack(accountFS, mountPoints, hub, cfg, desktopMode, thumbnailLimit)
			if err != nil {
				return nil, err
			}
		}
		userFiles = func(account string) (filesystem.FS, error) {
			if accountFS, ok := workers.FS(account); ok {
				return accountFS, nil
			}
			return nil, service.ErrNoFileAccess
		}
	}

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
		Files:          userFiles,
	})

	// Create handlers
	authHandler := handler.NewAuthHandler(authService)
	wsHandler := handler.NewWebSocketHandler(hub, authService, cfg.AllowedOrigins)
	wsHandler.SetDevMode(devMode)
	systemHandler := handler.NewSystemHandler(systemService, cfg.ChunkSizeMB)
	settingsHandler := handler.NewSettingsHandler(settingsService)
	shareHandler := handler.NewShareHandler(shareService, settingsService, cfg.MaxUploadMB)

	// Create router
	router := createRouter(cfg, devMode, authService, authHandler, stacks, wsHandler, systemHandler, settingsHandler, shareHandler, desktopMode, connectService)

	// Create HTTP server
	server := &http.Server{
		Addr:         fmt.Sprintf("%s:%d", cfg.Host, cfg.Port),
		Handler:      router,
		ReadTimeout:  config.HTTPReadTimeout,
		WriteTimeout: config.HTTPWriteTimeout,
		IdleTimeout:  config.HTTPIdleTimeout,
	}

	return &app{server: server, hub: hub, auth: authService, stacks: stacks}, nil
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
	stacks fileStacks,
	wsHandler *handler.WebSocketHandler,
	systemHandler *handler.SystemHandler,
	settingsHandler *handler.SettingsHandler,
	shareHandler *handler.ShareHandler,
	desktopMode bool,
	connectService *service.ConnectService,
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
			r.Use(middleware.RejectClientCredentials)
			r.Use(middleware.RateLimit(cfg.RateLimitRPS, cfg.TrustedProxies...))
			authHandler.RegisterRoutes(r)
		})
		if connectService != nil {
			connectHandler := handler.NewConnectHandler(connectService)
			r.Route("/connect", func(r chi.Router) {
				connectHandler.RegisterPublicRoutes(r, middleware.RateLimit(0.5, cfg.TrustedProxies...))
				r.Group(func(r chi.Router) {
					r.Use(middleware.JWTAuth(authService, connectService))
					r.Use(middleware.RateLimit(cfg.RateLimitRPS, cfg.TrustedProxies...))
					connectHandler.RegisterRoutes(r)
				})
			})
		}

		// Public share-link routes (the share token is the only credential)
		r.Route("/share", func(r chi.Router) {
			r.Use(middleware.RateLimit(shareRateLimitRPS, cfg.TrustedProxies...))
			shareHandler.RegisterPublicRoutes(r)
		})

		// Protected routes (auth required)
		r.Group(func(r chi.Router) {
			if devMode {
				r.Use(middleware.RejectClientCredentials)
				r.Use(middleware.DevelopmentAuth)
			} else {
				r.Use(middleware.JWTAuth(authService, connectService))
			}

			// User-file routes use the signed-in account's file stack.
			r.Mount("/files", stacks.route(func(s *fileStack) http.Handler { return s.files }))
			r.Mount("/stream", stacks.route(func(s *fileStack) http.Handler { return s.stream }))
			r.Mount("/search", stacks.route(func(s *fileStack) http.Handler { return s.search }))
			r.Mount("/jobs", stacks.route(func(s *fileStack) http.Handler { return s.job }))

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
		r.With(middleware.RejectClientCredentials).Get("/ws", wsHandler.ServeWS)
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
func waitForShutdown(cancel context.CancelFunc, app *app, desktopMode, nasMode bool) {
	// Create channel to receive OS signals
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)
	if nasMode {
		// After Notify, so a stop requested during startup is still graceful.
		nas.WatchShutdown(sigCh)
	}
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
	app.stopJobs()

	// Shutdown HTTP server
	log.Info().Msg("Shutting down HTTP server...")
	if err := app.server.Shutdown(shutdownCtx); err != nil {
		log.Error().Err(err).Msg("Error during server shutdown")
	}

	log.Info().Msg("Server shutdown complete")

	// Stop background cleanups
	app.stopCleanups()
}
