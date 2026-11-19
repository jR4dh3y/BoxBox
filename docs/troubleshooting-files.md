# Files and Sharing Problems

Fixes for mounts, permissions, uploads, previews, share links and live updates. For problems starting BoxBox or signing in, see [Troubleshooting](troubleshooting.md).

## Mount Point Does Not Show Up

Check that the path exists inside the container:

```bash
docker compose exec boxbox ls -la /home/user
docker compose exec boxbox ls -la /media/devmon
```

Remember that `config.yaml` paths are container paths, not host paths.

A mount without an explicit `kind` that sits inside another configured mount is listed under **Places**, not **Drives**, and does not appear on **This Server**. Auto-discovered mounts and mounts nested only under a `/` mount stay drives. Set `kind: drive` on the mount point to list it as a drive.

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

## Share Link Returns 404

Unknown, expired, and revoked links all return the same `404`. Also check that:

- The link still appears under **Settings → Shared Links** for the account that created it.
- The shared path still exists and its mount point was not renamed or removed from `config.yaml`.
- The `boxbox-data` volume was kept when the container was recreated. Links are stored in `/data/shares.json`.

## WebSocket Does Not Connect

Check:

- The URL is `/api/v1/ws`.
- A token is supplied as `?token=` or `Authorization: Bearer`.
- Your reverse proxy forwards WebSocket upgrade headers.
- `allowed_origins` includes the browser origin if you configured an allow-list.

