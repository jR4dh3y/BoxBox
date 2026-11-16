# Release Notes

What changed in each release and what to do when you upgrade. To ship a release, see [Release workflow](release.md).

## v0.3.2 Desktop Release

The next release adds the Linux x86_64 desktop app alongside the existing amd64/arm64 server packages. Check [Downloads](/download/) for published assets; the website does not offer a desktop link until a real release asset exists. See [Linux Desktop](desktop.md) for dependencies and pairing.

`VERSION` is the release source of truth and also sets the Electrobun app version. Build the full distribution locally with:

```bash
bun scripts/package-release.ts
```

This builds the desktop runtime, a self-contained installer archive, both standalone server targets, `SHA256SUMS` and `release-manifest.json` under `dist/release/`. Every packaging run builds the desktop from the current checkout so its manifest cannot describe a stale bundle. A local working-tree build is marked dirty in its manifest; official tag builds require clean source.

For a public stable release, merge both the desktop PR and its website/release follow-up into master first. Set `VERSION` to the intended version, keep `docs/releases/v0.3.2.md` current, and tag the tested master commit with `v0.3.2`. The release workflow verifies that the tag matches VERSION and belongs to master. It validates the server, website, Docker build, desktop tests and artifacts before publishing. The GitHub release stays a draft while files upload, then becomes public only after the Docker image succeeds. Published releases are not overwritten.

Manual workflow dispatch prepares CI artifacts without creating a public release or moving Docker `latest`. Only a version tag publishes. Publishing an unmerged desktop branch as stable is rejected.

## v0.3.1 Security Release

`v0.3.1` fixes session invalidation and recovery for GHSA-jj9r-xc63-6c4p. All users must sign in again after upgrading.

### Fixes

- Authentication uses the version 1 active-session registry, `auth-sessions.json`, under `data_dir`. The file stores session IDs, usernames, SHA-256 hashes of current refresh tokens, and deadlines, never raw JWTs. Access and refresh tokens both require an active session. Logout removes the whole session and invalidates its access tokens; separate logins remain active.
- Login, refresh, and logout persist registry changes before reporting success. Rename publishes each change after the temporary file is synced and closed; success also requires syncing the parent directory. A failure before rename returns `500` without a token pair or `Set-Cookie`. Refresh and logout preserve the existing cookie for retry, while login does not clear an unrelated cookie. A failure after rename returns `401 SESSION_RECOVERY_REQUIRED` without a token pair. Refresh and logout clear the cookie; login does not clear an unrelated cookie.

### Upgrade notes

- Keep `data_dir` on persistent storage and run one server process per data directory. The active-session registry, `auth-sessions.json`, lives under `data_dir`.
- On startup, a missing registry or a valid version 1 snapshot with an empty `sessions` map rejects old access and refresh tokens. Users can sign in again; registry loss alone does not require secret rotation.
- A zero-byte, malformed, unreadable, non-regular, unsupported, or future-version registry stops startup. Restore a valid backup rather than deleting or resetting the file.
- Restoring an older valid backup can revive its sessions, access tokens, and current refresh tokens. Rotate `BOXBOX_JWT_SECRET` after rollback. A registry deleted while the server runs does not clear its in-memory sessions or trigger a disk reload.
- `auth-revocations.json` is ignored and is not deleted. The old denylist cannot be converted into active sessions. Legacy tokens without a session ID are rejected.
- A downgrade to a validator that ignores session IDs and the registry may accept tokens. Rotate `BOXBOX_JWT_SECRET` when downgrading.

After the `v0.3.1` image is published, update an existing Compose deployment:

```bash
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.3.1 docker compose pull
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.3.1 docker compose up -d
```

The stable Docker workflow runs from the `v0.3.1` tag and publishes:

```text
ghcr.io/jr4dh3y/boxbox:v0.3.1
ghcr.io/jr4dh3y/boxbox:latest
```

## v0.3.0 Release Notes

`v0.3.0` adds file and folder sharing, folder uploads, separate Drives and Places in the sidebar, and a redesigned share page. It also moves the default container port from `80` to `8080`.

### Highlights

- Share a file or folder with a token link. Folder links offer View only, Upload only, Upload + delete, and Full access. Links can expire, can be revoked, and are managed from **Settings → Shared Links**.
- Recipients get a public share page with a path bar, file list, previews, a code editor for text files (read-only unless the link has full access), previous/next navigation, and ZIP downloads for folders.
- Upload whole folders from the file browser or a full-access share page by choosing a folder or dragging one in. Nested paths stay intact.
- The sidebar lists Drives and Places separately. A mount inside another mount, such as `Downloads` inside `home`, is a place. Set `kind: drive` or `kind: place` on a mount point to choose explicitly.
- Each user's wallpaper is stored on the server and shown on their share pages.
- `auto_discover` mounts are no longer dropped at startup, so discovered drives appear again. Inaccessible discovered mounts are skipped.
- `chunk_size_mb` now controls browser upload chunks. The default is `10`.
- Context-menu shortcuts (F2, Delete, Ctrl/Cmd+C/X/V) work from the keyboard, and dialogs take focus when they open.
- The backend now builds with Go 1.27. The frontend state moved to Svelte 5 runes.

### Upgrade notes

- **The container now listens on `8080`.** The container side of the port mapping, the healthcheck, and any reverse proxy that reaches the container directly must use the same port as `port` in `config.yaml` (or `BOXBOX_PORT`). The simplest path is to download the new `docker-compose.yml` and set `port: 8080` in your bind-mounted `config.yaml`. If you keep your v0.2.2 `docker-compose.yml`, which maps and health-checks port `80`, set `BOXBOX_PORT=80` instead.
- Share links and stored wallpapers live in the data directory (`/data` in the container), in `shares.json` and the `wallpapers/` folder. Keep the `boxbox-data` volume when you recreate the container, or existing links stop working.
- Nested mounts now appear under Places instead of Drives, and only drives appear on **This Server**. Add `kind: drive` to any mount you want to keep listed as a drive.
- Back up `shares.json` before downgrading. Older versions cannot keep the per-link upload/delete permissions and upload caps if they rewrite the file.
- Read the [security guide](security.md#share-links) for how share tokens, rate limits, and recipient access work.

The stable Docker workflow runs from the `v0.3.0` tag and publishes:

```text
ghcr.io/jr4dh3y/boxbox:v0.3.0
ghcr.io/jr4dh3y/boxbox:latest
```

To update an existing deployment after the image is published:

```bash
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.3.0 docker compose pull
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.3.0 docker compose up -d
```

To roll back, set `BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.2.2` and keep the server on the port your compose file maps and health-checks. With the v0.3.0 compose file, keep `port: 8080` in `config.yaml` (or set `BOXBOX_PORT=8080`); v0.2.2 honors both. Then pull and restart.

## v0.2.2 Release Notes

`v0.2.2` is a security and deployment-focused release. It adds the single-binary build and loopback-only development mode, refactors browsing/streaming and uploads, fixes upload finalization races, and adds stronger mount, job, preview, authentication, and container isolation.

Before upgrading, read the [security guide](security.md) and update deployment configuration:

- Configure every user with a bcrypt password hash.
- Set a non-placeholder `BOXBOX_JWT_SECRET` of at least 32 bytes.
- Ensure bind mounts are accessible to the default UID/GID `10001:10001`, or set `PUID` and `PGID`.
- Do not expect the old host-root mount; it is removed by default and requires explicit `allow_root_mount: true`.
- Existing browser sessions must authenticate again because refresh tokens moved to an HttpOnly cookie.

The stable Docker workflow runs from the `v0.2.2` tag and publishes:

```text
ghcr.io/jr4dh3y/boxbox:v0.2.2
ghcr.io/jr4dh3y/boxbox:latest
```

To update an existing deployment after the image is published:

```bash
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.2.2 docker compose pull
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:v0.2.2 docker compose up -d
```

## v0.2.0 Upgrade Notes

BoxBox `v0.2.0` keeps old FileManager deployment names working so upgrades do not hard-break existing servers. The old names are deprecated and should be renamed before a future breaking release.

Environment variables using the old `FM_` prefix still work in `v0.2.0`, but the matching `BOXBOX_` variable always takes precedence when both are set:

| Deprecated | Use instead |
| --- | --- |
| `FM_JWT_SECRET` | `BOXBOX_JWT_SECRET` |
| `FM_USERS_<username>` | `BOXBOX_USERS_<username>` |
| `FM_RATE_LIMIT_RPS` | `BOXBOX_RATE_LIMIT_RPS` |
| `FM_ALLOWED_ORIGINS` | `BOXBOX_ALLOWED_ORIGINS` |
| `FM_PORT` | `BOXBOX_PORT` |
| `FM_HOST` | `BOXBOX_HOST` |
| `FM_MAX_UPLOAD_MB` | `BOXBOX_MAX_UPLOAD_MB` |
| `FM_CHUNK_SIZE_MB` | `BOXBOX_CHUNK_SIZE_MB` |

The old `/etc/filemanager/config.yaml` path also still works as a fallback, but move it to `/etc/boxbox/config.yaml`.

Run the check-only migration helper from a repository checkout to inspect a deployment directory:

```bash
scripts/check-0.2-migration.sh
```

The script does not edit `.env`, rewrite Compose files, stop containers, or change Docker volumes. It exits `0` when no legacy settings are found, `2` when migration work is detected, and `1` only for script/runtime errors.

If you used old Docker named volumes, copy them manually only when you are ready to switch Compose to the new names. Docker Compose may prefix volume names with the project name; use `scripts/check-0.2-migration.sh` to print the actual volume names on your host, and set them below. The new names must be the ones the new Compose project mounts.

`docker volume inspect` stops the copy when an old volume name is wrong. Without it, `docker run` would create an empty volume under that name and the copy would "succeed" with nothing in it.

```bash
OLD_DATA=filemanager-data   NEW_DATA=boxbox-data
OLD_TEMP=filemanager-temp   NEW_TEMP=boxbox-temp

docker compose down &&
  docker volume inspect "$OLD_DATA" >/dev/null &&
  docker volume create "$NEW_DATA" &&
  docker run --rm \
    -v "$OLD_DATA":/from:ro \
    -v "$NEW_DATA":/to \
    alpine sh -c 'cd /from && cp -a . /to' &&
  docker volume inspect "$OLD_TEMP" >/dev/null &&
  docker volume create "$NEW_TEMP" &&
  docker run --rm \
    -v "$OLD_TEMP":/from:ro \
    -v "$NEW_TEMP":/to \
    alpine sh -c 'cd /from && cp -a . /to' &&
  docker compose up -d
```

The commands are chained, so the first failure stops the rest and the new deployment does not start.

Old `filemanager-*` volumes are not deleted. Do not start with the new Compose volume names unless every needed copy command succeeds. If a copy fails, the old `filemanager-*` volume remains the source of truth; the new `boxbox-*` target may be partial, so remove and recreate the partial target or copy into a fresh target before starting the new Compose config.

Rollback is still changing `BOXBOX_IMAGE` to the last known-good tag, then pulling and restarting.

