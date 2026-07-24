// Package middleware provides HTTP middleware for the BoxBox API.
//
// # Available Middleware
//
//   - JWTAuth: Validates JWT tokens and adds claims to request context
//   - MountPointGuard: Validates paths against configured mount points
//   - SecurityHeaders: Adds security headers to responses
//
// # Usage
//
//	r.Use(middleware.SecurityHeaders)
//	r.Use(middleware.JWTAuth(authService))
//	r.Use(middleware.MountPointGuard(mountPoints))
package middleware
