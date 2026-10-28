package handler

import (
	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"net/http"
)

type DesktopTransfersHandler struct{ transfers *service.DesktopTransfers }

func NewDesktopTransfersHandler(transfers *service.DesktopTransfers) *DesktopTransfersHandler {
	return &DesktopTransfersHandler{transfers}
}
func (h *DesktopTransfersHandler) RegisterRoutes(r chi.Router) {
	r.Get("/transfers", h.List)
	r.Post("/transfers", h.Create)
	r.Get("/transfers/events", h.Events)
	r.Get("/transfers/{id}", h.Get)
	r.Delete("/transfers/{id}", h.Cancel)
}
func (h *DesktopTransfersHandler) List(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, JobListResponse{Jobs: h.transfers.List()}, http.StatusOK)
}
func (h *DesktopTransfersHandler) Get(w http.ResponseWriter, r *http.Request) {
	job, err := h.transfers.Get(chi.URLParam(r, "id"))
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, job, http.StatusOK)
}
func (h *DesktopTransfersHandler) Create(w http.ResponseWriter, r *http.Request) {
	var params model.JobParams
	if err := decodeJSONBody(w, r, &params); err != nil {
		HandleServiceError(w, service.ErrInvalidJobParams)
		return
	}
	job, err := h.transfers.Create(r.Context(), params)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, job, http.StatusAccepted)
}
func (h *DesktopTransfersHandler) Cancel(w http.ResponseWriter, r *http.Request) {
	if err := h.transfers.Cancel(chi.URLParam(r, "id")); err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, map[string]string{"message": "Transfer cancellation requested"}, http.StatusOK)
}
