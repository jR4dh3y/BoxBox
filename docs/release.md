# Release Workflow

Use this workflow to keep stable releases and nightly builds in separate GHCR packages while still getting automatic nightly and branch images for testing.

What changed in each version, and how to upgrade, is in [Release notes](/docs/release-notes/).

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
git tag v0.3.2
git push origin v0.3.2
```

The release workflow runs the preflight again, publishes the version tags, and updates:

```text
ghcr.io/jr4dh3y/boxbox:latest
```

Use a new patch tag if you need a hotfix. Do not retag an already-published version.

## Optional: Versioned Prerelease

You do not need prerelease tags for normal testing because `test/*` branch images already cover that. Use a prerelease only if you want a named image for other people to test before a stable release:

```bash
git tag v0.3.2-rc.1
git push origin v0.3.2-rc.1
```

Set `VERSION` to `0.3.2-rc.1` and add matching release notes before creating that tag. It publishes `ghcr.io/jr4dh3y/boxbox:v0.3.2-rc.1` but does not update `latest`.

## Desktop AppImages

After GitHub downloads are published, the release workflow calls [Publish desktop AppImages](../.github/workflows/appimages.yml). Native Ubuntu 24.04 runners build x86_64 and ARM64 desktop AppImages and verify CEF launch, folder handoff and shutdown before uploading both architectures and their `.sha256` files. This job does not modify the GHCR images or existing archive checksums.

For an existing release such as `v0.3.2`, manually run **Publish desktop AppImages** and provide that tag. Packaging tools come from the workflow revision; application code comes from the release tag. Uploads refuse to overwrite existing assets. If a run fails before uploading, fix the failure and rerun it. See the [desktop guide](desktop.md#appimage-builds) for runtime dependencies and launch instructions.

## AUR publishing

Stable releases also call **Publish AUR package**, which generates `boxbox-bin` metadata from the published desktop archive and its SHA-256 checksum. GitHub Actions builds the package in an Arch Linux container, runs namcap, and pushes the packaging files to `aur.archlinux.org/boxbox-bin.git`. Packaging templates live in `packaging/aur/`; the launcher and desktop entry reuse the desktop sources. Only x86_64 is supported because the desktop archive is x86_64.

The repository secret `AUR_SSH_PRIVATE_KEY` must contain a key authorized for the AUR maintainer account. The workflow uses an SSH agent and the pinned AUR host key; it does not commit credentials. Publishing occurs only through GitHub Actions. Use the workflow's manual tag input to publish an existing stable release such as `v0.3.2`. Prereleases do not update AUR. Install or update with `paru -S boxbox-bin`.
