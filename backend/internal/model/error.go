package model

// ErrorResponse represents a standard API error response
type ErrorResponse struct {
	Error   string `json:"error"`
	Code    string `json:"code"`
	Details string `json:"details,omitempty"`
}

// Error codes for API responses
const (
	ErrCodeNotFound         = "NOT_FOUND"
	ErrCodeAccessDenied     = "ACCESS_DENIED"
	ErrCodeReadOnly         = "READ_ONLY"
	ErrCodeInvalidPath      = "INVALID_PATH"
	ErrCodeUnauthorized     = "UNAUTHORIZED"
	ErrCodeTokenInvalid     = "TOKEN_INVALID"
	ErrCodePermissionDenied = "PERMISSION_DENIED"
	ErrCodeConflict         = "CONFLICT"
	ErrCodeValidationError  = "VALIDATION_ERROR"
	ErrCodeJobNotFound      = "JOB_NOT_FOUND"
	ErrCodeChunkMissing     = "CHUNK_MISSING"
	ErrCodeChecksumMismatch = "CHECKSUM_MISMATCH"
	ErrCodeInternalError    = "INTERNAL_ERROR"
)
