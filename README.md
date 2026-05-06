# BoxBox

BoxBox is a self-hosted file manager for homelab and NAS-style servers. It provides a browser UI for mounted Linux paths, large file uploads, previews, search, and background file operations.

## Features

- Browse multiple configured mount points from one web UI.
- Upload large files with chunked and resumable upload support.
- Preview common image, audio, video, PDF, and code/text files.
- Copy, move, and delete files through background jobs.
- Track job progress through WebSocket updates.
- Search directories by file or folder name.
- Configure read-only mounts, users, rate limits, and allowed origins.

## Stack

- Backend: Go 1.24, Chi, JWT, Gorilla WebSocket, Afero.
- Frontend: SvelteKit 2, Svelte 5, TypeScript, Tailwind CSS 4.
- Packaging: Bun, Docker multi-stage build, embedded static frontend.

## Repository Layout

```text
backend/      Go API server and embedded frontend host
frontend/     SvelteKit application
website/      Astro marketing site
docs/         Public project documentation
scripts/      Local development helpers
Dockerfile    Unified frontend/backend production image
```

## Development

Install frontend dependencies:

```bash
bun install --cwd frontend
```

Run frontend checks:

```bash
bun run --cwd frontend check
```

Run backend tests:

```bash
go -C backend test ./...
```

Build the frontend:

```bash
bun run --cwd frontend build
```

Build the container image:

```bash
docker build -t boxbox .
```

## Configuration

The backend reads `backend/config.yaml` plus environment variables with the `FM_` prefix. Before exposing BoxBox on a network, set a strong JWT secret, configure real users, restrict mount points to the directories you intend to manage, and review allowed origins.

See the files in `docs/` for API, architecture, configuration, Docker, development, and security notes.

## License

MIT. See [LICENSE](LICENSE).
