# Homelab File Manager vs Cloudreve: Detailed Comparative Report

Date: 2026-03-01  
Compared commits:
- Homelab File Manager: `192f5a9` (2026-03-01)
- Cloudreve: `5b82330` (2026-02-24)
- Cloudreve frontend submodule: `8f98777`

## 1) Executive Summary

You are building a focused homelab file manager. Cloudreve is a full multi-tenant cloud storage platform.

This is the core truth from the codebases:
- Cloudreve is significantly more mature in feature depth, auth/session model, storage abstraction, persistence model, and release/operations engineering.
- Your project is better in simplicity, approachability, and lower operational complexity.
- You currently have several critical security and integration flaws that should be fixed before scaling usage.

## 2) Size and Scope Snapshot

Observed from repository scans:
- Homelab repo files: `147`
- Cloudreve repo files (backend repo only): `473`
- Cloudreve frontend submodule files: `1277`
- Homelab Go files: `49` (~10,009 lines)
- Cloudreve Go files: `458` (~159,221 lines)
- Homelab route declarations (rough grep count): `46`
- Cloudreve route declarations (rough grep count): `189`

Interpretation: your project is currently a compact product, while Cloudreve is a platform.

## 3) What Cloudreve Does Better

### 3.1 Product and Feature Breadth

Cloudreve has broad functional surface area that your app does not currently target:
- OAuth/OpenID-like flows and token exchange endpoints
- WebAuthn/passkey flows and 2FA support
- Share-link lifecycle and public short links
- WebDAV endpoint support
- WOPI document collaboration hooks
- Admin API for settings/groups/policies/users/tasks

Evidence:
- Router breadth: `/tmp/cloudreve/routers/router.go` lines ~201-420, ~780-980, ~1260-1344.

### 3.2 Auth and Session Security Model

Cloudreve’s token model is substantially stronger:
- Access vs refresh token type separation (`token_type` claim)
- Refresh validation enforces `refresh` token type
- Refresh invalidation tied to mutable user state hash (password/site changes)
- Scope model and root token revocation integration

Evidence:
- `/tmp/cloudreve/pkg/auth/jwt.go` lines ~61-82, ~125-163, ~218-221, ~253-278.

### 3.3 Identity and Account Model

Cloudreve stores and validates password digests (with compatibility handling), plus account status and 2FA flows.

Evidence:
- Password verification + digesting: `/tmp/cloudreve/inventory/user.go` lines ~543-587 and ~601-616.
- Login policy checks + 2FA flow: `/tmp/cloudreve/service/user/login.go` lines ~123-150 and ~219-244.

### 3.4 Storage and File-System Abstraction Depth

Cloudreve implements rich filesystem interfaces and upload/session management for multiple backends and cluster/node modes.

Evidence:
- Filesystem abstractions: `/tmp/cloudreve/pkg/filemanager/fs/fs.go` lines ~50-140.
- Upload manager pipeline: `/tmp/cloudreve/pkg/filemanager/manager/upload.go` lines ~25-147, ~188-220.
- Upload service orchestration: `/tmp/cloudreve/service/explorer/upload.go` lines ~21-94, ~156-209.

### 3.5 Persistence, Migration, and Operations Maturity

Cloudreve is designed for durable state and upgrades:
- Postgres + Redis reference deployment
- Structured migration tooling and resumable migration state
- Multi-arch release and image publishing via GoReleaser

Evidence:
- Compose dependencies: `/tmp/cloudreve/docker-compose.yml` lines ~1-42.
- Migration flow/state: `/tmp/cloudreve/application/migrator/migrator.go` (step/state orchestration).
- Release pipeline: `/tmp/cloudreve/.goreleaser.yaml` lines ~1-118.

### 3.6 Frontend Product Maturity

Cloudreve’s frontend has large subsystem coverage (admin panels, uploader core, many viewers, i18n, PWA-related tooling).

Evidence:
- Dependency and tooling breadth: `/tmp/cloudreve/assets/package.json` lines ~16-113.
- Directory structure includes admin/viewers/uploader/router/redux modules.

## 4) What You Are Doing Better

### 4.1 Simplicity and Operability

Your architecture is easier to understand and operate:
- Single service with embedded frontend build
- Fewer moving parts (no DB/Redis dependency for basic usage)
- Lower cognitive overhead for homelab users

Evidence:
- Unified Docker build and embedded frontend: `/home/pico/server/dapps/homelab-filemgr/Dockerfile` lines ~2-86.

### 4.2 Lean Dependency Footprint

Your backend dependencies are intentionally minimal and readable.

Evidence:
- `/home/pico/server/dapps/homelab-filemgr/backend/go.mod` lines ~5-14.

### 4.3 Modern Frontend Stack with Small Surface

You are using current Svelte 5 + Tailwind 4 + SvelteKit 2 with a much smaller frontend dependency graph than Cloudreve.

Evidence:
- `/home/pico/server/dapps/homelab-filemgr/frontend/package.json` lines ~16-44.

### 4.4 Good Engineering Signals: Property-Based Testing

You have property-based testing in core backend domains (file, jobs, middleware), which is uncommon in many projects.

Evidence:
- `/home/pico/server/dapps/homelab-filemgr/backend/internal/service/file_property_test.go` lines ~12-15, ~41-120.
- `/home/pico/server/dapps/homelab-filemgr/backend/internal/service/job_property_test.go` lines ~165-239.
- `/home/pico/server/dapps/homelab-filemgr/backend/internal/middleware/security_property_test.go` lines ~12-15, ~23-123.

### 4.5 Upload Path Is Clean and Understandable

Your chunk upload flow has clear stages and includes checksum + atomic finalization.

Evidence:
- `/home/pico/server/dapps/homelab-filemgr/backend/internal/handler/stream.go` lines ~346-447, ~554-569.

## 5) What You Are Doing Wrong (Prioritized)

## Critical

### C1. Access/Refresh Token Boundary Is Broken

Problem:
- Your access and refresh tokens are generated from the same claim shape with no token-type separation.
- Middleware validates token signature/expiry only, so a refresh token can be accepted as API auth.

Impact:
- Privilege/session boundary collapse and replay risk.

Evidence:
- Token generation has no token type distinction: `/home/pico/server/dapps/homelab-filemgr/backend/internal/service/auth.go` lines ~171-205.
- Generic validation only: same file lines ~133-156.

### C2. Credential Storage Is Plaintext + Unsafe Default Admin Fallback

Problem:
- Credentials are stored as `map[string]string` plaintext.
- If no users configured, code falls back to `admin:admin`.

Impact:
- High risk for accidental insecure deployments.

Evidence:
- Plaintext user map and direct compare: `/home/pico/server/dapps/homelab-filemgr/backend/internal/service/auth.go` lines ~59 and ~97-101.
- Default admin fallback: `/home/pico/server/dapps/homelab-filemgr/backend/cmd/server/main.go` lines ~129-134.

### C3. Deployment Env Variables Are Misaligned with Config Loader

Problem:
- Config loader uses `FM_` prefix for env overrides.
- Compose/example use `JWT_SECRET` instead of `FM_JWT_SECRET`.
- Static `backend/config.yaml` contains default insecure secret string.

Impact:
- Operators may think secret was overridden when it was not.

Evidence:
- Env prefix: `/home/pico/server/dapps/homelab-filemgr/backend/internal/config/config.go` lines ~34-36.
- Compose env: `/home/pico/server/dapps/homelab-filemgr/docker-compose.yml` lines ~28-31.
- Example env: `/home/pico/server/dapps/homelab-filemgr/.env.example` lines ~10-14.
- Insecure default secret in config: `/home/pico/server/dapps/homelab-filemgr/backend/config.yaml` lines ~7-9.

## High

### H1. WebSocket Client Integration Drift

Problem:
- Backend may batch multiple JSON messages in one WS frame (newline-delimited), but frontend parses one JSON object only.
- WebSocket store exists but appears not wired anywhere else in the app.

Impact:
- Real-time updates can silently fail under burst conditions.

Evidence:
- Backend batched write behavior: `/home/pico/server/dapps/homelab-filemgr/backend/internal/websocket/client.go` lines ~90-95.
- Frontend single parse path: `/home/pico/server/dapps/homelab-filemgr/frontend/src/lib/stores/websocket.ts` lines ~141-144.
- Store usage search found only self-references.

### H2. Resumable Upload Feature Is Not Actually Used End-to-End

Problem:
- Upload store generates `uploadId` and tracks by that ID.
- `uploadFile()` generates a new internal `uploadId`, so store and uploader IDs diverge.
- `resumeUpload()` exists but upload queue path always calls `uploadFile()`.

Impact:
- Resume/progress continuity can break.

Evidence:
- Store-generated ID: `/home/pico/server/dapps/homelab-filemgr/frontend/src/lib/stores/upload.svelte.ts` lines ~75-86.
- Utility-generated ID: `/home/pico/server/dapps/homelab-filemgr/frontend/src/lib/utils/upload.ts` lines ~226-233.
- Resume function exists but is not used in store queue path: same files lines ~311+ and ~132.

### H3. Rate Limiter Cleanup Is Implemented but Not Started

Problem:
- Rate limiter has cleanup support, but `RateLimit()` never starts it.

Impact:
- Unbounded IP map growth in long-running environments.

Evidence:
- Middleware creation: `/home/pico/server/dapps/homelab-filemgr/backend/internal/middleware/ratelimit.go` lines ~69-89.
- Cleanup API exists: same file lines ~121-146.

## Medium

### M1. Docs Drift and Contradictions

Examples:
- API docs show outdated upload response fields (`received`, `size`, `checksum`) that don’t match handler output.
- Security docs mention 1-hour access token default while code defaults to 15 minutes.
- README describes `nginx/` structure that does not exist in repository.

Evidence:
- Docs upload response: `/home/pico/server/dapps/homelab-filemgr/docs/api.md` lines ~213-230.
- Actual upload response: `/home/pico/server/dapps/homelab-filemgr/backend/internal/handler/stream.go` lines ~383-446.
- Security doc token statement: `/home/pico/server/dapps/homelab-filemgr/docs/security.md` lines ~11-13.
- README structure claim: `/home/pico/server/dapps/homelab-filemgr/README.md` lines ~96-123.

### M2. Memory-Only Runtime State for Important Flows

Problem:
- Job history, revoked tokens, and upload sessions are in memory.

Impact:
- State loss on restart and weak horizontal scale story.

Evidence:
- Revoked token map: `/home/pico/server/dapps/homelab-filemgr/backend/internal/service/auth.go` lines ~60-63.
- In-memory jobs/work queue: `/home/pico/server/dapps/homelab-filemgr/backend/internal/service/job.go` lines ~51-55.
- In-memory upload sessions map: `/home/pico/server/dapps/homelab-filemgr/backend/internal/handler/stream.go` lines ~142-156.

### M3. Security Posture vs Deployment Defaults Is Inconsistent

Problem:
- Security docs warn against broad mounts, but compose mounts host root (`/:/host_root`).

Impact:
- Increases blast radius if auth is bypassed or misconfigured.

Evidence:
- Warning guidance: `/home/pico/server/dapps/homelab-filemgr/docs/security.md` lines ~98-102.
- Compose host root mount: `/home/pico/server/dapps/homelab-filemgr/docker-compose.yml` lines ~38-40.

## 6) Practical Action Plan (What to Fix First)

### Next 24 hours (must-do)

1. Split token types in JWT claims and enforce access-only in auth middleware.  
2. Remove `admin:admin` fallback; fail startup if no users configured.  
3. Standardize env vars: migrate to `FM_JWT_SECRET` (or support both explicitly), and remove insecure default secret from checked-in runtime config.

### Next 7 days

1. Fix websocket parsing for newline-delimited payloads; wire socket lifecycle into authenticated layout.  
2. Unify upload ID source and connect queue path to `resumeUpload()`.  
3. Start limiter cleanup lifecycle or refactor to expiring per-IP entries.

### Next 30 days

1. Introduce proper password hashing strategy and migration path (e.g., bcrypt/argon2id).  
2. Introduce durable persistence for jobs/sessions (at least sqlite/postgres for metadata).  
3. Bring docs in sync with implementation and add CI checks to prevent docs/API drift.

## 7) Strategic Direction Choice (Important)

You should explicitly choose one of these product paths:

- Path A: Homelab-first simple manager
  - Keep scope tight, prioritize reliability/security/usability.
  - Avoid Cloudreve-level feature explosion.

- Path B: Cloud platform competitor
  - Accept substantial architecture growth: DB domain model, scopes/roles, multi-storage policy engine, full admin and sharing model.
  - This is a multi-phase, high-complexity roadmap.

Right now your codebase structure strongly fits Path A. Trying to mimic Cloudreve breadth immediately will likely reduce quality and shipping velocity.

## 8) Bottom Line

Cloudreve is ahead in platform maturity by a large margin.

Your project is in a good position for a focused homelab product, but the current critical auth/deployment issues must be fixed before feature expansion. If you resolve those and keep your simplicity advantage, you can be better than Cloudreve for the specific "single-admin homelab file manager" use case.
