# Quick Start

Use this page to get BoxBox running quickly with Docker Compose and the published GitHub Container Registry image. This is the preferred deployment path and does not require cloning the repository. For the full deployment reference, see [Docker deployment](/docs/docker/).

## Quick Install with Compose

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

Open `http://localhost:8080` and sign in as `admin` with the password you set in `FM_USERS_admin`.

For a single-container `docker run` command or local source build, use the separate alternatives in [Docker deployment](/docs/docker/).

## Required Edits

Before starting a reachable deployment, change at least:

```bash
FM_JWT_SECRET="$(openssl rand -base64 32)"
FM_USERS_admin="a-long-unique-password"
HOST_PORT=8080
```

Also edit `backend/config.yaml` if you want to expose different paths. Add more entries under `mount_points` and bind each host path in `docker-compose.yml`.

## Open BoxBox

For the quick install above, open:

```text
http://localhost:8080
```
