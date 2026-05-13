# Security

BoxBox is built for private homelab use. It can modify real host files, so treat deployment configuration as part of the security boundary.

## Required Hardening

Before exposing BoxBox beyond your own machine:

- Set `FM_JWT_SECRET` to a long random value.
- Set `FM_USERS_admin` or configure users in `config.yaml`.
- Use a reverse proxy with HTTPS.
- Mount only the directories BoxBox needs.
- Use `read_only: true` for backups and sensitive locations.
- Avoid exposing `/host_root` unless you really need whole-host browsing.
- Restrict WebSocket origins with `allowed_origins` or `FM_ALLOWED_ORIGINS`.
- Put the service behind your normal VPN, Tailscale, WireGuard, or trusted reverse proxy access controls when possible.

## Authentication Model

BoxBox uses JWT access and refresh tokens:

- Access tokens expire after 15 minutes.
- Refresh tokens expire after 7 days.
- Logout revokes the submitted refresh token.
- Auth endpoints are rate-limited per client IP.

Users are currently configured as username/password pairs in YAML or environment variables:

```yaml
users:
  admin: "a-long-password"
```

```bash
FM_USERS_admin="a-long-password"
```

Current limitation: passwords are compared from configured values, not stored as password hashes. Keep config files and environment handling private.

## Default Credential Fallback

If no users are configured, the server falls back to:

```text
admin:admin
```

This is a development convenience only. The server logs a warning when it happens.

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

Avoid broad host paths unless the deployment is private and intentional:

```yaml
mount_points:
  - name: "root"
    path: "/host_root"
    read_only: false
```

The default compose file includes `/host_root` because it is useful for a personal server, but it has a large blast radius if credentials are weak or the service is exposed.

## Path Validation

The backend validates requested paths against configured mount points and blocks traversal outside those roots. Examples of blocked paths include:

```text
../secret
..%2Fsecret
media/../../etc/passwd
```

Read-only mount points also block write operations after path resolution.

## WebSocket Origins

With no `allowed_origins`, BoxBox allows WebSocket upgrades from any origin for simple homelab setups.

Restrict origins for browser-exposed deployments:

```yaml
allowed_origins:
  - "https://boxbox.example.com"
  - "*.internal.example.com"
```

Or through env:

```bash
FM_ALLOWED_ORIGINS="https://boxbox.example.com,*.internal.example.com"
```

## Reverse Proxy Checklist

Use your proxy to provide:

- HTTPS certificates.
- HTTP to HTTPS redirect.
- WebSocket forwarding for `/api/v1/ws`.
- Request size limits compatible with your upload size.
- Optional extra auth, IP allow-listing, or VPN-only access.

For Traefik, the provided compose file already adds labels for an HTTP router. Add TLS labels according to your Traefik setup.

## Upload Safety

Chunked uploads are assembled in `/tmp/filemanager` by default and moved into the final destination when complete. Configure enough disk space for temporary chunks:

```yaml
volumes:
  - filemanager-temp:/tmp/filemanager
```

Use `max_upload_mb` to cap accepted upload sizes.

## Operational Checklist

- Rotate `FM_JWT_SECRET` if it was ever committed or shared.
- Rotate admin passwords after test deployments.
- Review mounted paths after adding new host disks.
- Check logs for repeated failed logins or path validation errors.
- Keep the image rebuilt from current dependencies.
- Back up `/data` if custom drive names matter to you.
