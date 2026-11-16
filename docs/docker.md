# Docker Deployment

BoxBox builds into one container: Bun compiles the SvelteKit frontend, Go embeds those static files, and the runtime image starts one HTTP server on port `8080`.

## Prerequisites

- Docker 24+ or a recent Docker Engine.
- Docker Compose v2 if using `docker-compose.yml`.
- A Linux host if you want accurate system drive discovery and mount propagation.

## Preferred Deployment

Deploy BoxBox with Docker Compose using the published GitHub Container Registry image. This is the recommended path for servers because it does not require cloning the repository and keeps updates simple.

Download the deployment files, edit the environment, then start the published image:

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

Set these values in `.env` before starting:

```env
BOXBOX_JWT_SECRET=generate-a-long-random-secret
BOXBOX_USERS_admin='$2b$12$paste-generated-hash-here'
HOST_PORT=8080
# Optional: defaults to the HOME of the user running docker compose.
# HOME_PATH=/home/your-user
# Optional: point sidebar shortcuts somewhere other than HOME_PATH/<Folder>.
# DOWNLOADS_PATH=/mnt/downloads
PUID=10001
PGID=10001
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.3.1
```

Generate the password hash with `htpasswd -bnBC 12 admin 'your-password' | cut -d: -f2`. Keep the hash single-quoted because bcrypt hashes contain `$`.

Open `http://localhost:8080`, or use the host and port you configured with `HOST_PORT`.

## Published Images

Release images are published to GitHub Container Registry:

```bash
docker pull ghcr.io/jr4dh3y/boxbox:v0.3.1
```

Stable release tags update `latest`. Prerelease tags such as `v0.3.1-rc.1` publish versioned images without moving `latest`, nightly builds publish to the dedicated `boxbox-nightly` package, and test branch images publish under branch-scoped tags in the `boxbox-branch` package, such as `ghcr.io/jr4dh3y/boxbox-branch:branch-test-my-change`.

See [Release workflow](/docs/release/) for the full test-branch and stable-release process.

If you are upgrading an older deployment that still uses `FM_*`, `filemanager`, or `filemanager-*` names, see the `v0.2.0` notes in [Release notes](/docs/release-notes/#v0-2-0-upgrade-notes). The `scripts/check-0.2-migration.sh` helper is check-only and prints the manual changes without editing files or Docker volumes.

For a reverse proxy, a single `docker run` container, local source builds, volumes and mount propagation, see [Docker options](/docs/docker-options/).

## Updating

```bash
docker compose pull
docker compose up -d
```

For `v0.2.0`, old `FM_*` environment variables still work as deprecated aliases, but rename them to `BOXBOX_*`. `BOXBOX_*` values take precedence when both are set.

For local source builds:

```bash
git pull
docker build --no-cache -t boxbox:local .
docker compose up -d
```

For a `docker run` deployment:

```bash
docker pull ghcr.io/jr4dh3y/boxbox:v0.3.1
docker stop boxbox
docker rm boxbox
# Re-run the docker run command with the same volumes and env values.
```

For the `v0.3.1` security release, read the upgrade and session-registry guidance in [Release notes](/docs/release-notes/#v0-3-1-security-release).

## Health and Logs

```bash
curl http://localhost:8080/health
docker compose ps
docker compose logs -f boxbox
```

The health response is:

```json
{"status":"ok"}
```

## Permission Notes

The container runs as UID/GID `10001:10001` by default and drops all Linux capabilities. Set `PUID`/`PGID` before first start if your mounted files use another identity. BoxBox does not chown host paths or existing named volumes; grant that identity only the host permissions it needs.
