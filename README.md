  
<h1 align="center">BoxBox</h1>

<p align="center">
  <strong>A modern, self-hosted file manager for your homelab</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Go-00ADD8?style=flat-square&logo=go&logoColor=white" alt="Go">
  <img src="https://img.shields.io/badge/SvelteKit-FF3E00?style=flat-square&logo=svelte&logoColor=white" alt="SvelteKit">
  <img src="https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white" alt="TypeScript">
  <img src="https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white" alt="Docker">
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License">
</p>


BoxBox is a self-hosted file manager for homelab and NAS-style servers. It provides a browser UI for mounted Linux paths, large file uploads, previews, search, and background file operations.

## Quick Start

```bash
git clone https://github.com/jR4dh3y/BoxBox.git
cd BoxBox
cp .env.example .env
# Edit .env, especially FM_JWT_SECRET and FM_USERS_admin.
docker compose pull
docker compose up -d
```

Docker images are published to GitHub Container Registry at `ghcr.io/jr4dh3y/boxbox`. The provided compose file is Traefik-oriented. For a simple local `docker run` setup or local image builds, see [docs/docker.md](docs/docker.md).

## Features

- Browse multiple configured mount points from one web UI.
- Upload large files with chunked and resumable upload support.
- Preview common image, audio, video, PDF, and code/text files.
- Copy, move, and delete files through background jobs.
- Track job progress through WebSocket updates.
- Search directories by file or folder name.
- Configure read-only mounts, users, rate limits, and allowed origins.

## Repository Layout

```text
backend/      Go API server and embedded frontend host
frontend/     SvelteKit application
docs/         Public project documentation
scripts/      Local development helpers
Dockerfile    Unified frontend/backend production image
```

## Documentation

Start with [docs/index.md](docs/index.md) for install, configuration, API, development, security, architecture, and troubleshooting docs.

## License

MIT. See [LICENSE](LICENSE).
