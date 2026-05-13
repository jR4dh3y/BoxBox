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
  internal/service/    auth, file, search, job, system, settings services
  internal/static/     embedded frontend assets
  internal/websocket/  hub, client, message protocol
```

## Request Flow

File listing:

```text
GET /api/v1/files/home/projects
  -> JWT middleware
  -> mount point guard
  -> FileHandler.GetPath
  -> FileService.List or FileService.GetInfo
  -> filesystem abstraction
```

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

## Frontend App

```text
frontend/src/
  routes/              SvelteKit routes
  lib/api/             typed API functions
  lib/components/      app UI and preview components
  lib/components/ui/   reusable base UI primitives
  lib/stores/          auth, files, jobs, upload, websocket state
  lib/types/           shared TypeScript types
  lib/utils/           upload, file type, formatting, storage helpers
```

The frontend uses a same-origin API base of `/api/v1`. In production the browser talks to the same Go server that served the app.

## State and Persistence

Most server state is intentionally lightweight:

- Auth refresh-token revocations are in memory.
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
