# Changelog

## v0.1.7 - 2026-06-15

- Add default sidebar mount points for Desktop, Downloads, Documents, Music, Pictures, and Videos in the backend config.
- Let Docker Compose map sidebar shortcuts from the host home directory by default, with per-folder `.env` overrides such as `DOWNLOADS_PATH`.
- Update Docker, configuration, and quickstart docs for the new default home-folder mounts and override variables.
- Add the `dev:test` script alias and make the local test runner expose quick-access folder mounts, using temporary stubs when host folders do not exist.
- Improve settings progress, wallpaper preview behavior, and editable path suggestions.
- Harden backend authentication and file operations, improve frontend browser/upload behavior, and fix local drive discovery paths.
- Refresh website demo/docs rendering, fix wallpaper asset permissions, remove unused UI plumbing, and extract reusable toolbar/status components.

Verification: `bun run build`
