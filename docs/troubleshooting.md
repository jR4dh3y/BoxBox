# Troubleshooting

Fixes for starting BoxBox and signing in. For mounts, uploads, previews, share links and live updates, see [Files and sharing problems](troubleshooting-files.md).

## Container Fails to Start

The default compose file publishes BoxBox on `HOST_PORT`, which defaults to `8080`. If the container fails to start, check whether that host port is already in use:

```bash
docker compose ps
docker compose logs -f boxbox
```

Change the published port in `.env` if needed:

```bash
HOST_PORT=8081
docker compose up -d
```

## Login, Logout, or Refresh Fails

`409 REFRESH_SUPERSEDED` means another request replaced the refresh cookie. The browser retries once automatically. If another refresh is still in flight, retry the action; this response does not clear the cookie or end the session.

If login, refresh, or logout returns `500` before rename, the server did not publish the session update. It sends no new token pair or `Set-Cookie` header. Refresh and logout leave the existing cookie unchanged so you can retry. Login does not clear an unrelated existing cookie. Check that `data_dir` is writable and on persistent storage, then retry.

If the browser cannot restore a session, it preserves the refresh cookie on retryable errors and shows **Retry session**. Repair storage or connectivity, then use that button without reloading. While restoration is pending, the retry and sign-in controls are disabled. An expired or invalid cookie requires a fresh sign-in; the stale retry warning is removed.

`401 SESSION_RECOVERY_REQUIRED` means rename published the update, but a later durability step failed. The server keeps its in-memory state and returns no token pair. Refresh and logout clear the cookie; login does not clear an unrelated cookie. Sign in again. Logout did not report success.

If startup reports an `auth-sessions.json` error, check the logs, `data_dir` mount, file type, permissions, and backup state. Corrupt, unreadable, non-regular, unsupported, and future-version registries stop startup. Do not replace the file with an empty file to bypass the error. Restore a valid registry backup if available.

If the registry file was missing, or a valid version 1 snapshot had an empty `sessions` map, when the server restarted, its active session list is empty. Previously issued access and refresh tokens are rejected, and users can sign in again. A zero-byte or malformed file is not an empty registry and causes startup to fail. A secret rotation is not required solely because the registry was lost. `auth-revocations.json` is ignored and is not deleted; the old denylist cannot reconstruct active sessions.

Deleting `auth-sessions.json` while the server is running does not clear the in-memory sessions. A later successful write can save them again. Do not use runtime file deletion as immediate revocation.

An older valid registry backup can revive its sessions, access tokens, and current refresh tokens. Rotate `BOXBOX_JWT_SECRET` after restoring one. If you downgrade to an older validator that ignores session IDs and the registry, rotate the secret because that validator may accept those tokens. Tokens without a session ID are rejected by the current server and are never enrolled automatically.

## Unreachable After Upgrading to v0.3.0

`v0.3.0` changed the default container port from `80` to `8080`. If the container is healthy but the browser or reverse proxy cannot reach it, the port mapping and `config.yaml` disagree. Check which port the server bound:

```bash
docker compose logs boxbox | grep "Starting HTTP server"
```

The container side of the port mapping (`HOST_PORT:<container-port>`), the healthcheck URL, and any reverse proxy that reaches the container directly must use the server port from `config.yaml` (or `BOXBOX_PORT`). `HOST_PORT` can differ; browsers on the host use it. See the [v0.3.0 upgrade notes](release-notes.md#v0-3-0-release-notes).

## Server Rejects the Configured User

BoxBox requires a bcrypt hash, not a plaintext configured password:

```bash
BOXBOX_USERS_admin='$2b$12$...'
```

```yaml
users:
  admin: "$2b$12$..."
```

Generate it with `htpasswd -bnBC 12 admin 'your-password' | cut -d: -f2`, then restart.

## `jwt_secret is required`

Set `BOXBOX_JWT_SECRET` or `jwt_secret` in `config.yaml`:

```bash
BOXBOX_JWT_SECRET="$(openssl rand -base64 32)"
```

## Deprecated `FM_*` Warnings After Upgrade

BoxBox `v0.2.0` still accepts old `FM_*` environment variables, but logs migration warnings so you can rename them safely. For example, rename `FM_JWT_SECRET` to `BOXBOX_JWT_SECRET` and `FM_USERS_admin` to `BOXBOX_USERS_admin`.

If both old and new variables are set, `BOXBOX_*` wins. Run the check-only helper from a repository checkout to inspect `.env`, `docker-compose.yml`, and old Docker volumes:

```bash
scripts/check-0.2-migration.sh
```

The helper does not edit files or Docker volumes. See [Release notes](release-notes.md#v0-2-0-upgrade-notes) for volume copy commands and failure handling.

## Frontend Is Blank in a Built Container

Rebuild the image so the SvelteKit static output is copied into the Go build stage:

```bash
docker build --no-cache -t boxbox:local .
```

If running the backend directly from source, build the frontend first if you expect embedded static assets:

```bash
bun run --cwd frontend build
```

## Health Check

Use either endpoint:

```bash
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/health
```

Expected response:

```json
{"status":"ok"}
```
