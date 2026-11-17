# API Reference

BoxBox exposes a REST API, streaming endpoints, and a WebSocket endpoint under `/api/v1`.

## Base URLs

```text
http://localhost:8080/api/v1   # if Docker maps 8080 -> container port 8080
http://localhost/api/v1        # inside a reverse proxy host
```

Health checks are available at both `/health` and `/api/v1/health`.

## Authentication

All API routes except auth and health require a JWT access token.

### Login

```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "your-password"
}
```

Response:

```json
{
  "accessToken": "eyJhbGciOiJIUzI1NiIs...",
  "expiresAt": "2026-05-13T10:15:00Z"
}
```

The refresh token is not in the body. The server sets it as the `boxbox_refresh` cookie (`HttpOnly`, `SameSite=Strict`, path `/api/v1/auth`, `Secure` over HTTPS). Send requests with cookies enabled so refresh and logout can use it.

Login creates a new session and saves it before returning the token pair. If the save fails before publication, login returns `500` without a token pair or `Set-Cookie` header. The browser's existing refresh cookie remains unchanged.

### Use an Access Token

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
```

### Refresh

```http
POST /api/v1/auth/refresh
```

The request has no body. The server reads the `boxbox_refresh` cookie, checks that its session is active and that the token is the session's current refresh token, then returns a new `accessToken` and `expiresAt` and rotates the refresh cookie. The access token keeps working until its own expiry while the session remains active.

### Logout

```http
POST /api/v1/auth/logout
```

The request has no body. Logout needs a valid current refresh cookie. It does not need an access-token header. Logout removes the entire session, so all access tokens issued by that session stop working, including tokens from earlier refresh rotations. Other logins for the same account remain active. A missing or invalid refresh cookie returns `401 TOKEN_INVALID` and clears the cookie.

A still-unexpired refresh token that another request has replaced returns `409 REFRESH_SUPERSEDED` from refresh or logout. The response contains no token pair and no `Set-Cookie` header, preserving the replacement cookie shared by browser tabs. Retry with the current cookie. A repeated conflict is retryable, not proof that the session has ended.

Login, refresh, and logout save session state before reporting success. Publication occurs at the file rename. Success requires syncing and closing the file, renaming it, and syncing the parent directory. Before rename, a failed write returns `500 INTERNAL_ERROR` without publishing a token pair or a `Set-Cookie` header. Refresh and logout preserve the existing cookie so the client can retry. Login does not clear an unrelated cookie.

If a durability step fails after rename, the server keeps the installed in-memory state and returns `401 SESSION_RECOVERY_REQUIRED` without a token pair. Refresh and logout clear the cookie. Login sends no new cookie and does not clear an unrelated existing cookie. Sign in again after this error. Logout did not report success.

Every access and refresh token must name an active session. If the registry file is missing at startup, or a valid version 1 registry has an empty `sessions` map, the server starts with no active sessions and previously issued tokens receive `401`. A zero-byte or malformed file fails startup. Deleting the registry while the server is running does not clear its in-memory sessions; an access token can still receive `200`, and a later successful write can persist the registry again.

Tokens without a session ID, including legacy tokens, are rejected. The server does not enroll them automatically.


## Endpoint pages

| Page | Covers |
| --- | --- |
| [Files](api-files.md) | Mount points, directories, file info, create, rename, move, delete and search. |
| [Downloads and uploads](api-transfers.md) | Download, folder archives, previews and chunked uploads. |
| [Share links](api-shares.md) | Creating, listing, updating and revoking shares, and the public recipient endpoints. |
| [Jobs, settings and WebSocket](api-system.md) | Background jobs, system settings, live events and the desktop file chooser. |

## Errors

Errors use:

```json
{
  "error": "Path is required",
  "code": "VALIDATION_ERROR",
  "details": ""
}
```

Common codes include `VALIDATION_ERROR`, `UNAUTHORIZED`, `TOKEN_INVALID`, `SESSION_RECOVERY_REQUIRED`, `ACCESS_DENIED`, `READ_ONLY`, `INVALID_PATH`, `NOT_FOUND`, `CONFLICT`, `JOB_NOT_FOUND`, `CHECKSUM_MISMATCH`, and `INTERNAL_ERROR`.

Auth storage errors before rename return `500` with `INTERNAL_ERROR` and no token pair or `Set-Cookie` header. Refresh and logout preserve the existing cookie for retry. Login does not clear an unrelated cookie. `SESSION_RECOVERY_REQUIRED` returns `401` if a durability step fails after rename. It has no token pair; refresh and logout clear the cookie, while login does not clear an unrelated cookie. Logout does not count as successful in this case.

