package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// SearchHandler handles search-related HTTP requests
type SearchHandler struct {
	searchService service.SearchService
}

// NewSearchHandler creates a new search handler
func NewSearchHandler(searchService service.SearchService) *SearchHandler {
	return &SearchHandler{
		searchService: searchService,
	}
}

// RegisterRoutes registers search routes on the given router
func (h *SearchHandler) RegisterRoutes(r chi.Router) {
	r.Get("/", h.Search)
}

// SearchResponse represents the search results response
type SearchResponse struct {
	Path    string           `json:"path"`
	Query   string           `json:"query"`
	Results []model.FileInfo `json:"results"`
	Count   int              `json:"count"`
}

// Search handles search requests
// GET /api/v1/search?path=&q=
func (h *SearchHandler) Search(w http.ResponseWriter, r *http.Request) {
	// Get query parameters
	path := r.URL.Query().Get("path")
	query := r.URL.Query().Get("q")

	// Validate query is not empty
	if query == "" {
		writeError(w, "Search query is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Default path to empty string (will search from root mount points)
	if path == "" {
		writeError(w, "Search path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Perform search
	results, err := h.searchService.Search(r.Context(), path, query)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	// Return results
	resp := SearchResponse{
		Path:    path,
		Query:   query,
		Results: results,
		Count:   len(results),
	}

	writeJSON(w, resp, http.StatusOK)
}


