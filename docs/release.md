# Release Workflow

Use this workflow to keep `latest` stable while still getting GitHub-built images for server testing.

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
- Public releases come from `v*` tags. Stable tags such as `v0.2.0` update `latest`; optional prerelease tags such as `v0.2.0-rc.1` do not.
- Keep `master` protected in GitHub so changes land through pull requests after CI passes.

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
ghcr.io/jr4dh3y/boxbox:branch-test-my-change
```

The workflow also publishes a `sha-<short-sha>` tag for immutable testing.

## 2. Test the GitHub-built Image on Your Server

On the server, point Compose at the branch image:

```bash
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:branch-test-my-change docker compose pull
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:branch-test-my-change docker compose up -d
```

Or edit `.env` and keep the tag there while testing:

```env
BOXBOX_IMAGE=ghcr.io/jr4dh3y/boxbox:branch-test-my-change
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
git pull
git tag v0.2.0
git push origin v0.2.0
```

The release workflow runs the preflight again, publishes the version tags, and updates:

```text
ghcr.io/jr4dh3y/boxbox:latest
```

Use a new patch tag if you need a hotfix. Do not retag an already-published version.

## Optional: Versioned Prerelease

You do not need prerelease tags for normal testing because `test/*` branch images already cover that. Use a prerelease only if you want a named image for other people to test before a stable release:

```bash
git tag v0.2.0-rc.1
git push origin v0.2.0-rc.1
```

That publishes `ghcr.io/jr4dh3y/boxbox:v0.2.0-rc.1` but does not update `latest`.
