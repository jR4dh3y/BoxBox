# Changelog

## [Unreleased]

### Added

- Added standard range-over-func iterator `WalkSeq` (`iter.Seq2[WalkEntry, error]`) to the `Walker` filesystem traversal service.

### Changed

- Upgraded Go toolchain and module directives to Go 1.27.
- Migrated Docker multi-stage build image to `golang:1.27-alpine`.
- Replaced external dependency `github.com/google/uuid` with Go 1.27 standard library `uuid`.
- Refactored background worker, upload session, and token cleanup lifecycles to use `sync.WaitGroup.Go`.
- Refactored token, upload session, and WebSocket subscription pruning to use `maps.DeleteFunc`.
- Refactored recursive file search to consume the `WalkSeq` iterator.
- Updated error inspection to use `errors.AsType`.
- Modernized async job timestamp serialization with `omitzero`.

## [0.2.2] - 2026-08-08

This release improves file browsing, upload reliability, deployment, and security.

### Added

- Added a self-contained single-binary build with the Svelte frontend embedded.
- Added loopback-only `--dev` mode for local development without authentication.
- Added scheduled nightly Docker images with rolling and commit-specific tags.
- Added same-origin WebSocket defaults and trusted-proxy configuration.

### Changed

- Refactored file browsing, streaming, previews, search, uploads, and background jobs.
- Improved drive listing, folder navigation, preview dialogs, and wallpaper settings.
- Moved refresh tokens to `HttpOnly`, `SameSite=Strict` cookies and kept access tokens in browser memory.
- Switched configured passwords to bcrypt hashes and reduced the default authentication rate limit.
- Hardened the container to run as an unprivileged UID with all Linux capabilities dropped.
- Updated Go, Alpine, Svelte, Monaco, TypeScript, Astro, and other dependencies.

### Fixed

- Fixed upload cleanup and concurrent finalization races.
- Fixed stored XSS through HTML, SVG, and XML file previews with sandboxed responses and attachment fallback.
- Fixed filename quoting in `Content-Disposition` headers.
- Added stronger mount-boundary, symlink, request-size, thumbnail, and per-user job isolation checks.
- Removed the default host-root filesystem mount.

### Upgrade notes

- Configure users with bcrypt hashes; plaintext passwords are rejected at startup.
- Set `BOXBOX_JWT_SECRET` to a non-placeholder value of at least 32 bytes.
- Existing browser sessions will need to authenticate again after upgrading.
- The container runs as UID/GID `10001` by default; ensure bind mounts are accessible or set `PUID`/`PGID`.
- The host root filesystem is no longer mounted by default. Enabling a root mount requires explicit `allow_root_mount: true`.
- An empty `allowed_origins` list now means same-origin only. Configure `trusted_proxies` when using forwarded client-IP headers.
