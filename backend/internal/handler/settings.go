package handler

import (
	"errors"
	"io"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type SettingsHandler struct {
	settingsService service.SettingsService
}

func NewSettingsHandler(settingsService service.SettingsService) *SettingsHandler {
	return &SettingsHandler{settingsService: settingsService}
}

func (h *SettingsHandler) RegisterRoutes(r chi.Router) {
	r.Get("/drive-names", h.GetDriveNames)
	r.Put("/drive-names", h.SetDriveName)
	r.Delete("/drive-names/{mountPoint}", h.DeleteDriveName)
	r.Get("/wallpaper", h.GetWallpaper)
	r.Put("/wallpaper", h.SetWallpaper)
	r.Patch("/wallpaper", h.SetWallpaperDisplay)
	r.Delete("/wallpaper", h.DeleteWallpaper)
}

func (h *SettingsHandler) GetDriveNames(w http.ResponseWriter, r *http.Request) {
	names, err := h.settingsService.GetDriveNames()
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	mappings := make([]model.DriveNameMapping, 0, len(names))
	for mountPoint, customName := range names {
		mappings = append(mappings, model.DriveNameMapping{
			MountPoint: mountPoint,
			CustomName: customName,
		})
	}

	response := model.DriveNamesResponse{
		Mappings: mappings,
	}

	writeJSON(w, response, http.StatusOK)
}

func (h *SettingsHandler) SetDriveName(w http.ResponseWriter, r *http.Request) {
	var req model.DriveNamesRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if req.MountPoint == "" {
		writeError(w, "Mount point is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if strings.TrimSpace(req.CustomName) == "" {
		writeError(w, "Custom name is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if err := h.settingsService.SetDriveName(req.MountPoint, strings.TrimSpace(req.CustomName)); err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, map[string]any{"success": true}, http.StatusOK)
}

func (h *SettingsHandler) DeleteDriveName(w http.ResponseWriter, r *http.Request) {
	mountPoint := chi.URLParam(r, "mountPoint")
	if mountPoint == "" {
		writeError(w, "Mount point is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if err := h.settingsService.DeleteDriveName(mountPoint); err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, map[string]any{"success": true}, http.StatusOK)
}

// GetWallpaper describes the caller's stored wallpaper, or 404 when there is none.
// GET /api/v1/settings/wallpaper
func (h *SettingsHandler) GetWallpaper(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	meta, err := h.settingsService.GetWallpaper(username)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, meta, http.StatusOK)
}

// SetWallpaper stores the caller's wallpaper so their share pages can show it.
// PUT /api/v1/settings/wallpaper?mode=cover&frostedGlass=true&source=ref with the raw image as the body.
func (h *SettingsHandler) SetWallpaper(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	image, err := io.ReadAll(http.MaxBytesReader(w, r.Body, config.MaxWallpaperBytes))
	if err != nil {
		var maxBytesErr *http.MaxBytesError
		if errors.As(err, &maxBytesErr) {
			writeError(w, "Wallpaper exceeds 20 MB", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge)
			return
		}
		writeError(w, "Unable to read wallpaper", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	meta, err := h.settingsService.SetWallpaper(username, image, model.WallpaperDisplay{
		Mode:         r.URL.Query().Get("mode"),
		FrostedGlass: r.URL.Query().Get("frostedGlass") == "true",
	}, r.URL.Query().Get("source"))
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, meta, http.StatusOK)
}

// SetWallpaperDisplay changes how the caller's stored wallpaper is drawn.
// PATCH /api/v1/settings/wallpaper
func (h *SettingsHandler) SetWallpaperDisplay(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	var display model.WallpaperDisplay
	if err := decodeJSONBody(w, r, &display); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	meta, err := h.settingsService.SetWallpaperDisplay(username, display)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, meta, http.StatusOK)
}

// DeleteWallpaper removes the caller's stored wallpaper.
// DELETE /api/v1/settings/wallpaper
func (h *SettingsHandler) DeleteWallpaper(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	if err := h.settingsService.DeleteWallpaper(username); err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, map[string]any{"success": true}, http.StatusOK)
}
