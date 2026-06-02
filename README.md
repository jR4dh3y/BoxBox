  
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

The preferred deployment path is Docker Compose using the published GitHub Container Registry image. No source checkout is required.

```bash
mkdir -p boxbox
cd boxbox

curl -fsSLO https://raw.githubusercontent.com/jR4dh3y/BoxBox/master/docker-compose.yml
curl -fsSLO https://raw.githubusercontent.com/jR4dh3y/BoxBox/master/.env.example
mkdir -p backend
curl -fsSL https://raw.githubusercontent.com/jR4dh3y/BoxBox/master/backend/config.yaml -o backend/config.yaml

cp .env.example .env
$EDITOR .env

docker compose pull
docker compose up -d
```

Open `http://localhost:8080` and sign in as `admin` with the password from `FM_USERS_admin`. For reverse proxy examples, local source builds, and update workflows, see [docs/docker.md](docs/docker.md).

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

its here -  [boxbox.radhey.dev/docs/](https://boxbox.radhey.dev/docs/)

## License

MIT. See [LICENSE](LICENSE).
