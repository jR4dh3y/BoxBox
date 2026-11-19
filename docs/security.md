# Security

BoxBox is built for private homelab use. It can modify real host files, so treat deployment configuration as part of the security boundary.

## Required Hardening

Before exposing BoxBox beyond your own machine:

- Set `BOXBOX_JWT_SECRET` to a long random value.
- Set `BOXBOX_USERS_admin` to a bcrypt hash or configure a bcrypt hash in `config.yaml`.
- Use a reverse proxy with HTTPS.
- Mount only the directories BoxBox needs.
- Use `read_only: true` for backups and sensitive locations.
- Do not expose `/host_root`; whole-host browsing is intentionally absent from the default deployment.
- Keep the default same-origin WebSocket policy or use a narrow `allowed_origins` list.
- Put the service behind your normal VPN, Tailscale, WireGuard, or trusted reverse proxy access controls when possible.

## Authentication Model

- Access tokens expire after 15 minutes. Refresh tokens expire after 7 days and are rotated in an HttpOnly, SameSite=Strict cookie.
- Both token types carry a stable session ID. Every authenticated HTTP request checks that the session exists in the active registry and belongs to a configured user. WebSocket authentication is checked at handshake. Refresh also checks that the token is the session's current refresh token.
- Logout needs only the current refresh cookie, not an access-token header. It removes the whole session, so all access tokens from that login stop working, including tokens issued before refresh. Other logins for the same user remain active.
- The version 1 `auth-sessions.json` registry under `data_dir` stores session IDs, usernames, SHA-256 hashes of current refresh tokens, and deadlines. It never stores raw JWTs. Authentication is stateful: login, refresh, and logout each write the registry.
- On Linux, the service restricts the registry directory to `0700` and the file to `0600`. A missing registry file or a valid version 1 snapshot with an empty `sessions` map at startup creates an empty active registry, so old access and refresh tokens fail after restart. A zero-byte file is invalid. Corrupt, unreadable, non-regular, unsupported, or future-version stores stop startup instead of resetting.
- A write failure before rename returns `500`. The service restores only the session changed by that request. Login, refresh, and logout do not publish a token pair or `Set-Cookie`; refresh and logout preserve the existing cookie for retry. Login does not clear an unrelated existing cookie.
- Rename publishes the new registry. Success also requires the temporary file to be synced and closed and the parent directory to be synced. If a durability step fails after rename, the service keeps the installed in-memory state and returns `401 SESSION_RECOVERY_REQUIRED` without a token pair. Refresh and logout clear the cookie. Login does not clear an unrelated cookie.
- Keep `data_dir` on persistent storage and run one server process per directory. The service does not coordinate processes that share a directory. Deleting the registry while the server runs does not clear its in-memory sessions, and a later successful write can persist them again. This is not immediate disk-loss revalidation.
- Browser refresh coalescing applies within one tab. A replaced refresh token for an active session returns `409 REFRESH_SUPERSEDED` without changing the shared cookie or issuing tokens. Refresh and logout retry once with the browser's current cookie. Repeated conflicts remain retryable and do not end the session. Expired tokens and removed sessions still return `401`.
- `auth-revocations.json` is ignored and is not deleted. Its denylist cannot identify sessions to load. Legacy tokens without a session ID are rejected on first use and never auto-enrolled; users sign in again. A missing registry file or valid empty registry after restart rejects old tokens without requiring JWT-secret rotation solely because the registry was lost.
- Restoring an older valid `auth-sessions.json` backup can revive sessions, access tokens, and their current refresh tokens. Rotate `BOXBOX_JWT_SECRET` after restoring a backup. An older validator that ignores session IDs and the registry may accept tokens after downgrade; rotate the secret if you downgrade.
- After startup with a user removed from configuration, that user's tokens fail validation. Session deadlines preserve the existing sliding refresh lifetime and retain the longest existing deadline when configured token TTLs shrink.
- Auth endpoints are rate-limited per client IP.
- Repeated failures trigger exponential per-account lockouts.

Generate bcrypt hashes with `htpasswd` (from `apache2-utils` on Debian/Ubuntu):

```bash
htpasswd -bnBC 12 admin 'your-password' | cut -d: -f2
```

Store only the resulting hash:

```yaml
users:
  admin: "$2b$12$..."
```

```bash
BOXBOX_USERS_admin='$2b$12$...'
```

Plaintext configured passwords and JWT secrets shorter than 32 bytes are rejected at startup. If no users are configured, the server also refuses to start.

## Mount Point Safety

Mount points define what the authenticated UI and API can access.

Prefer narrow paths:

```yaml
mount_points:
  - name: "media"
    path: "/srv/media"
    read_only: false
  - name: "backups"
    path: "/srv/backups"
    read_only: true
```

The default compose file does not mount `/`. A mount that resolves to `/` is rejected unless `allow_root_mount: true` is explicitly configured after reviewing the risk.

## Path Validation

The backend validates requested paths against configured mount points and blocks traversal outside those roots. Examples of blocked paths include:

```text
../secret
..%2Fsecret
media/../../etc/passwd
```

Read-only mount points also block write operations after path resolution.

## WebSocket Origins

With no `allowed_origins`, BoxBox accepts browser WebSocket upgrades only when `Origin` matches the request host. A literal `*` is supported as an explicit, risky opt-out.

Restrict origins for browser-exposed deployments:

```yaml
allowed_origins:
  - "https://boxbox.example.com"
  - "*.internal.example.com"
```

Or through env:

```bash
BOXBOX_ALLOWED_ORIGINS="https://boxbox.example.com,*.internal.example.com"
```

## Reverse Proxy Checklist

Use your proxy to provide:

- HTTPS certificates.
- HTTP to HTTPS redirect.
- WebSocket forwarding for `/api/v1/ws`.
- Request size limits compatible with your upload size.
- Optional extra auth, IP allow-listing, or VPN-only access.

If you use Traefik or another reverse proxy, add routing and TLS configuration according to that proxy's setup. The default compose file uses normal host port binding.

Forwarded client-IP headers are ignored unless the direct proxy IP/CIDR is configured in `trusted_proxies` or `BOXBOX_TRUSTED_PROXIES`.

## Upload Safety

Chunked uploads are assembled in `/tmp/boxbox` by default and moved into the final destination when complete. Configure enough disk space for temporary chunks:

```yaml
volumes:
  - boxbox-temp:/tmp/boxbox
```

Use `max_upload_mb` to cap accepted upload sizes.

## Share Links

Share links expose one file or a folder tree to anyone holding the token URL:

- Tokens are 256-bit random values (43 URL-safe characters); possession of the token is the only credential for recipient endpoints.
- Recipient endpoints are public and rate-limited per client IP, separately from the stricter auth budget.
- Unknown, expired, and revoked tokens all return the same `404`, so recipients cannot probe why a link stopped working.
- Share management is scoped to the creating account: users can list, update, and revoke only their own links.
- Owners can revoke a link at any time; expiry is optional (`expiresInSeconds` at creation). A revocation that completes during an upload prevents that upload from being published.
- A file link can delete its file when it expires (`deleteOnExpiry`, off by default; files on writable mounts only). The background job makes one attempt per link. It keeps the file unless the path still names the same file (size, modification time, and device and inode on Unix), the mount is writable, and no other link reaches the file, either at the expiry time or when the job runs. Other links are matched by path and, on Unix, by file and folder identity, so hard links and bind mounts are covered, including a folder mounted inside another shared folder. The removal runs through an opened mount root, so it cannot leave the mount, and it moves the file aside before its final check, so it removes only that file and never a save that replaced it. Revoking a link never deletes its file. Each deletion and each kept file is logged.
- Shared targets are re-resolved against the current mount configuration on every recipient access. Removed or renamed mounts invalidate the link, and a mount that became read-only still serves reads while denying uploads or deletes.
- Recipient downloads and previews carry the same sandboxed `Content-Security-Policy` as the app's streaming endpoints, and active document formats (HTML, SVG, XML) are forced to download instead of rendering inline.
- Folder uploads are capped by both the per-link `maxUploadBytes` and server `max_upload_mb` limits. Upload-only links cannot replace existing items; delete permission also permits replacement and deletion below (never at) the shared root.
- Folder ZIP archives validate traversal and resolved paths against the share root before streaming.
- Full-access links also allow folder creation and empty file edits. Folder creation uses an opened filesystem root to confine mutations if another writer swaps a parent directory for a symlink.
- Anyone holding an active share link can view the owner's stored wallpaper. The browser keeps the wallpaper owner marker and cancels pending synchronization when the authenticated session ends.
- Recipient metadata responses never include mount names or internal paths.
- The share store directory is restricted to owner access (`0700`) and its token-bearing `shares.json` file to `0600`, including existing stores when the service initializes.
- Before downgrading to a version predating split upload/delete permissions, back up `shares.json`; older versions cannot preserve the new per-link permissions and upload caps if they rewrite the store. An older version that rewrites the store also drops `deleteOnExpiry`, so those files are kept.

## Operational Checklist

- Rotate `BOXBOX_JWT_SECRET` if it was ever committed or shared.
- Legacy tokens without a session ID are rejected on first use. Users sign in again; no automatic enrollment or JWT-secret rotation is needed solely for that one-time sign-out.
- Rotate admin passwords after test deployments.
- Review mounted paths after adding new host disks.
- Check logs for repeated failed logins or path validation errors.
- Keep the image rebuilt from current dependencies.
- Back up `/data` if custom drive names matter to you.
