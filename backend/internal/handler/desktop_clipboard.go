package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type DesktopClipboardHandler struct {
	clipboard service.DesktopClipboard
}

type desktopClipboardRequest struct {
	Operation string   `json:"operation"`
	Paths     []string `json:"paths"`
}

func NewDesktopClipboardHandler(clipboard service.DesktopClipboard) *DesktopClipboardHandler {
	return &DesktopClipboardHandler{clipboard: clipboard}
}

func (h *DesktopClipboardHandler) RegisterRoutes(r chi.Router) {
	r.Get("/clipboard", h.GetClipboard)
	r.Post("/clipboard", h.UpdateClipboard)
}

func (h *DesktopClipboardHandler) GetClipboard(w http.ResponseWriter, r *http.Request) {
	contents, err := h.clipboard.Get(r.Context())
	if err != nil {
		writeDesktopClipboardError(w, err)
		return
	}
	writeJSON(w, contents, http.StatusOK)
}

func (h *DesktopClipboardHandler) UpdateClipboard(w http.ResponseWriter, r *http.Request) {
	var request desktopClipboardRequest
	if err := decodeJSONBody(w, r, &request); err != nil {
		writeError(w, "Invalid clipboard request", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	var err error
	switch request.Operation {
	case string(service.DesktopClipboardCopy):
		err = h.clipboard.Set(r.Context(), service.DesktopClipboardCopy, request.Paths)
	case string(service.DesktopClipboardCut):
		err = h.clipboard.Set(r.Context(), service.DesktopClipboardCut, request.Paths)
	case "clear-cut":
		err = h.clipboard.ClearCut(r.Context(), request.Paths)
	default:
		writeError(w, "Invalid clipboard operation", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if err != nil {
		writeDesktopClipboardError(w, err)
		return
	}
	writeJSON(w, desktopActionResponse{OK: true}, http.StatusOK)
}
