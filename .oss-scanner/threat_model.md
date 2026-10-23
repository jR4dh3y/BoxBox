# BoxBox threat model

## Project and security boundaries

BoxBox is a self-hosted file manager. A Go HTTP server embeds a SvelteKit web
client. Users browse configured filesystem mounts, edit and transfer files,
upload and download content, stream media, search, and create public share links.

The scan targets the public master branch. The deployed Go server and web client
are in scope. Website marketing content and developer performance tooling have
lower priority. Desktop work on other branches is outside this enrollment.

The host administrator controls configuration, bcrypt user hashes, the JWT secret,
mount definitions, and reverse proxy trust. An administrator deliberately granting
access to a directory is expected behavior. A remote user must not escape those
mounts, bypass a read-only mount, or acquire access beyond their share's permissions.
Configured mounts are not per-user private directories unless the implementation
explicitly enforces that distinction.

## Untrusted input and priorities

- Unauthenticated HTTP clients reach login and refresh, public share routes,
  health checks, and the WebSocket handshake. Check session revocation,
  refresh rotation, cookie handling, origin checks, and rate limiting.
- A public share token is a credential for its configured target and permissions.
  Test password protection, expiry, download and upload permissions, share folder
  containment, and access after a share is revoked.
- Authenticated clients supply virtual paths, search expressions, filenames,
  upload chunks, archive requests, and copy, move, rename, and delete jobs.
  Check traversal, symlinks, time-of-check/time-of-use races, read-only enforcement,
  cross-mount operations, and cleanup after cancellation or failure.
- Stored filenames, file contents, metadata, and public share content can be
  attacker-controlled. Check preview and download content types, browser execution
  origins, HTML injection, and stored XSS against a logged-in user.
- Treat attacker-triggered unbounded memory, disk, CPU, goroutines, and WebSocket
  queues as relevant when reachable within the documented request limits.

Important code includes backend/cmd/server/main.go for routing;
backend/internal/handler and middleware for HTTP and authentication;
backend/internal/service/auth.go for sessions; service/mounts.go and
service/path_safety.go for filesystem confinement; service/share.go for shares;
and the file, stream, upload, archive, search, and job implementations.
frontend/src/lib and frontend/src/routes implement the browser client.

## Build and exercise

The image retains the checkout at /src, Bun and Go, installed frontend dependencies,
downloaded Go modules, compiled web assets, and /src/backend/server with Go debug
information. The frontend build is embedded under backend/internal/static/dist.

Run these commands inside the offline scanner environment:

```sh
cd /src/frontend && bun run test
cd /src/backend && go test ./...
cd /src/backend && go test -race ./...
```

The backend has handler, middleware, service, filesystem, and property tests.
Use temporary files, temporary mounts, and an isolated data directory in reproducers.
Do not use real user data or contact external services.

The executable takes -config /path/to/config.yaml. Start from backend/config.yaml
with loopback binding, temporary mounts, a disposable JWT secret, a test bcrypt
user hash, and a temporary data_dir. The server exposes /health and /api/v1.
The -dev flag deliberately disables authentication and binds to loopback. Findings
that rely solely on that documented opt-in are not production authentication bypasses.

## Severity and reports

Unauthenticated code execution, arbitrary host filesystem write, or authentication
bypass with broad file access may be critical when a practical reproducer proves
the impact. Reading or modifying files outside an authorized mount or share,
revocation bypass, and stored XSS that compromises an authenticated session are
generally high, subject to reachability and required privileges. Bounded denial of
service and limited information leaks are generally medium or lower according to
their demonstrated impact. Do not infer code execution from a crash alone.

Provide a minimal reproducer, the affected entry point, attacker prerequisites,
actual access gained, and a focused regression test with the proposed patch.
Distinguish production defaults from explicitly insecure development settings.
Report dependencies when BoxBox exposes a reachable vulnerable use, rather than
reporting a version number without an exploit path.
