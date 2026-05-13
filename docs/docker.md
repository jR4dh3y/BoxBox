# Docker Deployment

BoxBox builds into one container: Bun compiles the SvelteKit frontend, Go embeds those static files, and the runtime image starts one HTTP server on port `80`.

## Prerequisites

- Docker 24+ or a recent Docker Engine.
- Docker Compose v2 if using `docker-compose.yml`.
- A Linux host if you want accurate system drive discovery and mount propagation.

## Simple Local Container

Use this path when you do not have Traefik set up yet.

```bash
git clone https://github.com/jR4dh3y/BoxBox.git
cd BoxBox
docker build -t boxbox:local .

docker run -d \
  --name boxbox \
  -p 8080:80 \
  -e FM_JWT_SECRET="$(openssl rand -base64 32)" \
  -e FM_USERS_admin="replace-this-password" \
  -v "$PWD/backend/config.yaml:/app/config.yaml:ro" \
  -v "$HOME:/home/user" \
  -v boxbox-data:/data \
  -v boxbox-temp:/tmp/filemanager \
  boxbox:local
```

Open `http://localhost:8080` and sign in as `admin` with the password you set in `FM_USERS_admin`.

## Compose With Traefik

The checked-in `docker-compose.yml` is configured for an external Traefik network named `proxy`.

```bash
cp .env.example .env
$EDITOR .env
docker network create proxy
docker compose up -d --build
```

Set these values in `.env` before starting:

```env
FM_JWT_SECRET=generate-a-long-random-secret
FM_USERS_admin=replace-this-password
TRAEFIK_HOST=boxbox.example.test
HOME_PATH=/home/your-user
```

If the `proxy` network already exists, `docker network create proxy` will fail with a harmless "already exists" error.

## Volumes

The default compose file mounts:

| Host path | Container path | Why |
| --- | --- | --- |
| `./backend/config.yaml` | `/app/config.yaml` | Runtime configuration. |
| `/media/devmon` | `/media/devmon` | Auto-discovered removable drives. |
| `/` | `/host_root` | Host root browsing. High blast radius. |
| `${HOME_PATH}` | `/home/user` | User home directory browsing. |
| `filemanager-temp` | `/tmp/filemanager` | Chunked upload assembly. |
| `filemanager-data` | `/data` | Persistent app data, including custom drive names. |

Use `:ro` on a volume or `read_only: true` in `config.yaml` when a path should never be modified through BoxBox.

## Mount Propagation

The compose file uses `rslave` for host mount paths so new host mounts can appear inside the container. This matters for removable drives and some NAS/rclone mounts.

```yaml
volumes:
  - /media/devmon:/media/devmon:rslave
  - /:/host_root:rslave
```

## Updating

```bash
git pull
docker compose build --no-cache
docker compose up -d
```

For a `docker run` deployment:

```bash
docker stop boxbox
docker rm boxbox
docker build -t boxbox:local .
# Re-run the docker run command with the same volumes and env values.
```

## Health and Logs

```bash
curl http://localhost:8080/health
docker compose ps
docker compose logs -f filemanager
```

The health response is:

```json
{"status":"ok"}
```

## Permission Notes

The container runs with a small set of Linux capabilities in compose so it can read and manage a variety of host-owned files without full privileged mode. If a path still returns permission errors, check the host path permissions and consider narrowing BoxBox to directories owned by a consistent user or group.
