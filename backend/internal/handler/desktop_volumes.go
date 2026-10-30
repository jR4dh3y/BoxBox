package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// DesktopVolumesHandler exposes removable-drive state and GIO actions to the
// private desktop UI. It is not registered for remote or web servers.
type DesktopVolumesHandler struct {
	volumes service.DesktopVolumesService
}

func NewDesktopVolumesHandler(volumes service.DesktopVolumesService) *DesktopVolumesHandler {
	return &DesktopVolumesHandler{volumes: volumes}
}

// RegisterRoutes adds /volumes below the caller's desktop route group.
func (h *DesktopVolumesHandler) RegisterRoutes(r chi.Router) {
	r.Get("/volumes", h.ListVolumes)
	r.Post("/volumes/{volumeID}/mount", h.action(service.DesktopVolumeActionMount))
	r.Post("/volumes/{volumeID}/unmount", h.action(service.DesktopVolumeActionUnmount))
	r.Post("/volumes/{volumeID}/eject", h.action(service.DesktopVolumeActionEject))
}

// ListVolumes returns the current removable volumes.
// GET /api/v1/desktop/volumes
func (h *DesktopVolumesHandler) ListVolumes(w http.ResponseWriter, r *http.Request) {
	response, err := h.volumes.ListVolumes(r.Context())
	if err != nil {
		writeError(w, "Could not read removable drives", model.ErrCodeInternalError, http.StatusInternalServerError)
		return
	}
	w.Header().Set("Cache-Control", "no-store")
	writeJSON(w, response, http.StatusOK)
}

func (h *DesktopVolumesHandler) action(action service.DesktopVolumeAction) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		volumeID := chi.URLParam(r, "volumeID")
		var (
			response service.DesktopVolumesResponse
			err      error
		)
		switch action {
		case service.DesktopVolumeActionMount:
			response, err = h.volumes.MountVolume(r.Context(), volumeID)
		case service.DesktopVolumeActionUnmount:
			response, err = h.volumes.UnmountVolume(r.Context(), volumeID)
		case service.DesktopVolumeActionEject:
			response, err = h.volumes.EjectVolume(r.Context(), volumeID)
		default:
			writeError(w, "Unsupported volume action", model.ErrCodeValidationError, http.StatusBadRequest)
			return
		}
		if err != nil {
			writeDesktopVolumeError(w, err)
			return
		}
		w.Header().Set("Cache-Control", "no-store")
		writeJSON(w, response, http.StatusOK)
	}
}
