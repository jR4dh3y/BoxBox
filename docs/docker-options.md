# Docker Options

Other ways to run the container, and the details behind its volumes. For the standard Compose setup, see [Docker deployment](docker.md).

## Reverse Proxy or Traefik

The default compose file uses normal host port binding and does not require Traefik. If you want to put BoxBox behind Traefik, remove the `ports` entry, attach the service to your Traefik network, and add labels for your router.

```yaml
services:
  boxbox:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.boxbox.rule=Host(`boxbox.example.test`)"
      - "traefik.http.routers.boxbox.entrypoints=web"
      - "traefik.http.services.boxbox.loadbalancer.server.port=8080"
    networks:
      - proxy

networks:
  proxy:
    external: true
```

For public or semi-public access, put TLS and access controls on your reverse proxy before exposing BoxBox beyond a trusted network.

## Simple Local Container

Use this path when you want a quick local test without Compose. It still uses the published GHCR image.

```bash
mkdir -p boxbox
cd boxbox

cat > config.yaml <<'EOF'
port: 8080
host: "0.0.0.0"
jwt_secret: "replace-this-with-a-long-random-secret"
mount_points:
  - name: "home"
    path: "/home/user"
    read_only: false
EOF

docker pull ghcr.io/jr4dh3y/boxbox:v0.4.0

docker run -d \
  --name boxbox \
  --user 10001:10001 \
  --cap-drop ALL \
  --security-opt no-new-privileges:true \
  -p 127.0.0.1:8080:8080 \
  -e BOXBOX_JWT_SECRET="$(openssl rand -base64 32)" \
  -e BOXBOX_USERS_admin='paste-a-bcrypt-hash-here' \
  -v "$PWD/config.yaml:/app/config.yaml:ro" \
  -v "$HOME:/home/user" \
  -v boxbox-data:/data \
  -v boxbox-temp:/tmp/boxbox \
  ghcr.io/jr4dh3y/boxbox:v0.4.0
```

Open `http://localhost:8080` and sign in as `admin` with the plaintext password used to generate the hash. The port is bound to `127.0.0.1`, so only this machine can reach it.

The container runs as UID/GID `10001:10001`, so that identity must be able to read the mounted home folder; a `0700` home owned by your user shows as empty. Grant that identity access to the folder (see [Permission notes](docker.md#permission-notes)). Do not change `--user` here: the `boxbox-data` and `boxbox-temp` volumes belong to `10001`, and BoxBox stops at startup if it cannot set up `/data`.

## Local Source Builds

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
```

