# BoxBox Security Audit — Strict Report

Date: 2026-07-28
Scope: full backend (`backend/internal/**`), frontend auth/preview/websocket paths, Dockerfile, docker-compose, `.env.example`, CI workflows.

**Method:** full read of every backend Go source file under `internal/` (handlers, services, middleware, websocket, validator, filesystem, config), all auth/storage/preview/websocket paths of the frontend, Dockerfile/compose/.env.example, and CI workflows; `go vet`, `govulncheck -mode=source` (0 reachable CVEs), `git ls-files` secrets sweep. Every finding below was verified against source directly.

Scope note: CSRF is correctly out of scope (no cookie auth). Path traversal was aggressively probed — URL-decode loops, mount-prefix stripping, `..` injection, cross-mount — and **held up**; the validator is the strongest part of the codebase.

---

## HIGH

### H1 — Live JWTs leak into server/proxy logs on every preview, download, and WS connection

`backend/internal/middleware/auth.go:31-34` accepts `?token=` on **every** authenticated route (not just streaming), and the frontend appends it to all media requests: `frontend/src/lib/api/files.ts:272-286` (`${baseUrl}?token=...`), `frontend/src/lib/stores/websocket.ts:95-96`. `chi/middleware.Logger` (`cmd/server/main.go:72`) logs the full request URI including query string, so every log line for an `<img>`, `<video>`, `<iframe src>` preview, download, or WS handshake contains a valid 24-hour access token. Any reader of container logs, reverse-proxy access logs, or log aggregation has standing credentials. WS URLs also persist in browser/devtools history.

**Fix:** accept query tokens only on `/stream/*` and `/ws`, and strip query strings before logging (custom `RequestLogger` that rewrites `r.URL.RawQuery = ""`).

### H2 — Passwords stored and compared in plaintext, no account lockout, feasible online brute force

`backend/internal/service/auth.go:96,103` — `user.Password` from config compared via `subtle.ConstantTimeCompare` against the raw request password; `service/auth_test.go:30` hardcodes `Password: "testpassword"`. Config and `.env.example` ship plaintext passwords with no hashing support anywhere. Compounding: no lockout (explicitly acknowledged in code), and the only throttle is per-IP rate limiting that **resets every 10 minutes** (`middleware/ratelimit.go:67-78`, `dropLimitersPeriodically`) at 10 rps + 20 burst. An attacker gets roughly sustained ~10 guesses/s indefinitely against weak user-chosen passwords, and any config/env/backup leak hands over credentials directly.

**Fix:** hash passwords (bcrypt/argon2; accept pre-hashed `$2b$`-prefixed values in config), add exponential backoff/lockout after N failures per account, and remove the full-map limiter reset in favor of per-key idle TTL eviction.

### H3 — Default deployment exposes the host root filesystem read-only to all authenticated users

`docker-compose.yml` mounts `/:/host_root:ro,rslave` and `backend/config.yaml:35-38` defines mount `root → /host_root, read_only: true`. The container runs as **root** (no `USER` in `Dockerfile`) with `cap_add: DAC_READ_SEARCH`, so through `/api/v1/files/root/...` and `/api/v1/stream/download/root/...` any authenticated user can read `/etc/shadow`, SSH keys, and every bind-mounted secret on the host. The thumbnail service (`service/thumbnail.go:140-145`) even has an absolute-path deny-list for `/proc`, `/etc`, etc. — indicating the developers recognized sensitive paths — but that deny-list applies only to thumbnails, not to list/download, so `/host_root/etc/*` remains fully readable.

**Fix:** remove the `root` mount from the default config/compose, or make it opt-in with an explicit warning; refuse to start if an enabled mount resolves to `/` without an override flag.

### H4 — Container runs as root with privilege-granting capabilities

`Dockerfile` stage 3 sets no `USER`; `docker-compose.yml` grants `DAC_READ_SEARCH | CHOWN | FOWNER`. Combined with bind-mounted host home dirs (`$HOME:/home/user`), any RCE inside the app (a Go memory bug, a thumbnail decoder bug) is immediately root-over-user-files, and `chmod 1777 /tmp/boxbox` doesn't bound it. `no-new-privileges` helps but doesn't change the UID.

**Fix:** run as a non-root fixed UID with `PUID/PGID` support (the `.env.example` already anticipates this at the bottom) and drop the capabilities; where ownership is needed, chown at entrypoint or document bind-permission requirements instead of running root.

---

## MEDIUM

### M1 — WebSocket + CORS origin checks are opt-in; default empty allow-list accepts all origins

`internal/websocket/websocket.go:21-26` — `CheckOrigin` returns `true` when `AllowedOrigins` is empty, which is the shipped default (`config.yaml:20`, `.env.example:36`). Same for HTTP CORS (`cmd/server/main.go:62-64`). Impact is currently blunted by non-ambient auth (token in header/query, not cookies), so a cross-site page can't authenticate as the victim — but (a) any future cookie/basic-auth change becomes instantly exploitable (CSWSH), (b) in dev mode the origin check is the only barrier, and (c) defense-in-depth for CSWSH means checking origin *regardless* of auth transport.

**Fix:** fail closed — default to same-origin semantics (compare `Origin` host to `Host` when unconfigured), and document wildcard allowance as an explicit risk.

### M2 — CSP is drafted around Monaco to the point of near-uselessness; XFO contradicts the PDF viewer

`middleware/security.go:27-36` — `script-src 'self' 'unsafe-eval' 'unsafe-inline' blob: https://static.cloudflareinsights.com`. With `'unsafe-inline'` scripts, CSP provides essentially no XSS mitigation (which matters given H1's token-in-URL in a localStorage world). `object-src`, `base-uri`, and `frame-ancestors` are absent entirely, and the Cloudflare Insights tag is whitelisted on a self-hosted app (third-party JS in an authenticated origin holding long-lived bearer tokens — supply-chain exposure by policy). Separately, `X-Frame-Options: DENY` (line 18) blocks the app's own `<iframe>` PDF preview (`frontend/src/lib/components/preview/PdfPreview.svelte:24`) — the headers and product features disagree.

**Fix:** introduce a strict build with nonces/hashes for the SPA bootstrap (SvelteKit supports CSP nonces), keep `unsafe-eval` only if Monaco truly needs it, add `object-src 'none'; base-uri 'self'; frame-ancestors 'self'`, drop the Cloudflare whitelist from the app CSP, and replace XFO with `frame-ancestors 'self'`.

### M3 — Thumbnail deny-list is a blacklist over absolute paths and is symlink-bypassable

`service/thumbnail.go:140-145` — `isForbiddenThumbnailPath` checks the pre-symlink-resolved `absPath` against literal prefixes. A symlink anywhere under any mount pointing at `/etc` yields thumbnails of forbidden paths (including `/host_root/etc/...`). Only images up to 25 MB (`thumbnail.go:189-196`), so impact is bounded to image-readable secrets rendered to JPEG.

**Fix:** drop the absolute-path blacklist, `filepath.EvalSymlinks` the path and re-check it against the mount's resolved root (same pattern as `ValidatePathAgainstMounts`), and rely on mount boundaries, not path prefixes.

### M4 — Shared-IP rate limiting keyed on `RemoteAddr` only

`middleware/ratelimit.go:33` keys strictly on `r.RemoteAddr`; there's no `X-Forwarded-For`/proxy handling anywhere in the stack. Behind any reverse proxy all users share one bucket: 10 rps total for login — trivially exhausted by one attacker, denying logins for everyone (and then force-reset every 10 minutes). In direct exposure, one NAT == one bucket is similarly crude.

**Fix:** opt-in trusted-proxy config, then key on the derived client IP; cap buckets with idle TTL instead of mass reset.

### M5 — No request-body size limits outside the chunked-upload path

`service/upload.go:153-161` reads parts with `io.Copy` after `req.ParseMultipartForm()` with no `http.MaxBytesReader` on either the multipart chunk endpoint or any JSON endpoint — `Login`, `Refresh`, `PUT /files`, `PUT /settings/drive-names` decode unlimited bodies (`json.NewDecoder(r.Body).Decode`). Auth endpoints are rate-limited, but authenticated write endpoints aren't. `max_upload_mb` (10 GB by config) is enforced client-side only (`frontend/src/lib/utils/upload.ts`).

**Fix:** `http.MaxBytesReader` everywhere (e.g. 1 MB on JSON routes, `chunk_size_mb`+overhead on the multipart chunk route); enforce `max_upload_mb` server-side against cumulative assembled size.

### M6 — Access tokens are 24 h and non-revocable; revocation list for access tokens is dead code

`service/auth.go` — revocation "only affects refresh tokens"; `ValidateAccessToken:137-146` checks `isRevoked(claims.ID)` but `Logout` only stores the *refresh* token's hash — the access-JTI path is never written, so a stolen 24 h token is good for 24 h, full stop. Frontend stores both tokens in `localStorage` (`frontend/src/lib/utils/storage.ts` — `tokenStorage.setTokens`), readable by any JS in the origin; combined with M2's CSP and H1's log leakage, the exposure surface for a long-lived bearer token is wide.

**Fix:** shorten access TTL to 5–15 min with silent refresh (the 5-min refresh interval already exists), move refresh tokens to `HttpOnly; Secure; SameSite=Strict` cookies scoped to `/api/v1/auth`, and keep access tokens in memory only.

### M7 — Any authenticated user sees and controls all users' background jobs

`service/job.go` — `List` returns every job, `Cancel` cancels any job; no username scoping anywhere in the job model or WS hub (`websocket/hub.go` broadcasts all job events to every connected socket). Multi-user instances leak other users' file paths/activity and let any user cancel any operation.

**Fix:** scope jobs by claims.Username; filter WS broadcasts per-subscription.

---

## LOW

- **L1 — Search and directory walks have defensible but incomplete symlink guards with a TOCTOU window.** `service/search.go:26-51` EvalSymlinks the root and `SkipDir`s symlinked dirs; `service/walker.go:14-18` also skips link dirs — but a symlink swapped in *after* the check still races. File symlinks are followed without a post-open mount check everywhere (`service/file.go:30-45,83-93`, `thumbnail.go:101-138`, shared dir/file walking). Requires out-of-band ability to plant symlinks (no API creates them; extraction writes archive symlinks as plain files), but a writable group-share mount is precisely that, and a copy job then reads through such links into other mounts (`service/job.go:104-111`).
- **L2 — Upload chunk assembly is TOCTOU-symlink-writable within a world-writable temp tree.** `service/upload.go:156-167` opens the part file by name with `O_CREATE|O_TRUNC` — follows symlinks. `/tmp/boxbox` is a shared volume (`chmod 1777`, Dockerfile) — anything else with write access there can redirect chunk writes onto arbitrary host paths of the container user. Restrict to `O_NOFOLLOW`/`openat2` style reopening or a 0700 per-session subdir.
- **L3 — Login user-enumeration via timing.** `service/auth.go:103` short-circuits `!exists || compare(...)`: unknown users fail faster. Error strings are properly uniform (`ErrInvalidCredentials`); compare against a dummy hash to close the timing gap.
- **L4 — `jwt_secret` placeholder not rejected.** `config.Validate` requires only non-empty; `"change-me-in-production-use-a-long-random-string"` passes silently in both `config.yaml:8` and compose's `${BOXBOX_JWT_SECRET:?}` flow (it fails only when *unset*). Reject known placeholders and secrets < 32 bytes at startup.
- **L5 — Unbounded per-request memory on hot paths.** Directory `List` reads every entry into memory, upload buffers per-part up to `MaxMultipartMemory=32MB`, and job copies are unthrottled; with the 512 MB container memory limit that's a reachable authenticated OOM.
- **L6 — Archive extraction has no quota/symlink-target validation on extracted content and streams into the destination without rollback.** Combined with L1's file-symlink following, a hostile zip dropped onto a read-write mount and extracted over an existing symlinked path writes through it. Prefer extract-then-rename into a staging dir within the same mount.
- **L7 — `/api/version` and `/health` unauthenticated fingerprinting** — minor info disclosure (build commit/version) aiding targeted attacks.
- **L8 — Supply-chain hygiene:** base images pinned by mutable tags not digests (`oven/bun:1-alpine`, `golang:1.26-alpine`, `alpine:3.24`); runtime image pulled as `:latest` by default in compose; GitHub Actions pinned by major tag not SHA; frontend parses hostile user files with `@e965/xlsx`, `docx-preview`, `pptx-preview` in the token-holding origin (historically CVE-dense parsers); `X-XSS-Protection: 1; mode=block` is legacy and should be dropped.

---

## Dependency scan verdict

`govulncheck -mode=source ./...` on the backend: **0 vulnerabilities in reachable code**. 10 CVEs exist in required modules — 7 in `golang.org/x/image` (bmp/tiff/webp/sfnt decoders), 1 in `x/text`, 2 platform-scoped (`x/sys/windows`) — all **unreachable**: the service layer imports *only* `golang.org/x/image/draw` (verified), and unregistered image formats can't be decoded. Still bump `x/image → v0.43.0+`, `x/text → v0.39.0+` so future imports don't inherit them.

No committed secrets (only `.env.example`); Dependabot active; CI/publish workflows have minimal, well-scoped permissions with tag-gated publishing.

## What's solid (attacked and held)

- **Path traversal**: `validator.SanitizePath`'s iterative URL-decode + literal `..`/absolute rejection, and `ValidatePathAgainstMounts`' mount-prefix + relative re-check, closed every variant tried (encoded, double-encoded, `home/../x`, prefix-collision mounts).
- **JWT**: HS256 pinned via `ParseWithClaims(..., ValidMethods)` — no alg confusion; HMAC-constant-time refresh-token matching; rotation with replay acknowledgment; 7-day refresh expiry with correct claims scoping.
- **No command injection**: single `exec.Command("df", "-P", "-B1")` with fixed args.
- **Header injection**: `Content-Disposition` filename is sanitized and quoted on download.
- **Svelte frontend**: zero `{@html}` sinks; no `eval`/`new Function`; all user content flows through auto-escaped interpolation.

## Recommended priority

H2 → H1 → H3/H4 → M1 (one-line origin default) → M5 → M6. H1 and M1 are each small changes and close the biggest silent exposures today.
