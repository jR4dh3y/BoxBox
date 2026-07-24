package handler

import (
	"encoding/json"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
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
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
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
