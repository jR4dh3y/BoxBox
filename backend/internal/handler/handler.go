// Package handler provides HTTP handlers for the BoxBox API.
//
// # Handlers
//
// The package contains the following handlers:
//   - AuthHandler: Authentication (login, logout, token refresh)
//   - FileHandler: File operations (list, create, rename, delete)
//   - StreamHandler: Streaming uploads and downloads
//   - JobHandler: Background job management (copy, move, delete)
//   - SearchHandler: File search operations
//   - WebSocketHandler: Real-time updates via WebSocket
//   - SystemHandler: System information (drives, mount points)
//   - ShareHandler: Share link management and token-authenticated recipient access
//
// # Error Handling
//
// All handlers use HandleServiceError() for error conversion
// and return JSON responses using writeJSON() and writeError().
//
// # Authentication
//
// Protected routes require a valid JWT token in the Authorization header
// or as a query parameter (for streaming endpoints).
package handler
