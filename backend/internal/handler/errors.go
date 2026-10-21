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

	// Share service errors
	{service.ErrShareNotFound, "Share not found", model.ErrCodeNotFound, http.StatusNotFound},
	{service.ErrShareTooLarge, "Upload exceeds the size limit", model.ErrCodeValidationError, http.StatusRequestEntityTooLarge},

	// Wallpaper errors
	{service.ErrWallpaperNotFound, "Wallpaper not found", model.ErrCodeNotFound, http.StatusNotFound},
	{service.ErrInvalidWallpaper, "Wallpaper must be a JPEG, PNG, GIF, or WebP image up to 20 MB with a valid display mode", model.ErrCodeValidationError, http.StatusBadRequest},

	// Auth service errors
	{service.ErrRefreshSuperseded, "Another request refreshed this session. Retry with the current cookie.", model.ErrCodeRefreshSuperseded, http.StatusConflict},
	{service.ErrInvalidCredentials, "Invalid credentials", model.ErrCodeUnauthorized, http.StatusUnauthorized},
	{service.ErrInvalidToken, "Invalid token", model.ErrCodeTokenInvalid, http.StatusUnauthorized},
	{service.ErrTokenExpired, "Token has expired", model.ErrCodeTokenInvalid, http.StatusUnauthorized},
	{service.ErrTokenRevoked, "Token has been revoked", model.ErrCodeTokenInvalid, http.StatusUnauthorized},
	{service.ErrSessionRecoveryRequired, "Your session could not be safely recovered. Please sign in again.", model.ErrCodeSessionRecoveryRequired, http.StatusUnauthorized},
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

func writeDesktopAppsError(w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, service.ErrDesktopRuntimeUnavailable):
		writeError(w, "Could not inspect this file type. Make sure GIO is available.", model.ErrCodeInternalError, http.StatusServiceUnavailable)
	case errors.Is(err, service.ErrDesktopApplicationUnavailable):
		writeError(w, "That application is no longer available for this file. Refresh the list and try again.", model.ErrCodeNotFound, http.StatusNotFound)
	case errors.Is(err, service.ErrDesktopTerminalUnavailable):
		writeError(w, "No supported terminal application is installed.", model.ErrCodeInternalError, http.StatusServiceUnavailable)
	case errors.Is(err, service.ErrDesktopTerminalLaunchFailed):
		writeError(w, "Could not open a terminal in that directory.", model.ErrCodeInternalError, http.StatusInternalServerError)
	case errors.Is(err, service.ErrDesktopLaunchFailed):
		writeError(w, "Could not launch the selected desktop application.", model.ErrCodeInternalError, http.StatusInternalServerError)
	default:
		HandleServiceError(w, err)
	}
}

func writeDesktopClipboardError(w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, service.ErrDesktopClipboardUnavailable):
		writeError(w, "Desktop clipboard helper is unavailable", model.ErrCodeInternalError, http.StatusServiceUnavailable)
	case errors.Is(err, service.ErrDesktopClipboardChanged):
		writeError(w, "Clipboard changed before the cut completed", model.ErrCodeConflict, http.StatusConflict)
	default:
		HandleServiceError(w, err)
	}
}

func writeDesktopVolumeError(w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, service.ErrDesktopVolumeNotFound):
		writeError(w, "This removable volume is no longer connected", model.ErrCodeNotFound, http.StatusNotFound)
	case errors.Is(err, service.ErrDesktopVolumesUnavailable):
		writeError(w, "Removable drive support is unavailable on this system", "VOLUMES_UNAVAILABLE", http.StatusServiceUnavailable)
	case errors.Is(err, service.ErrDesktopVolumeNotMounted):
		writeError(w, "Unmount the volume before ejecting it", model.ErrCodeConflict, http.StatusConflict)
	default:
		var commandErr *service.DesktopVolumeCommandError
		if errors.As(err, &commandErr) {
			writeError(w, commandErr.UserMessage(), model.ErrCodeConflict, http.StatusConflict)
			return
		}
		writeError(w, "Could not update the removable volume", model.ErrCodeInternalError, http.StatusInternalServerError)
	}
}

func writeDesktopTrashError(w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, service.ErrTrashUnavailable):
		writeError(w, "Trash is unavailable on this filesystem", "TRASH_UNAVAILABLE", http.StatusServiceUnavailable)
	case errors.Is(err, service.ErrTrashItemNotFound):
		writeError(w, "Trash item not found", model.ErrCodeNotFound, http.StatusNotFound)
	case errors.Is(err, service.ErrTrashConflict):
		writeError(w, "The original path already exists", model.ErrCodeConflict, http.StatusConflict)
	case errors.Is(err, service.ErrTrashCannotRestore):
		writeError(w, "This Trash item cannot be restored to its original location", model.ErrCodeConflict, http.StatusConflict)
	case errors.Is(err, service.ErrInvalidTrashItem):
		writeError(w, "This Trash item cannot be read safely", model.ErrCodeConflict, http.StatusConflict)
	default:
		HandleServiceError(w, err)
	}
}
