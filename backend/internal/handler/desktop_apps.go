package handler

import (
	"context"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type DesktopAppsHandler struct {
	apps service.DesktopApps
}

type openWithRequest struct {
	Path  string `json:"path"`
	AppID string `json:"appId"`
}

type desktopPathRequest struct {
	Path string `json:"path"`
}

type desktopActionResponse struct {
	OK bool `json:"ok"`
}

// NewDesktopAppsHandler creates handlers for local Open With and terminal actions.
func NewDesktopAppsHandler(apps service.DesktopApps) *DesktopAppsHandler {
	return &DesktopAppsHandler{apps: apps}
}

// RegisterRoutes mounts /open, /apps, /open-with, and /terminal below /api/v1/desktop.
func (h *DesktopAppsHandler) RegisterRoutes(r chi.Router) {
	r.Post("/open", h.Open)
	r.Get("/apps", h.ListApps)
	r.Post("/open-with", h.OpenWith)
	r.Post("/terminal", h.OpenTerminal)
}

func (h *DesktopAppsHandler) ListApps(w http.ResponseWriter, r *http.Request) {
	path := r.URL.Query().Get("path")
	if strings.TrimSpace(path) == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	result, err := h.apps.ListApps(r.Context(), path)
	if err != nil {
		writeDesktopAppsError(w, err)
		return
	}
	writeJSON(w, result, http.StatusOK)
}

func (h *DesktopAppsHandler) OpenWith(w http.ResponseWriter, r *http.Request) {
	var request openWithRequest
	if err := decodeJSONBody(w, r, &request); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if strings.TrimSpace(request.Path) == "" || strings.TrimSpace(request.AppID) == "" {
		writeError(w, "Path and appId are required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if err := h.apps.OpenWith(r.Context(), request.Path, request.AppID); err != nil {
		writeDesktopAppsError(w, err)
		return
	}
	writeJSON(w, desktopActionResponse{OK: true}, http.StatusOK)
}

func (h *DesktopAppsHandler) Open(w http.ResponseWriter, r *http.Request) {
	h.runPathAction(w, r, h.apps.Open)
}

func (h *DesktopAppsHandler) OpenTerminal(w http.ResponseWriter, r *http.Request) {
	h.runPathAction(w, r, h.apps.OpenTerminal)
}

func (h *DesktopAppsHandler) runPathAction(w http.ResponseWriter, r *http.Request, action func(context.Context, string) error) {
	var request desktopPathRequest
	if err := decodeJSONBody(w, r, &request); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if strings.TrimSpace(request.Path) == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if err := action(r.Context(), request.Path); err != nil {
		writeDesktopAppsError(w, err)
		return
	}
	writeJSON(w, desktopActionResponse{OK: true}, http.StatusOK)
}
