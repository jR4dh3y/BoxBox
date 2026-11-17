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
chunk_size_mb: 10

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
| `data_dir` | `/data` | Dedicated directory for persisted settings, shares, and the active authentication-session registry. Relative paths resolve from the process working directory, not the YAML directory. Use one server process per directory. |
| `rate_limit_rps` | `2` | Auth endpoint requests per second per client IP. |
| `allowed_origins` | `[]` | WebSocket origin allow-list. Empty enforces same-origin; `*` explicitly allows all. |
| `trusted_proxies` | `[]` | Proxy IPs/CIDRs allowed to supply client-IP headers. |
| `public_url` | `""` | External origin, such as `https://files.example.com`, used for absolute URLs in share link previews. Empty derives it from each request's `Host` and `X-Forwarded-Proto`. Must have no path: BoxBox is served from the root of its origin. |
| `allow_root_mount` | `false` | Explicit override required for a mount resolving to `/`. |
| `max_upload_mb` | `10240` | Maximum upload size in MiB. |
| `chunk_size_mb` | `10` | Chunk size in MiB that the browser uses for uploads. Lower it when a reverse proxy limits request body size. |

An empty `data_dir` uses `/data`. Filesystem roots, including existing symlink aliases to a root, are rejected before persistence services initialize. Use a dedicated directory: the service restricts its permissions to `0700`, so do not point it at a shared home, application, or system directory.

On Linux, the auth service loads or creates the version 1 `auth-sessions.json` registry under `data_dir` at startup. Its JSON shape is `{"version":1,"sessions":{"<sid>":{"username":"...","refreshHash":"<sha256>","expiresAt":"<timestamp>"}}}`. It stores no raw JWTs. Startup stops if the file is corrupt, unreadable, non-regular, unsupported, or from a future version. Development mode also needs a writable `data_dir` because `--dev` does not bypass registry initialization.

If `auth-sessions.json` is missing, or contains a valid version 1 snapshot with an empty `sessions` map, the server starts with no active sessions. Previously issued access and refresh tokens fail after restart; users can sign in again. A zero-byte or malformed file is not an empty registry and causes startup to fail. The server does not check the registry file on each request. Deleting it while the server runs does not clear in-memory sessions, and a later successful auth write can persist those sessions again. Do not treat runtime file deletion as immediate session revocation.

An older valid backup can restore sessions, including their access tokens and current refresh tokens. Rotate `BOXBOX_JWT_SECRET` after restoring an older backup. `auth-revocations.json` is ignored and is not deleted; its denylist cannot be converted into active sessions. Legacy JWTs without a session ID are rejected rather than enrolled. Incompatible experimental session-store formats fail startup; the server does not migrate them or fall back to another store. If you downgrade to a validator that ignores session IDs and the registry, rotate the JWT secret because that validator may accept those tokens.

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
| `BOXBOX_PUBLIC_URL` | `public_url` |
| `BOXBOX_ALLOW_ROOT_MOUNT` | `allow_root_mount` |
| `BOXBOX_DATA_DIR` | `data_dir` |
| `BOXBOX_USERS_<username>` | `users.<username>` |

`CONFIG_PATH` is also supported as a convenience for selecting the YAML file path.

For `v0.2.0`, deprecated `FM_*` variables still work as aliases for older deployments. Rename them to `BOXBOX_*` before a future breaking release. If both old and new names are set, `BOXBOX_*` wins and the server logs a migration warning. See [Release notes](release-notes.md#v0-2-0-upgrade-notes) for the full upgrade notes and the check-only migration helper.

Mount points, read-only mounts and Docker path mapping are covered in [Mount points](mounts.md).

## Restart After Changes

Configuration is loaded on startup. Restart the container or process after editing YAML or environment variables.

```bash
docker compose restart boxbox
```
