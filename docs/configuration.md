# Configuration

BoxBox is configured with a YAML file plus optional environment variable overrides.

## Config File Location

The server looks for configuration in this order:

1. The `-config` flag, for example `/app/server -config /app/config.yaml`.
2. The `CONFIG_PATH` environment variable.
3. `config.yaml` in the current directory.
4. `./config/config.yaml`.
5. `/etc/boxbox/config.yaml`.
6. `/etc/filemanager/config.yaml` as a deprecated `v0.2.0` compatibility fallback.

The Docker image includes `/app/config.yaml`, and the compose file bind-mounts `./backend/config.yaml` there.

## Full Example

```yaml
port: 8080
host: "0.0.0.0"

jwt_secret: "replace-with-a-long-random-secret"

users:
  admin: "$2b$12$..."
  media: "$2b$12$..."

rate_limit_rps: 2

allowed_origins:
  - "http://localhost:8080"
  - "https://boxbox.example.com"
  - "*.internal.example.com"

trusted_proxies:
  - "172.18.0.0/16"

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
| `port` | `8080` | HTTP port inside the container or process. |
| `host` | `0.0.0.0` | Bind address. |
| `jwt_secret` | Required | JWT signing secret of at least 32 bytes. |
| `rate_limit_rps` | `2` | Auth endpoint requests per second per client IP. |
| `allowed_origins` | `[]` | WebSocket origin allow-list. Empty enforces same-origin; `*` explicitly allows all. |
| `trusted_proxies` | `[]` | Proxy IPs/CIDRs allowed to supply client-IP headers. |
| `allow_root_mount` | `false` | Explicit override required for a mount resolving to `/`. |
| `max_upload_mb` | `10240` | Maximum upload size in MiB. |
| `chunk_size_mb` | `5` | Backend chunk configuration value. Browser uploads currently send 10 MiB chunks by default. |

## Users

Users are configured as a map of username to bcrypt hash:

```yaml
users:
  admin: "$2b$12$..."
```

You can also set users through environment variables:

```bash
BOXBOX_USERS_admin='$2b$12$...'
BOXBOX_USERS_radhey='$2b$12$...'
```

Generate a hash with `htpasswd -bnBC 12 admin 'your-password' | cut -d: -f2`. If no users are configured, or a configured value is plaintext, BoxBox fails to start.

## Environment Variables

Environment overrides use the `BOXBOX_` prefix:

| Environment variable | Config key |
| --- | --- |
| `BOXBOX_JWT_SECRET` | `jwt_secret` |
| `BOXBOX_PORT` | `port` |
| `BOXBOX_HOST` | `host` |
| `BOXBOX_RATE_LIMIT_RPS` | `rate_limit_rps` |
| `BOXBOX_MAX_UPLOAD_MB` | `max_upload_mb` |
| `BOXBOX_CHUNK_SIZE_MB` | `chunk_size_mb` |
| `BOXBOX_ALLOWED_ORIGINS` | `allowed_origins`, comma-separated |
| `BOXBOX_TRUSTED_PROXIES` | `trusted_proxies`, comma-separated CIDRs |
| `BOXBOX_ALLOW_ROOT_MOUNT` | `allow_root_mount` |
| `BOXBOX_USERS_<username>` | `users.<username>` |

`CONFIG_PATH` is also supported as a convenience for selecting the YAML file path.

For `v0.2.0`, deprecated `FM_*` variables still work as aliases for older deployments. Rename them to `BOXBOX_*` before a future breaking release. If both old and new names are set, `BOXBOX_*` wins and the server logs a migration warning. See [Release workflow](/docs/release/) for the full upgrade notes and the check-only migration helper.

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
  boxbox:
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
docker compose restart boxbox
```
