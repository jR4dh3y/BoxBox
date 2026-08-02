package handler

import (
	"io"
	"net/http"
	"strconv"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// FileHandler handles file-related HTTP requests
type FileHandler struct {
	fileService service.FileManager
}

// NewFileHandler creates a new file handler
func NewFileHandler(fileService service.FileManager) *FileHandler {
	return &FileHandler{
		fileService: fileService,
	}
}

// RegisterRoutes registers file routes on the given router
func (h *FileHandler) RegisterRoutes(r chi.Router) {
	r.Get("/", h.ListRoots)
	r.Get("/stats", h.GetDriveStats)
	r.Get("/list/*", h.ListPath)
	r.Get("/*", h.GetPath)
	r.Post("/*", h.CreateItem)
	r.Put("/*", h.Rename)
	r.Patch("/*", h.SaveFileContent)
	r.Delete("/*", h.Delete)
}

// ListPath returns a directory page without the polymorphic file-info lookup.
func (h *FileHandler) ListPath(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}
	list, err := h.fileService.List(r.Context(), path, h.parseListOptions(r))
	if err != nil {
		HandleServiceError(w, err)
		return
	}
	writeJSON(w, list, http.StatusOK)
}

// MountPointResponse represents a mount point in API responses
type MountPointResponse struct {
	Name     string `json:"name"`
	Path     string `json:"path"`
	ReadOnly bool   `json:"readOnly"`
}

// RootsResponse represents the list of mount points
type RootsResponse struct {
	Roots []MountPointResponse `json:"roots"`
}

// CreateItemRequest represents the create file/directory request body
type CreateItemRequest struct {
	Name    string `json:"name"`
	Type    string `json:"type,omitempty"`
	Content string `json:"content,omitempty"`
}

// RenameRequest represents the rename request body
type RenameRequest struct {
	NewPath string `json:"newPath"`
}

// SaveFileRequest represents the file content save request body
type SaveFileRequest struct {
	Content string `json:"content"`
}

// ListRoots returns all configured mount points
// GET /api/v1/files
func (h *FileHandler) ListRoots(w http.ResponseWriter, r *http.Request) {
	mounts := h.fileService.ListMountPoints()

	roots := make([]MountPointResponse, len(mounts))
	for i, mount := range mounts {
		roots[i] = MountPointResponse{
			Name:     mount.Name,
			Path:     mount.Path,
			ReadOnly: mount.ReadOnly,
		}
	}

	writeJSON(w, RootsResponse{Roots: roots}, http.StatusOK)
}

// GetDriveStats returns disk usage statistics for all mount points
// GET /api/v1/files/stats
func (h *FileHandler) GetDriveStats(w http.ResponseWriter, r *http.Request) {
	stats, err := h.fileService.GetDriveStats(r.Context())
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, stats, http.StatusOK)
}

// GetPath handles GET requests for a path - returns directory listing or file info
// GET /api/v1/files/*path
func (h *FileHandler) GetPath(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		h.ListRoots(w, r)
		return
	}

	// Parse query parameters for listing options
	opts := h.parseListOptions(r)

	// Check if this is a directory or file
	info, err := h.fileService.GetInfo(r.Context(), path)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	if info.IsDir {
		// Return directory listing
		list, err := h.fileService.List(r.Context(), path, opts)
		if err != nil {
			HandleServiceError(w, err)
			return
		}
		writeJSON(w, list, http.StatusOK)
	} else {
		// Return file info
		writeJSON(w, info, http.StatusOK)
	}
}

// CreateItem creates a new directory or file
// POST /api/v1/files/*path
func (h *FileHandler) CreateItem(w http.ResponseWriter, r *http.Request) {
	basePath := chi.URLParam(r, "*")

	var req CreateItemRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	itemType := req.Type
	if itemType == "" {
		itemType = "directory"
	}

	if req.Name == "" {
		writeError(w, "Name is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if !validator.IsValidFileName(req.Name) {
		writeError(w, "Name cannot contain path separators or special path names", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Build full path
	fullPath := basePath
	if fullPath != "" {
		fullPath = fullPath + "/" + req.Name
	} else {
		fullPath = req.Name
	}

	switch itemType {
	case "directory", "folder":
		// Create directory
		if err := h.fileService.CreateDir(r.Context(), fullPath); err != nil {
			HandleServiceError(w, err)
			return
		}
	case "file":
		// Create file without overwriting any existing item
		file, err := h.fileService.CreateFile(r.Context(), fullPath)
		if err != nil {
			HandleServiceError(w, err)
			return
		}

		if req.Content != "" {
			if _, err := io.WriteString(file, req.Content); err != nil {
				_ = file.Close()
				_ = h.fileService.Delete(r.Context(), fullPath)
				writeError(w, "Failed to write file content", model.ErrCodeInternalError, http.StatusInternalServerError)
				return
			}
		}

		if err := file.Close(); err != nil {
			_ = h.fileService.Delete(r.Context(), fullPath)
			writeError(w, "Failed to create file", model.ErrCodeInternalError, http.StatusInternalServerError)
			return
		}
	default:
		writeError(w, "Type must be file or directory", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Return the created item info
	info, err := h.fileService.GetInfo(r.Context(), fullPath)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, info, http.StatusCreated)
}

// SaveFileContent overwrites an existing file's content
// PATCH /api/v1/files/*path
func (h *FileHandler) SaveFileContent(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	var req SaveFileRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if err := h.fileService.WriteFile(r.Context(), path, []byte(req.Content)); err != nil {
		HandleServiceError(w, err)
		return
	}

	info, err := h.fileService.GetInfo(r.Context(), path)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, info, http.StatusOK)
}

// Rename renames/moves a file or directory
// PUT /api/v1/files/*path
func (h *FileHandler) Rename(w http.ResponseWriter, r *http.Request) {
	oldPath := chi.URLParam(r, "*")
	if oldPath == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	var req RenameRequest
	if err := decodeJSONBody(w, r, &req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Validate new path
	if req.NewPath == "" {
		writeError(w, "New path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Perform rename
	if err := h.fileService.Rename(r.Context(), oldPath, req.NewPath); err != nil {
		HandleServiceError(w, err)
		return
	}

	// Return the renamed file/directory info
	info, err := h.fileService.GetInfo(r.Context(), req.NewPath)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, info, http.StatusOK)
}

// Delete removes a file or directory
// DELETE /api/v1/files/*path
func (h *FileHandler) Delete(w http.ResponseWriter, r *http.Request) {
	path := chi.URLParam(r, "*")
	if path == "" {
		writeError(w, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Check if confirmation is required (query param)
	confirm := r.URL.Query().Get("confirm")
	if confirm != "true" {
		// Check if path exists and get info
		info, err := h.fileService.GetInfo(r.Context(), path)
		if err != nil {
			HandleServiceError(w, err)
			return
		}

		// If it's a directory, require confirmation
		if info.IsDir {
			writeError(w, "Confirmation required to delete directory. Add ?confirm=true to confirm.", model.ErrCodeValidationError, http.StatusBadRequest)
			return
		}
	}

	// Perform delete
	if err := h.fileService.Delete(r.Context(), path); err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, map[string]string{"message": "Deleted successfully"}, http.StatusOK)
}

// parseListOptions extracts listing options from query parameters
func (h *FileHandler) parseListOptions(r *http.Request) model.ListOptions {
	opts := model.DefaultListOptions()

	if page := r.URL.Query().Get("page"); page != "" {
		if p, err := strconv.Atoi(page); err == nil && p > 0 {
			opts.Page = p
		}
	}

	if pageSize := r.URL.Query().Get("pageSize"); pageSize != "" {
		if ps, err := strconv.Atoi(pageSize); err == nil && ps > 0 && ps <= 1000 {
			opts.PageSize = ps
		}
	}

	if includeHidden := r.URL.Query().Get("includeHidden"); includeHidden != "" {
		if value, err := strconv.ParseBool(includeHidden); err == nil {
			opts.IncludeHidden = value
		}
	}

	if sortBy := r.URL.Query().Get("sortBy"); sortBy != "" {
		// Validate sort field
		validSortFields := map[string]bool{"name": true, "size": true, "modTime": true, "type": true}
		if validSortFields[sortBy] {
			opts.SortBy = sortBy
		}
	}

	if sortDir := r.URL.Query().Get("sortDir"); sortDir != "" {
		// Validate sort direction
		if sortDir == "asc" || sortDir == "desc" {
			opts.SortDir = sortDir
		}
	}

	if filter := r.URL.Query().Get("filter"); filter != "" {
		opts.Filter = filter
	}

	return opts
}
