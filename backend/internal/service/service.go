// Package service provides business logic for the file manager.
//
// # Services
//
// The package contains the following services:
//   - AuthService: User authentication and JWT token management
//   - FileService: File system operations (CRUD, listing, stats)
//   - JobService: Background job execution with progress tracking
//   - SearchService: Recursive file search
//   - SystemService: System information and drive discovery
//   - SettingsService: Custom drive name persistence
//   - ShareService: Single-file share links
//
// # Error Handling
//
// Services return domain-specific errors (ErrPathNotFound, ErrPermissionDenied, etc.)
// that handlers convert to appropriate HTTP responses.
//
// # Filesystem Abstraction
//
// All file operations use the filesystem.FS interface, allowing for
// easy testing with in-memory filesystems.
package service
