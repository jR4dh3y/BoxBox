# Release Workflow

Use this workflow to keep stable releases and nightly builds in separate GHCR packages while still getting automatic nightly and branch images for testing.

## v0.4.0 Desktop Release

The next release adds the Linux x86_64 desktop app alongside the existing amd64/arm64 server packages. Check [Downloads](/download/) for published assets; the website does not offer a desktop link until a real release asset exists. See [Linux Desktop](/docs/desktop/) for dependencies and pairing.

`VERSION` is the release source of truth and also sets the Electrobun app version. Build the full distribution locally with:

```bash
bun scripts/package-release.ts
```

This builds the desktop runtime, a self-contained installer archive, both standalone server targets, `SHA256SUMS` and `release-manifest.json` under `dist/release/`. Use `--skip-build` only with an already-built stable bundle for the same VERSION. A local working-tree build is marked dirty in its manifest; official tag builds require clean source.

For a public stable release, merge both the desktop PR and its website/release follow-up into master first. Set `VERSION` to the intended version, keep `docs/releases/v0.4.0.md` current, and tag the tested master commit with `v0.4.0`. The release workflow verifies that the tag matches VERSION and belongs to master. It validates the server, website, Docker build, desktop tests and artifacts before publishing. The GitHub release stays a draft while files upload, then becomes public only after the Docker image succeeds. Published releases are not overwritten.

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
- Read the [security guide](/docs/security/#share-links) for how share tokens, rate limits, and recipient access work.

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

Before upgrading, read the [security guide](/docs/security/) and update deployment configuration:

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

If you used old Docker named volumes, copy them manually only when you are ready to switch Compose to the new names. Docker Compose may prefix volume names with the project name; use `scripts/check-0.2-migration.sh` to print the actual volume names on your host.

```bash
docker compose down

docker volume create boxbox-data
docker run --rm \
  -v filemanager-data:/from:ro \
  -v boxbox-data:/to \
  alpine sh -c 'cd /from && cp -a . /to'

docker volume create boxbox-temp
docker run --rm \
  -v filemanager-temp:/from:ro \
  -v boxbox-temp:/to \
  alpine sh -c 'cd /from && cp -a . /to'

docker compose up -d
```

Old `filemanager-*` volumes are not deleted. Do not start with the new Compose volume names unless every needed copy command succeeds. If a copy fails, the old `filemanager-*` volume remains the source of truth; the new `boxbox-*` target may be partial, so remove and recreate the partial target or copy into a fresh target before starting the new Compose config.

Rollback is still changing `BOXBOX_IMAGE` to the last known-good tag, then pulling and restarting.

## Test Branch Model

- Do normal development on a branch, not on `master`.
- Use `test/*` when you want GitHub to build a clearly unstable test image for that branch.
- The nightly workflow checks `master` once per day and publishes only when it changed since the last successful nightly.
- Public releases come from `v*` tags. Stable tags such as `v0.2.0` update `latest`; optional prerelease tags such as `v0.2.0-rc.1` do not.
- Keep `master` protected in GitHub so changes land through pull requests after CI passes.

## Nightly Channel

At 02:00 Asia/Kolkata each day, the nightly workflow checks the newest commit on `master`. If that commit has not already had a successful nightly run, it publishes these multi-platform images:

```text
ghcr.io/jr4dh3y/boxbox-nightly:latest
ghcr.io/jr4dh3y/boxbox-nightly:<full-commit-sha>
```

Nightly is a dedicated GHCR package, not a tag in the stable `ghcr.io/jr4dh3y/boxbox` package. Use `boxbox-nightly:latest` to follow the newest development build. Use the commit-specific tag when you need a reproducible deployment or want to roll back after a later nightly breaks. The Nightly workflow does not update the stable package, and the release workflow does not update the Nightly package.

```bash
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox-nightly:latest docker compose pull
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox-nightly:latest docker compose up -d
```

The workflow can also be run manually for `master` from the GitHub Actions page. Manual runs always rebuild, even when the commit has already been published. Concurrent runs are cancelled so the rolling tag converges on a single build.

## 1. Work on a Test Branch

```bash
git switch -c test/my-change
# edit, commit, and push as usual
git push -u origin test/my-change
```

Pushing a `test/*` branch runs:

- `CI Validation`: Go tests, Svelte check/build, website build, and Docker build validation.
- `Branch Test Docker Image`: publishes GHCR tags for that branch and commit without touching `latest`.

For `test/my-change`, the branch image tag is:

```text
ghcr.io/jr4dh3y/boxbox-branch:branch-test-my-change
```

Branch images are a dedicated GHCR package, like nightly, so test builds never touch the stable `boxbox` package.

The workflow also publishes a `sha-<short-sha>` tag for immutable testing.

## 2. Test the GitHub-built Image on Your Server

On the server, point Compose at the branch image:

```bash
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox-branch:branch-test-my-change docker compose pull
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox-branch:branch-test-my-change docker compose up -d
```

Or edit `.env` and keep the tag there while testing:

```env
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox-branch:branch-test-my-change
```

Then run:

```bash
docker compose pull
docker compose up -d
docker compose ps
docker compose logs -f boxbox
```

Rollback is just changing `BOXBOX_IMAGE` back to `ghcr.io/jr4dh3y/boxbox:latest` or the last known-good version tag, then pulling and restarting.

## 3. Merge Through a Pull Request

Open a pull request into `master`. The PR template asks for the release-safety checks that matter for a self-hosted app: upgrade impact, rollback, storage, permissions, Docker image behavior, docs, and verification.

Recommended GitHub branch protection for `master` after these workflows are on the default branch:

- Require a pull request before merging.
- Require the `CI Validation` checks to pass.
- Require branches to be up to date before merging.
- Block force pushes and branch deletion.
- Include administrators if you want the same rules for maintainers.

## 4. Ship a Stable Release

When the tested change is merged into `master`, tag the release:

```bash
git switch master
git pull --ff-only
git tag v0.4.0
git push origin v0.4.0
```

The release workflow runs the preflight again, publishes the version tags, and updates:

```text
ghcr.io/jr4dh3y/boxbox:latest
```

Use a new patch tag if you need a hotfix. Do not retag an already-published version.

## Optional: Versioned Prerelease

You do not need prerelease tags for normal testing because `test/*` branch images already cover that. Use a prerelease only if you want a named image for other people to test before a stable release:

```bash
git tag v0.4.0-rc.1
git push origin v0.4.0-rc.1
```

Set `VERSION` to `0.4.0-rc.1` and add matching release notes before creating that tag. It publishes `ghcr.io/jr4dh3y/boxbox:v0.4.0-rc.1` but does not update `latest`.
