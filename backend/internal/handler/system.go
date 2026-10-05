package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// SystemHandler handles system-related HTTP requests
type SystemHandler struct {
	systemService  service.SystemService
	chunkSizeBytes int64
}

// UploadConfigResponse tells browsers how to split uploads.
type UploadConfigResponse struct {
	ChunkSizeBytes int64 `json:"chunkSizeBytes"`
}

// NewSystemHandler creates a new system handler
func NewSystemHandler(systemService service.SystemService, chunkSizeMB int) *SystemHandler {
	if chunkSizeMB <= 0 {
		chunkSizeMB = config.DefaultChunkSizeMB
	}
	return &SystemHandler{
		systemService:  systemService,
		chunkSizeBytes: int64(chunkSizeMB) * 1024 * 1024,
	}
}

// RegisterRoutes registers system routes on the given router
func (h *SystemHandler) RegisterRoutes(r chi.Router) {
	r.Get("/drives", h.GetDrives)
	r.Get("/upload", h.GetUploadConfig)
}

// GetUploadConfig returns the chunk size browsers should upload with
// GET /api/v1/system/upload
func (h *SystemHandler) GetUploadConfig(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, UploadConfigResponse{ChunkSizeBytes: h.chunkSizeBytes}, http.StatusOK)
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
