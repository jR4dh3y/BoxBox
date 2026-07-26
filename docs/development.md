# Development

This guide covers the local BoxBox development workflow.

## Prerequisites

- Go 1.24+
- Bun 1.1+
- Node.js 22+ for Svelte tooling compatibility
- `air` for backend hot reload:

```bash
go install github.com/air-verse/air@latest
```

Make sure `$GOPATH/bin` is on your `PATH`.

## Repository Layout

```text
backend/      Go API server and embedded static host
frontend/     SvelteKit app compiled to static files
docs/         Markdown project documentation
scripts/      Local development helpers
Dockerfile    Single-container production build
```

## Install Dependencies

```bash
bun install --cwd frontend
cd backend && go mod download
```

## Run the Product App

From the repository root:

```bash
bun run dev
```

This starts:

- SvelteKit frontend through Vite.
- Go backend through `air -c backend/.air.toml`.

The backend uses `backend/config.yaml`. For a safer local setup, edit that file to point at temporary directories instead of broad host paths:

```bash
mkdir -p /tmp/boxbox/media /tmp/boxbox/documents
```

```yaml
mount_points:
  - name: "media"
    path: "/tmp/boxbox/media"
    read_only: false
  - name: "documents"
    path: "/tmp/boxbox/documents"
    read_only: false
```

## Run Pieces Separately

Backend:

```bash
cd backend
go run ./cmd/server -config config.yaml
```

Frontend:

```bash
bun run --cwd frontend dev
```

## Build

Frontend app:

```bash
bun run --cwd frontend build
```

Self-contained executable with the embedded frontend:

```bash
bun run build:single
./dist/boxbox -config backend/config.yaml
```

The build command uses Bun to generate the static frontend and Go to produce the
only runtime artifact. Bun is not required on the machine that runs the binary.

For local testing without login credentials:

```bash
./dist/boxbox --dev -config backend/config.dev.yaml
```

Development mode disables HTTP and WebSocket authentication and always binds to
`127.0.0.1`, even if the configuration specifies another host. Never use it for
a shared or production deployment.

Production container:

```bash
docker build -t boxbox:local .
```

The Dockerfile builds the frontend first, copies `frontend/build` into `backend/internal/static/dist`, and compiles the Go server with embedded assets.

## Test and Validate

Backend:

```bash
cd backend
go test ./...
```

Frontend checks:

```bash
bun run --cwd frontend check
bun run --cwd frontend build
```

## Backend Patterns

Use the existing Handler -> Service -> Model/Filesystem shape:

- Handlers parse requests and write responses.
- Services own business logic and filesystem work.
- Models define request, response, and domain types.
- Shared constants belong in `backend/internal/config/constants.go`.
- File operations should go through `internal/pkg/filesystem` and existing validators.

## Frontend Patterns

- Use Svelte 5 patterns already present in the repo.
- API calls belong in `frontend/src/lib/api`.
- Shared UI primitives belong in `frontend/src/lib/components/ui`.
- Formatting belongs in `frontend/src/lib/utils/format.ts`.
- File type logic belongs in `frontend/src/lib/utils/fileTypes.ts`.
- Central config belongs in `frontend/src/lib/config.ts`.
