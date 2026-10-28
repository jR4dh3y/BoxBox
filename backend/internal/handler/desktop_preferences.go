package handler

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func RegisterDesktopPreferences(r chi.Router, preferences *service.DesktopPreferences, settings service.SettingsService) {
	serveWallpaper := func(w http.ResponseWriter, r *http.Request) {
		username := "dev"
		if id := chi.URLParam(r, "id"); id != "" {
			staged, ok := service.DesktopWallpaperUser(id)
			if !ok {
				writeError(w, "Invalid wallpaper reference.", model.ErrCodeValidationError, http.StatusBadRequest)
				return
			}
			username = staged
		}
		image, meta, err := settings.OpenWallpaper(username)
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		w.Header().Set("Content-Type", meta.ContentType)
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Content-Security-Policy", "sandbox; default-src 'none'")
		w.Write(image)
	}
	r.Get("/wallpaper", serveWallpaper)
	r.Get("/wallpaper/{id}", serveWallpaper)
	r.Put("/wallpaper", func(w http.ResponseWriter, r *http.Request) {
		image, err := io.ReadAll(http.MaxBytesReader(w, r.Body, config.MaxWallpaperBytes))
		if err != nil {
			writeError(w, "Wallpaper exceeds the size limit.", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge)
			return
		}
		// Staging never replaces an image referenced by committed preferences.
		digest := sha256.Sum256(image)
		username, _ := service.DesktopWallpaperUser(hex.EncodeToString(digest[:]))
		meta, err := settings.SetWallpaper(username, image, model.WallpaperDisplay{Mode: "cover"}, "desktop")
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		writeJSON(w, meta, http.StatusOK)
	})
	r.Get("/preferences", func(w http.ResponseWriter, r *http.Request) {
		value, err := preferences.Get()
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		writeJSON(w, value, http.StatusOK)
	})
	r.Put("/preferences", func(w http.ResponseWriter, r *http.Request) {
		value, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 128<<10))
		if err != nil {
			writeError(w, "Invalid desktop preferences.", model.ErrCodeValidationError, http.StatusBadRequest)
			return
		}
		if err = preferences.Set(value); err != nil {
			HandleServiceError(w, err)
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
}
