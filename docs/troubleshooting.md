# Troubleshooting

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

## Login Uses `admin:admin`

That means no users were configured. Set a user through env or YAML:

```bash
BOXBOX_USERS_admin="a-long-password"
```

```yaml
users:
  admin: "a-long-password"
```

Restart after changing credentials.

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

The helper does not edit files or Docker volumes. See [Release workflow](/docs/release/) for volume copy commands and failure handling.

## Mount Point Does Not Show Up

Check that the path exists inside the container:

```bash
docker compose exec boxbox ls -la /home/user
docker compose exec boxbox ls -la /media/devmon
```

Remember that `config.yaml` paths are container paths, not host paths.

## Permission Denied

Check host permissions and whether the container path is mounted read-only:

```bash
ls -la /path/on/host
docker compose exec boxbox id
docker compose exec boxbox ls -la /path/in/container
```

For sensitive paths, prefer keeping the mount read-only rather than expanding container permissions.

## Upload Fails

Check:

- The destination mount is not `read_only`.
- `/tmp/boxbox` has enough space for chunks.
- Your reverse proxy request size limit allows the upload.
- `max_upload_mb` is large enough.
- The browser request includes `X-Chunk-Size` and `X-Total-Size`.

Logs:

```bash
docker compose logs -f boxbox
```

## Preview or Download Fails

Preview and download endpoints require a valid token. The web app appends a token query parameter automatically for direct media URLs.

Direct API callers can use:

```http
Authorization: Bearer <access-token>
```

Or:

```text
/api/v1/stream/preview/home/video.mp4?token=<access-token>
```

## WebSocket Does Not Connect

Check:

- The URL is `/api/v1/ws`.
- A token is supplied as `?token=` or `Authorization: Bearer`.
- Your reverse proxy forwards WebSocket upgrade headers.
- `allowed_origins` includes the browser origin if you configured an allow-list.

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
