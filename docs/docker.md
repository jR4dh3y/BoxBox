# Docker Deployment

BoxBox builds into one container: Bun compiles the SvelteKit frontend, Go embeds those static files, and the runtime image starts one HTTP server on port `80`.

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
BOXBOX_USERS_admin=replace-this-password
HOST_PORT=8080
# Optional: defaults to the HOME of the user running docker compose.
# HOME_PATH=/home/your-user
# Optional: point sidebar shortcuts somewhere other than HOME_PATH/<Folder>.
# DOWNLOADS_PATH=/mnt/downloads
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:latest
```

Open `http://localhost:8080`, or use the host and port you configured with `HOST_PORT`.

## Published Images

Release images are published to GitHub Container Registry:

```bash
docker pull ghcr.io/jr4dh3y/boxbox:latest
```

Stable release tags update `latest`. Prerelease tags such as `v0.2.0-rc.1` publish versioned images without moving `latest`, and test branch images publish under branch-scoped tags such as `branch-test-my-change`.

See [Release workflow](/docs/release/) for the full test-branch and stable-release process.

## Optional: Reverse Proxy or Traefik

The default compose file uses normal host port binding and does not require Traefik. If you want to put BoxBox behind Traefik, remove the `ports` entry, attach the service to your Traefik network, and add labels for your router.

```yaml
services:
  boxbox:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.boxbox.rule=Host(`boxbox.example.test`)"
      - "traefik.http.routers.boxbox.entrypoints=web"
      - "traefik.http.services.boxbox.loadbalancer.server.port=80"
    networks:
      - proxy

networks:
  proxy:
    external: true
```

For public or semi-public access, put TLS and access controls on your reverse proxy before exposing BoxBox beyond a trusted network.

## Alternative: Simple Local Container

Use this path when you want a quick local test without Compose. It still uses the published GHCR image.

```bash
mkdir -p boxbox
cd boxbox

cat > config.yaml <<'EOF'
port: 80
host: "0.0.0.0"
jwt_secret: "replace-this-with-a-long-random-secret"
mount_points:
  - name: "home"
    path: "/home/user"
    read_only: false
EOF

docker pull ghcr.io/jr4dh3y/boxbox:latest

docker run -d \
  --name boxbox \
  -p 8080:80 \
  -e BOXBOX_JWT_SECRET="$(openssl rand -base64 32)" \
  -e BOXBOX_USERS_admin="replace-this-password" \
  -v "$PWD/config.yaml:/app/config.yaml:ro" \
  -v "$HOME:/home/user" \
  -v boxbox-data:/data \
  -v boxbox-temp:/tmp/boxbox \
  ghcr.io/jr4dh3y/boxbox:latest
```

Open `http://localhost:8080` and sign in as `admin` with the password you set in `BOXBOX_USERS_admin`.

## Alternative: Local Source Builds

Cloning the repository is supported, but it is not the preferred deployment method. Use this path only when you want to build the image from source instead of pulling the published GHCR image.

```bash
git clone https://github.com/jR4dh3y/BoxBox.git
cd BoxBox
docker build -t boxbox:local .
```

For compose, set `BOXBOX_IMAGE=boxbox:local` in `.env`, then run `docker compose up -d`.

## Volumes

The default compose file mounts:

| Host path | Container path | Why |
| --- | --- | --- |
| `./backend/config.yaml` | `/app/config.yaml` | Runtime configuration. |
| `/media/devmon` | `/media/devmon` | Auto-discovered removable drives. |
| `/` | `/host_root` | Host root browsing. High blast radius. |
| `${HOME_PATH}` or host `$HOME` | `/home/user` | User home directory browsing. |
| `${DESKTOP_PATH}` or `$HOME/Desktop` | `/home/user/Desktop` | Desktop sidebar shortcut. |
| `${DOWNLOADS_PATH}` or `$HOME/Downloads` | `/home/user/Downloads` | Downloads sidebar shortcut. |
| `${DOCUMENTS_PATH}` or `$HOME/Documents` | `/home/user/Documents` | Documents sidebar shortcut. |
| `${MUSIC_PATH}` or `$HOME/Music` | `/home/user/Music` | Music sidebar shortcut. |
| `${PICTURES_PATH}` or `$HOME/Pictures` | `/home/user/Pictures` | Pictures sidebar shortcut. |
| `${VIDEOS_PATH}` or `$HOME/Videos` | `/home/user/Videos` | Videos sidebar shortcut. |
| `boxbox-temp` | `/tmp/boxbox` | Chunked upload assembly. |
| `boxbox-data` | `/data` | Persistent app data, including custom drive names. |

Use `:ro` on a volume or `read_only: true` in `config.yaml` when a path should never be modified through BoxBox.

The sidebar's default folders are configured as normal mount points named `desktop`, `downloads`, `documents`, `music`, `pictures`, and `videos`. To remap one shortcut, set the matching host path in `.env` and restart, for example `DOWNLOADS_PATH=/srv/incoming`. Do not use `~` in `.env`; use an absolute path like `/home/alex/Downloads`.

## Mount Propagation

The compose file uses `rslave` for host mount paths so new host mounts can appear inside the container. This matters for removable drives and some NAS/rclone mounts.

```yaml
volumes:
  - /media/devmon:/media/devmon:rslave
  - /:/host_root:rslave
```

## Updating

```bash
docker compose pull
docker compose up -d
```

For local source builds:

```bash
git pull
docker build --no-cache -t boxbox:local .
docker compose up -d
```

For a `docker run` deployment:

```bash
docker pull ghcr.io/jr4dh3y/boxbox:latest
docker stop boxbox
docker rm boxbox
# Re-run the docker run command with the same volumes and env values.
```

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

The container runs with a small set of Linux capabilities in compose so it can read and manage a variety of host-owned files without full privileged mode. If a path still returns permission errors, check the host path permissions and consider narrowing BoxBox to directories owned by a consistent user or group.
