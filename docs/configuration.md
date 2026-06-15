# Configuration

BoxBox is configured with a YAML file plus optional environment variable overrides.

## Config File Location

The server looks for configuration in this order:

1. The `-config` flag, for example `/app/server -config /app/config.yaml`.
2. The `CONFIG_PATH` environment variable.
3. `config.yaml` in the current directory.
4. `./config/config.yaml`.
5. `/etc/filemanager/config.yaml`.

The Docker image includes `/app/config.yaml`, and the compose file bind-mounts `./backend/config.yaml` there.

## Full Example

```yaml
port: 80
host: "0.0.0.0"

jwt_secret: "replace-with-a-long-random-secret"

users:
  admin: "replace-with-a-long-unique-password"
  media: "another-password"

rate_limit_rps: 10

allowed_origins:
  - "http://localhost:8080"
  - "https://boxbox.example.com"
  - "*.internal.example.com"

max_upload_mb: 10240
chunk_size_mb: 5

mount_points:
  - name: "drives"
    path: "/media/devmon"
    read_only: false
    auto_discover: true

  - name: "home"
    path: "/home/user"
    read_only: false

  - name: "downloads"
    path: "/home/user/Downloads"
    read_only: false

  - name: "backups"
    path: "/mnt/backups"
    read_only: true
```

## Server Options

| Key | Default | Description |
| --- | --- | --- |
| `port` | `80` | HTTP port inside the container or process. |
| `host` | `0.0.0.0` | Bind address. |
| `jwt_secret` | Required | JWT signing secret. Use a long random value. |
| `rate_limit_rps` | `10` | Auth endpoint requests per second per client IP. |
| `allowed_origins` | `[]` | WebSocket origin allow-list. Empty allows all origins. |
| `max_upload_mb` | `10240` | Maximum upload size in MiB. |
| `chunk_size_mb` | `5` | Backend chunk configuration value. Browser uploads currently send 10 MiB chunks by default. |

## Users

Users are configured as a map of username to password:

```yaml
users:
  admin: "a-long-password"
```

You can also set users through environment variables:

```bash
FM_USERS_admin="a-long-password"
FM_USERS_radhey="another-password"
```

If no users are configured, BoxBox falls back to `admin:admin` and logs a warning. Treat that as a local development fallback only.

## Environment Variables

Environment overrides use the `FM_` prefix:

| Environment variable | Config key |
| --- | --- |
| `FM_JWT_SECRET` | `jwt_secret` |
| `FM_PORT` | `port` |
| `FM_HOST` | `host` |
| `FM_RATE_LIMIT_RPS` | `rate_limit_rps` |
| `FM_MAX_UPLOAD_MB` | `max_upload_mb` |
| `FM_CHUNK_SIZE_MB` | `chunk_size_mb` |
| `FM_ALLOWED_ORIGINS` | `allowed_origins`, comma-separated |
| `FM_USERS_<username>` | `users.<username>` |

`CONFIG_PATH` is also supported as a convenience for selecting the YAML file path.

## Mount Points

Every mounted path must have a unique `name` and an absolute container `path`.

```yaml
mount_points:
  - name: "media"
    path: "/srv/media"
    read_only: false
```

The `name` is the first segment in API paths. For example, a mount named `media` is browsed through `/api/v1/files/media`.

The built-in sidebar shortcuts use mount names `desktop`, `downloads`, `documents`, `music`, `pictures`, and `videos`. Keep those names if you want the default sidebar entries to open custom locations.

### Read-Only Mounts

Use read-only mounts for backups and other paths that should not be modified:

```yaml
mount_points:
  - name: "backups"
    path: "/mnt/backups"
    read_only: true
```

Writes, uploads, renames, moves, and deletes under that mount return `403`.

### Auto-Discovery

Set `auto_discover: true` on a parent directory when you want BoxBox to expose mounted subdirectories as individual mount points.

```yaml
mount_points:
  - name: "drives"
    path: "/media/devmon"
    read_only: false
    auto_discover: true
```

On Linux, BoxBox checks real system mounts and filters virtual filesystems before adding discovered entries.

## Docker Path Mapping

Paths in `config.yaml` are container paths. Bind host directories to those paths:

```yaml
services:
  filemanager:
    volumes:
      - /srv/media:/srv/media
      - /mnt/backups:/mnt/backups:ro
```

Then configure:

```yaml
mount_points:
  - name: "media"
    path: "/srv/media"
    read_only: false
  - name: "backups"
    path: "/mnt/backups"
    read_only: true
```

## Restart After Changes

Configuration is loaded on startup. Restart the container or process after editing YAML or environment variables.

```bash
docker compose restart filemanager
```
