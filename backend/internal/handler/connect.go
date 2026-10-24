package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type ConnectHandler struct {
	connect *service.ConnectService
}

func NewConnectHandler(connect *service.ConnectService) *ConnectHandler {
	return &ConnectHandler{connect: connect}
}

func (h *ConnectHandler) RegisterPublicRoutes(r chi.Router, redemptionLimit func(http.Handler) http.Handler) {
	// Apply before authentication so failures cannot be cached either.
	r.Use(connectNoStore)
	r.With(redemptionLimit).Post("/redeem", h.Redeem)
}

func (h *ConnectHandler) RegisterRoutes(r chi.Router) {
	r.Group(func(r chi.Router) {
		r.Use(connectNoStore)
		r.Group(func(r chi.Router) {
			r.Use(requireConnectAccount)
			r.Get("/pairing", h.Status)
			r.Post("/pairing", h.Enable)
			r.Delete("/pairing", h.Disable)
			r.Get("/clients", h.List)
			r.Delete("/clients/{id}", h.Revoke)
		})
		r.Delete("/client", h.RevokeSelf)
	})
}

func connectNoStore(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		next.ServeHTTP(w, r)
	})
}

func requireConnectAccount(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !authcontext.IsAccountSession(r.Context()) || authcontext.Username(r.Context()) == "" {
			writeError(w, "Account authentication required", "FORBIDDEN", http.StatusForbidden)
			return
		}
		if r.Method != http.MethodGet && !sameOriginRequest(r) {
			writeError(w, "Cross-origin pairing request rejected", "FORBIDDEN", http.StatusForbidden)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (h *ConnectHandler) Status(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, h.connect.PairingStatus(authcontext.Username(r.Context())), http.StatusOK)
}

func (h *ConnectHandler) Enable(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Password string `json:"password"`
	}
	if err := decodeJSONBody(w, r, &req); err != nil || req.Password == "" {
		writeError(w, "Password is required", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}
	code, err := h.connect.EnablePairing(r.Context(), authcontext.Username(r.Context()), req.Password)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, code, http.StatusOK)
}

func (h *ConnectHandler) Disable(w http.ResponseWriter, r *http.Request) {
	h.connect.DisablePairing(authcontext.Username(r.Context()))
	w.WriteHeader(http.StatusNoContent)
}

func (h *ConnectHandler) Redeem(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Code       string `json:"code"`
		ClientName string `json:"clientName"`
	}
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", "VALIDATION_ERROR", http.StatusBadRequest)
		return
	}
	client, err := h.connect.Redeem(req.Code, req.ClientName)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, client, http.StatusOK)
}

func (h *ConnectHandler) List(w http.ResponseWriter, r *http.Request) {
	clients, err := h.connect.ListClients(authcontext.Username(r.Context()))
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, struct {
		Clients []service.ConnectClient `json:"clients"`
	}{Clients: clients}, http.StatusOK)
}

func (h *ConnectHandler) Revoke(w http.ResponseWriter, r *http.Request) {
	if err := h.connect.RevokeClient(authcontext.Username(r.Context()), chi.URLParam(r, "id")); err != nil {
		HandleServiceError(w, err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h *ConnectHandler) RevokeSelf(w http.ResponseWriter, r *http.Request) {
	id := authcontext.ClientID(r.Context())
	if id == "" {
		writeError(w, "Paired client authentication required", "FORBIDDEN", http.StatusForbidden)
		return
	}
	if err := h.connect.RevokeClient(authcontext.Username(r.Context()), id); err != nil {
		HandleServiceError(w, err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
