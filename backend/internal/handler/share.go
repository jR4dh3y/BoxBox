package handler

import (
	"errors"
	"net/http"
	"path"
	"strings"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/rs/zerolog/log"
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
	r.Patch("/{id}", h.Update)
	r.Delete("/{id}", h.Revoke)
}

// RegisterPublicRoutes registers recipient-facing routes on the given router.
// It must be mounted inside a rate-limited public group; the share token in the
// path is the only credential.
func (h *ShareHandler) RegisterPublicRoutes(r chi.Router) {
	r.Get("/{token}", h.GetInfo)
	r.Get("/{token}/items", h.ListItems)
	r.Delete("/{token}/items", h.DeleteItem)
	r.Get("/{token}/download", h.Download)
	r.Get("/{token}/archive", h.Archive)
	r.Get("/{token}/preview", h.Preview)
	r.Post("/{token}/upload", h.Upload)
}

// Create creates a share link for an existing file or folder.
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

	share, err := h.shareService.Create(r.Context(), username, req.Path, service.ShareSettings{
		Permissions:    req.Permissions,
		MaxUploadBytes: req.MaxUploadBytes,
	}, expiresAt)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, model.ShareResponse{
		ID:             share.ID,
		Token:          share.Token,
		URL:            "/s/" + share.Token,
		FileName:       share.FileName,
		Permissions:    share.Permissions.ToResponse(),
		MaxUploadBytes: share.MaxUploadBytes,
		IsFolder:       share.IsFolder,
		CreatedAt:      share.CreatedAt,
		ExpiresAt:      share.ExpiresAt,
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
		items = append(items, shareSummary(share))
	}

	writeJSON(w, model.ShareListResponse{Shares: items}, http.StatusOK)
}

func shareSummary(share model.Share) model.ShareSummary {
	return model.ShareSummary{
		ID:             share.ID,
		Token:          share.Token,
		URL:            "/s/" + share.Token,
		FileName:       share.FileName,
		Path:           shareDisplayPath(share),
		Permissions:    share.Permissions.ToResponse(),
		MaxUploadBytes: share.MaxUploadBytes,
		IsFolder:       share.IsFolder,
		CreatedAt:      share.CreatedAt,
		ExpiresAt:      share.ExpiresAt,
	}
}

func shareDisplayPath(share model.Share) string {
	return path.Join(share.MountName, strings.ReplaceAll(share.RelPath, "\\", "/"))
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

// Update changes the access on one of the caller's active folder shares.
func (h *ShareHandler) Update(w http.ResponseWriter, r *http.Request) {
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
	var req model.UpdateShareRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	share, err := h.shareService.Update(username, id, service.ShareUpdateSettings{
		Permissions:    req.Permissions,
		MaxUploadBytes: req.MaxUploadBytes,
	})
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, shareSummary(*share), http.StatusOK)
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
	info, mimeType, err := h.shareService.InfoForRecipient(r.Context(), token)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, model.ShareInfoResponse{
		FileName:       info.Name,
		Size:           info.Size,
		MimeType:       mimeType,
		Permissions:    share.Permissions.ToResponse(),
		MaxUploadBytes: share.MaxUploadBytes,
		IsFolder:       share.IsFolder,
		ExpiresAt:      share.ExpiresAt,
	}, http.StatusOK)
}

// ListItems returns the visible entries below a shared folder.
// GET /api/v1/share/{token}/items?path=relative/path
func (h *ShareHandler) ListItems(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	items, err := h.shareService.ListForRecipient(r.Context(), token, r.URL.Query().Get("path"))
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, items, http.StatusOK)
}

// DeleteItem removes a file or subfolder below a share that grants deletion.
func (h *ShareHandler) DeleteItem(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if r.URL.Query().Get("path") == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	if err := h.shareService.DeleteForRecipientPath(r.Context(), token, r.URL.Query().Get("path")); err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, map[string]any{"success": true}, http.StatusOK)
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
	var file service.File
	var info *model.FileInfo
	if share.IsFolder {
		file, info, err = h.shareService.OpenForRecipientPath(r.Context(), token, r.URL.Query().Get("path"))
	} else {
		file, info, err = h.shareService.OpenForRecipient(r.Context(), token)
	}
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

// Archive streams a ZIP containing a shared folder or a folder below it.
func (h *ShareHandler) Archive(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	if token == "" {
		writeError(w, "Share token is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	archive, err := h.shareService.PrepareDirectoryArchive(r.Context(), token, r.URL.Query().Get("path"))
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	w.Header().Set("Content-Type", "application/zip")
	w.Header().Set("Content-Disposition", streamContentDisposition("attachment", archive.Name+".zip"))
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	if err := archive.WriteTo(r.Context(), w); err != nil {
		log.Error().Err(err).Msg("Could not finish shared-folder archive")
	}
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
	var file service.File
	var info *model.FileInfo
	if share.IsFolder {
		file, info, err = h.shareService.OpenForRecipientPath(r.Context(), token, r.URL.Query().Get("path"))
	} else {
		file, info, err = h.shareService.OpenForRecipient(r.Context(), token)
	}
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

// Upload adds or replaces a file below a writable shared folder.
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
	if !share.IsFolder || !share.Permissions.Upload {
		writeError(w, "This share does not allow uploads", model.ErrCodePermissionDenied, http.StatusForbidden)
		return
	}
	if r.ContentLength == 0 {
		writeError(w, "Request body is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	maxUploadBytes := share.MaxUploadBytes
	if maxUploadBytes <= 0 || maxUploadBytes > h.maxUploadBytes {
		maxUploadBytes = h.maxUploadBytes
	}
	if r.ContentLength > maxUploadBytes {
		writeError(w, "Upload exceeds the size limit", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge)
		return
	}

	r.Body = http.MaxBytesReader(w, r.Body, maxUploadBytes)
	written, fileName, err := h.shareService.WriteForRecipientPath(
		r.Context(), token, r.URL.Query().Get("path"), r.Body,
	)
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
		FileName: fileName,
		Size:     written,
	}, http.StatusOK)
}
