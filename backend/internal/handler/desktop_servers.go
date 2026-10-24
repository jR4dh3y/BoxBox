package handler

import (
	"io"
	"net/http"
	"net/url"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type DesktopServersHandler struct{ servers *service.DesktopServers }

func NewDesktopServersHandler(servers *service.DesktopServers) *DesktopServersHandler {
	return &DesktopServersHandler{servers: servers}
}
func (h *DesktopServersHandler) RegisterRoutes(r chi.Router) {
	r.Group(func(r chi.Router) {
		r.Use(func(next http.Handler) http.Handler {
			return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if !h.servers.Available() {
					DesktopGatewayUnavailable(w, r)
					return
				}
				next.ServeHTTP(w, r)
			})
		})
		r.Get("/servers", h.List)
		r.Post("/servers", h.Create)
		r.Patch("/servers/{serverID}", h.Rename)
		r.Delete("/servers/{serverID}", h.Delete)
		r.Post("/servers/{serverID}/pair", h.Pair)
		r.HandleFunc("/servers/{serverID}/api/v1/*", h.Forward)
	})
}
func (h *DesktopServersHandler) List(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, struct {
		Servers []service.DesktopServer `json:"servers"`
	}{h.servers.List()}, 200)
}
func (h *DesktopServersHandler) Create(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Name       string `json:"name"`
		URL        string `json:"url"`
		Code       string `json:"code"`
		ClientName string `json:"clientName"`
	}
	if err := decodeJSONBody(w, r, &req); err != nil {
		HandleServiceError(w, service.ErrDesktopServerInput)
		return
	}
	result, err := h.servers.Create(r.Context(), req.Name, req.URL, req.Code, req.ClientName)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, result, 201)
}
func (h *DesktopServersHandler) Pair(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Code       string `json:"code"`
		ClientName string `json:"clientName"`
	}
	if err := decodeJSONBody(w, r, &req); err != nil {
		HandleServiceError(w, service.ErrDesktopServerInput)
		return
	}
	result, err := h.servers.Pair(r.Context(), chi.URLParam(r, "serverID"), req.Code, req.ClientName)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, result, 200)
}
func (h *DesktopServersHandler) Rename(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Name string `json:"name"`
	}
	if err := decodeJSONBody(w, r, &req); err != nil {
		HandleServiceError(w, service.ErrDesktopServerInput)
		return
	}
	result, err := h.servers.Rename(chi.URLParam(r, "serverID"), req.Name)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, result, 200)
}
func (h *DesktopServersHandler) Delete(w http.ResponseWriter, r *http.Request) {
	if err := h.servers.Delete(r.Context(), chi.URLParam(r, "serverID")); err != nil {
		HandleServiceError(w, err)
		return
	}
	w.WriteHeader(204)
}
func (h *DesktopServersHandler) Forward(w http.ResponseWriter, r *http.Request) {
	query, queryErr := url.ParseQuery(r.URL.RawQuery)
	if queryErr != nil {
		HandleServiceError(w, service.ErrRemoteRoute)
		return
	}
	route := "/" + chi.URLParam(r, "*")
	response, err := h.servers.Forward(r.Context(), chi.URLParam(r, "serverID"), r.Method, route, query, r.Header, r.Body)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	defer response.Body.Close()
	for _, name := range []string{"Content-Type", "Content-Length", "Content-Disposition", "Content-Range", "Accept-Ranges", "ETag", "Last-Modified", "X-Create-Only"} {
		for _, value := range response.Header.Values(name) {
			w.Header().Add(name, value)
		}
	}
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Content-Security-Policy", streamSandboxCSP)
	w.WriteHeader(response.StatusCode)
	_, _ = io.CopyBuffer(w, response.Body, make([]byte, 64<<10))
}

// DesktopGatewayUnavailable keeps initialization failures out of the SPA fallback.
func DesktopGatewayUnavailable(w http.ResponseWriter, r *http.Request) {
	writeError(w, "Saved server storage unavailable", model.ErrCodeInternalError, http.StatusServiceUnavailable)
}
