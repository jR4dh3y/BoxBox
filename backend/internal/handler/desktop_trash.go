package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type DesktopTrashHandler struct {
	trash service.DesktopTrashService
}

type desktopTrashPathRequest struct {
	Path string `json:"path"`
}

type desktopTrashItemRequest struct {
	ID string `json:"id"`
}

type desktopTrashListResponse struct {
	Items []service.DesktopTrashItem `json:"items"`
}

type desktopTrashItemResponse struct {
	Item service.DesktopTrashItem `json:"item"`
}

type desktopTrashActionResponse struct {
	OK bool `json:"ok"`
}

// NewDesktopTrashHandler creates routes for the private desktop Trash panel.
func NewDesktopTrashHandler(trash service.DesktopTrashService) *DesktopTrashHandler {
	return &DesktopTrashHandler{trash: trash}
}

// RegisterRoutes mounts /trash below the desktop route group. Mount it under
// /api/v1/desktop to expose the desktop Trash API.
func (h *DesktopTrashHandler) RegisterRoutes(r chi.Router) {
	r.Get("/trash", h.List)
	r.Post("/trash", h.Trash)
	r.Post("/trash/restore", h.Restore)
	r.Post("/trash/delete", h.Delete)
}

func (h *DesktopTrashHandler) List(w http.ResponseWriter, r *http.Request) {
	items, err := h.trash.List(r.Context())
	if err != nil {
		writeDesktopTrashError(w, err)
		return
	}
	w.Header().Set("Cache-Control", "no-store")
	writeJSON(w, desktopTrashListResponse{Items: items}, http.StatusOK)
}

func (h *DesktopTrashHandler) Trash(w http.ResponseWriter, r *http.Request) {
	var request desktopTrashPathRequest
	if err := decodeJSONBody(w, r, &request); err != nil || request.Path == "" {
		writeError(w, "A file path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	item, err := h.trash.Trash(r.Context(), request.Path)
	if err != nil {
		writeDesktopTrashError(w, err)
		return
	}
	w.Header().Set("Cache-Control", "no-store")
	writeJSON(w, desktopTrashItemResponse{Item: item}, http.StatusCreated)
}

func (h *DesktopTrashHandler) Restore(w http.ResponseWriter, r *http.Request) {
	var request desktopTrashItemRequest
	if err := decodeJSONBody(w, r, &request); err != nil || request.ID == "" {
		writeError(w, "A Trash item ID is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if err := h.trash.Restore(r.Context(), request.ID); err != nil {
		writeDesktopTrashError(w, err)
		return
	}
	w.Header().Set("Cache-Control", "no-store")
	writeJSON(w, desktopTrashActionResponse{OK: true}, http.StatusOK)
}

func (h *DesktopTrashHandler) Delete(w http.ResponseWriter, r *http.Request) {
	var request desktopTrashItemRequest
	if err := decodeJSONBody(w, r, &request); err != nil || request.ID == "" {
		writeError(w, "A Trash item ID is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if err := h.trash.Delete(r.Context(), request.ID); err != nil {
		writeDesktopTrashError(w, err)
		return
	}
	w.Header().Set("Cache-Control", "no-store")
	writeJSON(w, desktopTrashActionResponse{OK: true}, http.StatusOK)
}
