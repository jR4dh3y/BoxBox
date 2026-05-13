# Quick Start

Use this page to get BoxBox running quickly. For the full deployment reference, see [Docker deployment](docker.md).

## Quick Install

```bash
git clone https://github.com/jR4dh3y/BoxBox.git
cd BoxBox
cp .env.example .env

# Edit .env and change FM_JWT_SECRET and FM_USERS_admin.
docker build -t boxbox:local .
docker compose up -d
```

The checked-in `docker-compose.yml` is Traefik-oriented and expects an external Docker network named `proxy`. For a simple local test without Traefik, use the `docker run` example in [Docker deployment](docker.md).

## Required Edits

Before starting a reachable deployment, change at least:

```bash
FM_JWT_SECRET="$(openssl rand -base64 32)"
FM_USERS_admin="a-long-unique-password"
```

## Open BoxBox

With the default compose file, access BoxBox through your Traefik host:

```text
http://boxbox.localhost
```

For a direct local container mapped with `-p 8080:80`, open:

```text
http://localhost:8080
```
