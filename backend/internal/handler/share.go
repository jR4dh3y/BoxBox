package handler

import (
	"errors"
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// ShareHandler handles share link management and recipient access.
// Management routes require authentication; recipient routes are public and
// authenticate purely by share token.
type ShareHandler struct {
	shareService   service.ShareService
	maxUploadBytes int64
}

// NewShareHandler creates a new share handler
func NewShareHandler(shareService service.ShareService, maxUploadMB int) *ShareHandler {
	if maxUploadMB <= 0 {
		maxUploadMB = config.DefaultMaxUploadMB
	}
	return &ShareHandler{
		shareService:   shareService,
		maxUploadBytes: int64(maxUploadMB) * 1024 * 1024,
	}
}

// RegisterRoutes registers owner-facing share management routes on the given
// router. It must be mounted inside the authenticated route group.
func (h *ShareHandler) RegisterRoutes(r chi.Router) {
	r.Post("/", h.Create)
	r.Get("/", h.List)
	r.Delete("/{id}", h.Revoke)
}

// RegisterPublicRoutes registers recipient-facing routes on the given router.
// It must be mounted inside a rate-limited public group; the share token in the
// path is the only credential.
func (h *ShareHandler) RegisterPublicRoutes(r chi.Router) {
	r.Get("/{token}", h.GetInfo)
	r.Get("/{token}/download", h.Download)
	r.Get("/{token}/preview", h.Preview)
	r.Post("/{token}/upload", h.Upload)
}

// Create creates a share link for a single existing file
// POST /api/v1/shares
func (h *ShareHandler) Create(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	var req model.CreateShareRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if req.Path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	var expiresAt time.Time
	if req.ExpiresInSeconds != nil {
		if *req.ExpiresInSeconds <= 0 {
			writeError(w, "expiresInSeconds must be a positive integer", model.ErrCodeValidationError, http.StatusBadRequest)
			return
		}
		expiresAt = time.Now().Add(time.Duration(*req.ExpiresInSeconds) * time.Second)
	}

	share, err := h.shareService.Create(r.Context(), username, req.Path, req.Permissions, expiresAt)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, model.ShareResponse{
		ID:          share.ID,
		Token:       share.Token,
		URL:         "/s/" + share.Token,
		FileName:    share.FileName,
		Permissions: share.Permissions,
		CreatedAt:   share.CreatedAt,
		ExpiresAt:   share.ExpiresAt,
	}, http.StatusCreated)
}

// List returns the caller's active share links
// GET /api/v1/shares
func (h *ShareHandler) List(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	shares, err := h.shareService.List(username)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	items := make([]model.ShareSummary, 0, len(shares))
	for _, share := range shares {
		items = append(items, model.ShareSummary{
			ID:          share.ID,
			Token:       share.Token,
			URL:         "/s/" + share.Token,
			FileName:    share.FileName,
			Path:        share.MountName + "/" + share.RelPath,
			Permissions: share.Permissions,
			CreatedAt:   share.CreatedAt,
			ExpiresAt:   share.ExpiresAt,
		})
	}

	writeJSON(w, model.ShareListResponse{Shares: items}, http.StatusOK)
}

// Revoke permanently disables a share link
// DELETE /api/v1/shares/{id}
func (h *ShareHandler) Revoke(w http.ResponseWriter, r *http.Request) {
	username := authcontext.Username(r.Context())
	if username == "" {
		writeError(w, "Authentication required", model.ErrCodeUnauthorized, http.StatusUnauthorized)
		return
	}

	id := chi.URLParam(r, "id")
	if id == "" {
		writeError(w, "Share id is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if err := h.shareService.Revoke(username, id); err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, map[string]any{"success": true}, http.StatusOK)
}

// GetInfo returns recipient-facing metadata for a share token. It never exposes
// mount names or internal paths.
// GET /api/v1/share/{token}
func (h *ShareHandler) GetInfo(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	share, err := h.shareService.ResolveForRecipient(token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	file, info, err := h.shareService.OpenForRecipient(r.Context(), token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	defer file.Close()

	writeJSON(w, model.ShareInfoResponse{
		FileName:    info.Name,
		Size:        info.Size,
		MimeType:    detectStreamMimeType(file, info.Name),
		Permissions: share.Permissions,
		ExpiresAt:   share.ExpiresAt,
	}, http.StatusOK)
}

// Download streams the shared file as an attachment with Range support
// GET /api/v1/share/{token}/download
func (h *ShareHandler) Download(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	share, err := h.shareService.ResolveForRecipient(token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	if !share.Permissions.Download {
		writeError(w, "This share does not allow downloads", model.ErrCodePermissionDenied, http.StatusForbidden)
		return
	}
	file, info, err := h.shareService.OpenForRecipient(r.Context(), token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	defer file.Close()

	w.Header().Set("Content-Type", detectStreamMimeType(file, info.Name))
	w.Header().Set("Content-Disposition", streamContentDisposition("attachment", info.Name))
	w.Header().Set("Content-Security-Policy", streamSandboxCSP)
	w.Header().Set("Accept-Ranges", "bytes")

	http.ServeContent(w, r, info.Name, info.ModTime, file)
}

// Preview streams the shared file inline with Range support. Active document
// formats are forced to attachment disposition, mirroring stream previews.
// GET /api/v1/share/{token}/preview
func (h *ShareHandler) Preview(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	share, err := h.shareService.ResolveForRecipient(token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	if !share.Permissions.View {
		writeError(w, "This share does not allow previews", model.ErrCodePermissionDenied, http.StatusForbidden)
		return
	}
	file, info, err := h.shareService.OpenForRecipient(r.Context(), token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	defer file.Close()

	mimeType := detectStreamMimeType(file, info.Name)
	disposition := "inline"
	if isActivePreviewMimeType(mimeType) {
		disposition = "attachment"
	}

	w.Header().Set("Content-Type", mimeType)
	w.Header().Set("Content-Disposition", streamContentDisposition(disposition, info.Name))
	w.Header().Set("Content-Security-Policy", streamSandboxCSP)
	w.Header().Set("Accept-Ranges", "bytes")
	// Recipients can overwrite the file, so previews must revalidate instead of
	// being served from heuristic browser cache with the pre-overwrite content.
	w.Header().Set("Cache-Control", "no-cache, no-transform")

	http.ServeContent(w, r, info.Name, info.ModTime, file)
}

// Upload overwrites the shared file with the request body in a single shot
// POST /api/v1/share/{token}/upload
func (h *ShareHandler) Upload(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	share, err := h.shareService.ResolveForRecipient(token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	if !share.Permissions.Write {
		writeError(w, "This share does not allow updates", model.ErrCodePermissionDenied, http.StatusForbidden)
		return
	}
	if r.ContentLength == 0 {
		writeError(w, "Request body is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if r.ContentLength > h.maxUploadBytes {
		writeError(w, "Upload exceeds the size limit", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge)
		return
	}

	r.Body = http.MaxBytesReader(w, r.Body, h.maxUploadBytes)
	written, err := h.shareService.WriteForRecipient(r.Context(), token, r.Body)
	if err != nil {
		var maxBytesErr *http.MaxBytesError
		if errors.As(err, &maxBytesErr) {
			writeError(w, "Upload exceeds the size limit", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge)
			return
		}
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, model.ShareUploadResponse{
		FileName: share.FileName,
		Size:     written,
	}, http.StatusOK)
}
