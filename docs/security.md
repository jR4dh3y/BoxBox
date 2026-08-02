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

BoxBox uses JWT access and refresh tokens:

- Access tokens expire after 15 minutes.
- Refresh tokens expire after 7 days and are rotated in an HttpOnly, SameSite=Strict cookie.
- Access tokens stay in browser memory and logout revokes both current tokens.
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

## Operational Checklist

- Rotate `BOXBOX_JWT_SECRET` if it was ever committed or shared.
- Rotate admin passwords after test deployments.
- Review mounted paths after adding new host disks.
- Check logs for repeated failed logins or path validation errors.
- Keep the image rebuilt from current dependencies.
- Back up `/data` if custom drive names matter to you.
