package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// SystemHandler handles system-related HTTP requests
type SystemHandler struct {
	systemService service.SystemService
}

// NewSystemHandler creates a new system handler
func NewSystemHandler(systemService service.SystemService) *SystemHandler {
	return &SystemHandler{
		systemService: systemService,
	}
}

// RegisterRoutes registers system routes on the given router
func (h *SystemHandler) RegisterRoutes(r chi.Router) {
	r.Get("/drives", h.GetDrives)
}

// GetDrives returns all mounted filesystems on the system
// GET /api/v1/system/drives
func (h *SystemHandler) GetDrives(w http.ResponseWriter, r *http.Request) {
	drives, err := h.systemService.GetAllDrives(r.Context())
	if err != nil {
		writeError(w, "Failed to get system drives", model.ErrCodeInternalError, http.StatusInternalServerError)
		return
	}

	writeJSON(w, drives, http.StatusOK)
}
