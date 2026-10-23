# Architecture

BoxBox is a single-server web app: a Go HTTP server exposes the API and serves an embedded SvelteKit static build.

```text
Browser
  |
  | REST, streaming, WebSocket
  v
Go server
  |-- auth, security headers, rate limits
  |-- file, stream, search, job, system, settings handlers
  |-- services for filesystem, jobs, auth, search, disk stats
  |-- embedded SvelteKit frontend
  v
Configured host/container mount points
```

## Runtime Model

The production Dockerfile has three stages:

1. Build the SvelteKit frontend with Bun.
2. Copy the static frontend into `backend/internal/static/dist` and compile the Go server.
3. Run the resulting binary in a small Alpine image.

At runtime there is one HTTP process. API routes are matched first; all other routes fall through to the embedded SPA frontend.

## Backend Packages

```text
backend/
  cmd/server/          entrypoint, router, service wiring
  internal/config/     config loading and runtime constants
  internal/handler/    HTTP handlers
  internal/middleware/ auth, rate limit, security, mount guards
  internal/model/      config, file, job, error, drive models
  internal/pkg/        filesystem, file utility, path validation helpers
  internal/service/    auth, file, search, job, system, settings services;
                       mounts.go resolves every virtual path
  internal/static/     embedded frontend assets
  internal/websocket/  hub, client, message protocol
```

## Request Flow

File listing:

```text
GET /api/v1/files/list/home/projects
  -> JWT middleware
  -> mount point guard
  -> FileHandler.ListPath
  -> FileService.List
  -> filesystem abstraction
```

Directory responses contain every entry, unsorted, up to the 10,000-entry safety
limit. The browser keeps one query result per folder, sorts and filters it locally
(`sortFiles.ts`), and renders only a visible window of list rows or grid tiles. Refresh replaces the collection;
it never combines pages from different filesystem states. Scroll anchors use file
paths and survive folder navigation and view changes.

Chunked upload:

```text
POST /api/v1/stream/upload/home/file.zip
  -> JWT middleware
  -> mount point guard
  -> StreamHandler.Upload
  -> UploadManager temp chunk storage
  -> checksum verification
  -> atomic final rename
```

Background copy or move:

```text
POST /api/v1/jobs
  -> JobHandler.Create
  -> JobService queue
  -> worker pool
  -> filesystem copy/move/delete
  -> WebSocket progress updates
```

## Path Resolution

Every service turns a virtual path such as `media/movies/a.mkv` into a real path through one function, `mounts.resolve` in `internal/service/mounts.go`. The caller states how it uses the path (read an existing path, modify an existing path, or create one). `resolve` then checks the mount name, rejects traversal, refuses writes to read-only mounts, and resolves symlinks so the result stays inside its mount.

Two places start from an already-resolved real path instead: a job re-checks its stored paths before it runs, and a share folder checks its sub-paths against the share root. They call the confinement helpers in `path_safety.go` directly.

## Frontend App

```text
frontend/src/
  routes/              SvelteKit routes
  lib/api/             typed API functions
  lib/components/      app UI and preview components
  lib/components/ui/   reusable base UI primitives
  lib/browse/          browse page logic: location, data, actions, uploads, selection
  lib/stores/          auth, jobs, settings, upload, websocket state (runes)
  lib/types/           shared TypeScript types
  lib/utils/           upload, file type, formatting, storage helpers
```

The frontend uses a same-origin API base of `/api/v1`. In production the browser talks to the same Go server that served the app.

## State and Persistence

Most server state is intentionally lightweight:

- Authentication uses an active-session registry in `auth-sessions.json` under `data_dir`. The version 1 JSON file maps session IDs to usernames, SHA-256 hashes of the current refresh tokens, and session deadlines. It never stores raw JWTs.
- Every access and refresh token carries a session ID. Validation requires a live registry entry for that ID, the configured username, and an unexpired token. Refresh also requires the token hash to match the registry's current refresh hash.
- Refresh rotates the current refresh hash while preserving the session ID and sliding refresh lifetime. The session deadline records the longest existing deadline across TTL changes. Logout removes one whole session, invalidating all of its access tokens, including tokens issued before refresh. Other logins remain active.
- Login, refresh, and logout persist registry changes before reporting success. The writer syncs and closes a temporary file, renames it over the store, then syncs the parent directory. Rename is the publication point. Success requires all three steps.
- Before rename, a failed write rolls back only the session changed by the request while holding the auth mutex. Login removes its new session; refresh restores its previous record; logout restores the removed record. The request fails with `500` and does not publish credentials.
- If a durability step fails after rename, the service keeps the installed in-memory state and reports `ErrSessionRecoveryRequired`. The HTTP API returns `401 SESSION_RECOVERY_REQUIRED` without a token pair. Refresh and logout clear the cookie; login does not clear an unrelated cookie.
- On startup, a missing registry file or a valid registry whose `sessions` map is empty produces an empty active-session registry. Old access and refresh tokens then fail after restart, while a fresh login can create a session. A zero-byte or malformed file is invalid and fails startup. A store deleted during runtime does not clear the in-memory registry, and a later successful write can persist it again.
- Corrupt, unreadable, non-regular, unsupported, or future-version stores fail startup. The service does not reset them. `auth-revocations.json` is ignored and is not deleted; a denylist cannot enumerate active sessions.
- The file store relies on Linux filesystem rename and sync behavior and supports one server process per `data_dir`. It does not coordinate multiple processes. Restoring an older valid `auth-sessions.json` backup can revive sessions, access tokens, and their current refresh tokens. Rotate `BOXBOX_JWT_SECRET` after a rollback to prevent that. An older validator that ignores session IDs and the registry may also accept downgraded tokens, so rotate the secret when downgrading.
- Removing a configured user makes that user's tokens fail validation after startup. The same applies to tokens for legacy sessions without a session ID; the service never enrolls them automatically. Share-link storage remains separate from the authentication registry.
- Jobs are in memory and cleaned after a retention period.
- Upload sessions are in memory with temporary chunks on disk.
- Custom drive names are persisted under `/data/drive-names.json`.
- Files themselves remain on mounted host paths.

## Security Boundaries

The important boundaries are:

- JWT authentication for API access.
- Mount point resolution and traversal prevention.
- Read-only mount point enforcement.
- Container volume boundaries.
- Reverse proxy and network exposure.

Because BoxBox can modify host files, mount scope and credentials matter more than traditional database permissions.
