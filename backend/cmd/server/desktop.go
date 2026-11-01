package main

import (
	"context"
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path/filepath"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/handler"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/rs/zerolog/log"
)

const desktopCookie = "boxbox_desktop"

func desktopThumbnails(files service.FileStreamer) (service.ThumbnailService, error) {
	value := os.Getenv("BOXBOX_DESKTOP_THUMBNAIL_COMMAND")
	if value == "" {
		return service.NewThumbnailService(files), nil
	}
	var command []string
	if err := json.Unmarshal([]byte(value), &command); err != nil || len(command) != 2 || !filepath.IsAbs(command[0]) || !filepath.IsAbs(command[1]) {
		return nil, fmt.Errorf("invalid desktop thumbnail command")
	}
	return service.NewThumbnailServiceWithRenderer(files, service.NewDesktopThumbnailRenderer(command)), nil
}

func desktopRoutes(next http.Handler, cfg *model.ServerConfig) http.Handler {
	return desktopRoutesWithChooser(next, cfg, nil)
}

func desktopRoutesWithChooser(next http.Handler, cfg *model.ServerConfig, chooser *service.DesktopChooser) http.Handler {
	watch := handler.DesktopWatch(service.NewDesktopWatcher(filesystem.NewOsFS(), cfg.MountPoints))
	apps := handler.NewDesktopAppsHandler(service.NewDesktopAppsService(filesystem.NewOsFS(), cfg.MountPoints, nil))
	clipboard := handler.NewDesktopClipboardHandler(service.NewDesktopClipboard(filesystem.NewOsFS(), cfg.MountPoints))
	trash := handler.NewDesktopTrashHandler(service.NewDesktopTrashService(filesystem.NewOsFS(), cfg.MountPoints))
	volumes := handler.NewDesktopVolumesHandler(service.NewDesktopVolumesService(cfg.MountPoints))
	preferences := service.NewDesktopPreferences(filesystem.NewOsFS(), cfg.DataDir)
	settings := service.NewSettingsService(filesystem.NewOsFS(), service.SettingsServiceConfig{DataDir: cfg.DataDir})
	if err := preferences.PruneWallpapers(settings); err != nil {
		log.Warn().Err(err).Msg("Could not remove unused desktop wallpapers")
	}
	servers, serverErr := service.NewDesktopServers(filesystem.NewOsFS(), cfg.DataDir, nil)
	router := chi.NewRouter()
	router.Route("/api/v1/desktop", func(r chi.Router) {
		r.Get("/watch", watch)
		if chooser != nil {
			handler.RegisterDesktopChooser(r, chooser)
		}
		handler.RegisterDesktopPreferences(r, preferences, settings)
		apps.RegisterRoutes(r)
		clipboard.RegisterRoutes(r)
		volumes.RegisterRoutes(r)
		trash.RegisterRoutes(r)
		if serverErr == nil {
			handler.NewDesktopServersHandler(servers).RegisterRoutes(r)
			handler.NewDesktopTransfersHandler(service.NewDesktopTransfers(filesystem.NewOsFS(), cfg.MountPoints, servers, cfg.DataDir+"/desktop/transfers")).RegisterRoutes(r)
		} else {
			r.HandleFunc("/servers", handler.DesktopGatewayUnavailable)
			r.HandleFunc("/servers/*", handler.DesktopGatewayUnavailable)
			r.HandleFunc("/transfers", handler.DesktopGatewayUnavailable)
			r.HandleFunc("/transfers/*", handler.DesktopGatewayUnavailable)
		}
	})
	router.NotFound(next.ServeHTTP)
	return router
}

func desktopToken() (string, error) {
	token := os.Getenv("BOXBOX_DESKTOP_TOKEN")
	if len(token) != 64 {
		return "", fmt.Errorf("desktop mode requires a 64-character BOXBOX_DESKTOP_TOKEN")
	}
	return token, nil
}

// The cookie gates static files, API calls and WebSocket upgrades alike.
// Host and Origin checks prevent rebinding and cross-origin filesystem access.
func desktopHandler(next http.Handler, address, token string) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("Referrer-Policy", "no-referrer")
		if r.Host != address || (r.Header.Get("Origin") != "" && r.Header.Get("Origin") != "http://"+address) {
			http.Error(w, "Forbidden", http.StatusForbidden)
			return
		}
		if r.URL.Path == "/_desktop" && r.Method == http.MethodGet {
			if subtle.ConstantTimeCompare([]byte(r.URL.Query().Get("token")), []byte(token)) != 1 {
				http.Error(w, "Unauthorized", http.StatusUnauthorized)
				return
			}
			http.SetCookie(w, &http.Cookie{
				Name: desktopCookie, Value: token, Path: "/", HttpOnly: true,
				SameSite: http.SameSiteStrictMode,
			})
			destination := "/"
			if next, err := url.ParseRequestURI(r.URL.Query().Get("next")); err == nil && (next.Path == "/browse" || next.Path == "/chooser") && !next.IsAbs() && next.Host == "" {
				destination = next.String()
			}
			http.Redirect(w, r, destination, http.StatusSeeOther)
			return
		}
		cookie, err := r.Cookie(desktopCookie)
		if err != nil || subtle.ConstantTimeCompare([]byte(cookie.Value), []byte(token)) != 1 {
			http.Error(w, "Unauthorized", http.StatusUnauthorized)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func startDesktopChooser(ctx context.Context, cfg *model.ServerConfig) (*service.DesktopChooser, func()) {
	chooser := service.NewDesktopChooser(filesystem.NewOsFS(), cfg.MountPoints)
	close, err := chooser.StartPortal(ctx)
	if err != nil {
		log.Warn().Err(err).Msg("File chooser portal unavailable")
		return chooser, func() { chooser.Close() }
	}
	return chooser, close
}
