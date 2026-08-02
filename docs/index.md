# BoxBox Documentation

BoxBox is a self-hosted file manager for homelab and NAS-style Linux servers. It serves a SvelteKit web app from a single Go binary and exposes configured filesystem locations through a browser UI.

## What to Read First

| Page | Use it for |
| --- | --- |
| [Quick start](/docs/quickstart/) | Run BoxBox quickly with Docker Compose and the published GHCR image. |
| [Docker deployment](/docs/docker/) | Deploy from GHCR with Compose, or use separate local run and source-build alternatives. |
| [Release workflow](/docs/release/) | Use test branches, test GHCR images, and stable tags. |
| [Configuration](/docs/configuration/) | Configure users, mount points, upload limits, origins, and ports. |
| [API reference](/docs/api/) | Use the REST, streaming, and WebSocket endpoints directly. |
| [Development](/docs/development/) | Run the backend and frontend app locally. |
| [Security](/docs/security/) | Harden credentials, mounted paths, reverse proxy, and network exposure. |
| [Architecture](/docs/architecture/) | Understand the Go backend, Svelte frontend, and embedded static deployment. |
| [Troubleshooting](/docs/troubleshooting/) | Diagnose common deployment, login, upload, and mount issues. |

## Current Runtime Facts

- The production image is a single container.
- The preferred deployment is Docker Compose pulling `ghcr.io/jr4dh3y/boxbox`.
- The server listens on port `80` by default.
- The frontend static build is embedded into the Go binary.
- API routes live under `/api/v1`; `/health` is also available at the root.
- Mount points are configured in `config.yaml` and can be customized by binding another config file to `/app/config.yaml`.
- Environment overrides use the `BOXBOX_` prefix, for example `BOXBOX_JWT_SECRET` and `BOXBOX_USERS_admin`.

## Required Credentials

BoxBox refuses to start without a user, a bcrypt password hash, and a JWT secret of at least 32 bytes:

```bash
BOXBOX_JWT_SECRET="$(openssl rand -base64 32)"
BOXBOX_USERS_admin='paste-a-bcrypt-hash-here'
```

Generate the hash with `htpasswd -bnBC 12 admin 'your-password' | cut -d: -f2`.

## Scope

BoxBox is intentionally a focused file manager. It is not a multi-tenant cloud storage platform, public-link sharing service, media server, or identity provider. Keep deployments private, use a reverse proxy with TLS when exposed beyond localhost, and mount only the directories you actually need.
