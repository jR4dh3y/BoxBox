package handler

import (
	"errors"
	"net/http"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/validator"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// ErrorMapping maps service errors to HTTP responses
type ErrorMapping struct {
	Error      error
	Message    string
	Code       string
	StatusCode int
}

// serviceErrorMappings contains all service error to HTTP response mappings
var serviceErrorMappings = []ErrorMapping{
	// File service errors
	{service.ErrPathNotFound, "Path not found", model.ErrCodeNotFound, http.StatusNotFound},
	{service.ErrPathExists, "Path already exists", model.ErrCodeConflict, http.StatusConflict},
	{service.ErrNotDirectory, "Path is not a directory", model.ErrCodeValidationError, http.StatusBadRequest},
	{service.ErrNotFile, "Path is not a file", model.ErrCodeValidationError, http.StatusBadRequest},
	{service.ErrPermissionDenied, "Permission denied", model.ErrCodePermissionDenied, http.StatusForbidden},
	{service.ErrMountPointNotFound, "Mount point not found", model.ErrCodeAccessDenied, http.StatusForbidden},
	{service.ErrInvalidOperation, "Invalid operation", model.ErrCodeValidationError, http.StatusBadRequest},
	{service.ErrDirectoryTooLarge, "Directory exceeds server entry limit", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge},
	{service.ErrThumbnailTooLarge, "Image exceeds thumbnail limits", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge},

	// Path validation errors
	{validator.ErrPathTraversal, "Invalid path", model.ErrCodeValidationError, http.StatusBadRequest},
	{validator.ErrEmptyPath, "Path is required", model.ErrCodeValidationError, http.StatusBadRequest},
	{validator.ErrInvalidPath, "Invalid path", model.ErrCodeValidationError, http.StatusBadRequest},
	{validator.ErrOutsideMountPoint, "Mount point not found", model.ErrCodeAccessDenied, http.StatusForbidden},
	{validator.ErrMountPointNotFound, "Mount point not found", model.ErrCodeAccessDenied, http.StatusForbidden},

	// Job service errors
	{service.ErrJobNotFound, "Job not found", model.ErrCodeJobNotFound, http.StatusNotFound},
	{service.ErrJobNotCancellable, "Job cannot be cancelled", model.ErrCodeValidationError, http.StatusBadRequest},
	{service.ErrInvalidJobType, "Invalid job type", model.ErrCodeValidationError, http.StatusBadRequest},
	{service.ErrInvalidJobParams, "Invalid job parameters", model.ErrCodeValidationError, http.StatusBadRequest},

	// Search service errors
	{service.ErrEmptyQuery, "Search query cannot be empty", model.ErrCodeValidationError, http.StatusBadRequest},

	// Auth service errors
	{service.ErrInvalidCredentials, "Invalid credentials", model.ErrCodeUnauthorized, http.StatusUnauthorized},
	{service.ErrInvalidToken, "Invalid token", model.ErrCodeTokenInvalid, http.StatusUnauthorized},
	{service.ErrTokenExpired, "Token has expired", model.ErrCodeTokenInvalid, http.StatusUnauthorized},
	{service.ErrTokenRevoked, "Token has been revoked", model.ErrCodeTokenInvalid, http.StatusUnauthorized},
}

// HandleServiceError converts service errors to HTTP responses
// This is the centralized error handler that should be used by all handlers
func HandleServiceError(w http.ResponseWriter, err error) {
	for _, mapping := range serviceErrorMappings {
		if errors.Is(err, mapping.Error) {
			writeError(w, mapping.Message, mapping.Code, mapping.StatusCode)
			return
		}
	}
	// Default to internal server error
	writeError(w, "Internal server error", model.ErrCodeInternalError, http.StatusInternalServerError)
}
