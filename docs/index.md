# BoxBox Documentation

BoxBox is a self-hosted file manager for homelab and NAS-style Linux servers. It serves a SvelteKit web app from a single Go binary and exposes configured filesystem locations through a browser UI.

## What to Read First

| Page | Use it for |
| --- | --- |
| [Quick start](quickstart.md) | Install BoxBox quickly and choose the right deployment path. |
| [Docker deployment](docker.md) | Build and run BoxBox as a single container. |
| [Configuration](configuration.md) | Configure users, mount points, upload limits, origins, and ports. |
| [API reference](api.md) | Use the REST, streaming, and WebSocket endpoints directly. |
| [Development](development.md) | Run the backend and frontend app locally. |
| [Security](security.md) | Harden credentials, mounted paths, reverse proxy, and network exposure. |
| [Architecture](architecture.md) | Understand the Go backend, Svelte frontend, and embedded static deployment. |
| [Troubleshooting](troubleshooting.md) | Diagnose common deployment, login, upload, and mount issues. |

## Current Runtime Facts

- The production image is a single container.
- The server listens on port `80` by default.
- The frontend static build is embedded into the Go binary.
- API routes live under `/api/v1`; `/health` is also available at the root.
- Mount points are configured in `config.yaml` and can be customized by binding another config file to `/app/config.yaml`.
- Environment overrides use the `FM_` prefix, for example `FM_JWT_SECRET` and `FM_USERS_admin`.

## Default Credentials

If no users are configured, BoxBox falls back to `admin:admin` and logs a warning. Do not run a reachable deployment without setting at least:

```bash
FM_JWT_SECRET="$(openssl rand -base64 32)"
FM_USERS_admin="a-long-unique-password"
```

## Scope

BoxBox is intentionally a focused file manager. It is not a multi-tenant cloud storage platform, public-link sharing service, media server, or identity provider. Keep deployments private, use a reverse proxy with TLS when exposed beyond localhost, and mount only the directories you actually need.
