# BoxBox Project Context

> **Last updated:** 2026-05-06  
> **Purpose:** Single source of truth for the non-`docs/` Markdown files and current codebase state.  
> **Scope:** Consolidates project truth from the previous non-`docs/` Markdown set. The old audit, roadmap, refactor-plan, and nested starter README files were removed on 2026-05-06 after this file became the local source of truth.  
> **Excluded by request:** Contents of `docs/` Markdown files are not merged here, though this file notes known drift between `docs/` and the implementation where it affects project truth.

---

## 1. Executive Summary

BoxBox, also described in older files as **Homelab File Manager**, is a focused self-hosted file manager for homelab and NAS-style deployments.

The current codebase fits the **homelab-first simple manager** path, not a Cloudreve-scale multi-tenant cloud platform. The right strategic direction is to keep scope tight and prioritize security, reliability, upload correctness, streaming UX, and day-to-day file-management usability.

Current high-level status:

- Backend and frontend are substantially more mature than the older refactor plans imply.
- Several frontend refactor items are already complete: centralized config, storage helper, file-type utility consolidation, Tailwind design tokens, UI base components, drag/drop upload, search bar, and breadcrumb segment navigation.
- Some critical issues from the 2026-03-01 audit still remain: JWT access/refresh token boundary, plaintext credentials, unsafe `admin:admin` fallback, deployment env-var mismatch, and deployment mount blast-radius.
- Frontend integration drift for upload IDs, standard resumable upload flow, WebSocket newline-batch parsing, WebSocket lifecycle wiring, active-job subscription syncing, and direct preview/download token storage access was fixed on 2026-05-06.
- Backend `go test ./...` currently passes in this environment.
- Frontend dependencies were installed on 2026-05-06. `bun run --cwd frontend check` and `bun run --cwd frontend build` pass with two existing Svelte warnings in `SearchBar.svelte` and `InlineRename.svelte`; full `lint` remains blocked by repo-wide pre-existing Prettier drift.

---

## 2. Current Product Identity

### Name

Use **BoxBox** as the public project/repository name. Older Markdown files use **Homelab File Manager**. Treat that as the product description or legacy name, not necessarily the repository identity.

### Positioning

BoxBox is a modern self-hosted file manager for homelabs. It is intended to make Linux server and mounted-drive management feel closer to browsing local folders in a browser.

### Primary value proposition

- Browse multiple mounted locations from one web UI.
- Upload large files using chunked upload plumbing.
- Preview common media and document types.
- Run long file operations as background jobs.
- Keep deployment simple with a single Go server that embeds the static frontend.

### Not currently trying to be

- A Cloudreve replacement with full multi-tenant storage policies.
- A full document-collaboration platform.
- A media server or transcoding farm.
- A sharing/public-link platform.
- A database-backed identity and permissions system.

---

## 3. Repository Layout

Current top-level structure:

| Path | Purpose | Current status |
|---|---|---|
| `backend/` | Go API server, embedded frontend host, file/job/auth/stream services | Active |
| `frontend/` | SvelteKit app compiled to static assets | Active |
| `website/` | Astro marketing site | Active but README is still starter text |
| `scripts/` | Development helper scripts | Active |
| `docs/` | Longer docs set | Excluded from this merge; known to contain drift |
| `Dockerfile` | Unified multi-stage image for frontend + backend | Active |
| `docker-compose.yml` | Traefik-oriented deployment | Active but has security/env drift |
| Root Markdown files | Public README plus local context | Old audit/plan/roadmap Markdown removed after consolidation |

README cleanup note: the root `README.md` was replaced on 2026-05-06 with a short BoxBox overview. The stale `nginx/` project-structure claim was removed from the public README. There is currently no top-level `nginx/` directory; deployment is described by the unified root `Dockerfile` and `docker-compose.yml`, with Traefik labels in compose.

---

## 4. Tech Stack

### Backend

- Language/runtime: Go `1.24.0`.
- Router: `github.com/go-chi/chi/v5`.
- Auth: JWT via `github.com/golang-jwt/jwt/v5`.
- WebSockets: `github.com/gorilla/websocket`.
- Filesystem abstraction: `github.com/spf13/afero` behind `internal/pkg/filesystem`.
- Config: `github.com/spf13/viper`.
- Logging dependency: `github.com/rs/zerolog`.
- Property tests: `github.com/leanovate/gopter`.

### Frontend app

- SvelteKit 2.
- Svelte 5.
- TypeScript 5.
- Vite 7.
- Tailwind CSS 4 through `@tailwindcss/vite`.
- `@tanstack/svelte-query` for query behavior.
- `monaco-editor` for code preview.
- `lucide-svelte` for icons.
- Static adapter with SPA fallback; output is embedded into the Go binary.

### Marketing website

- Astro 6.
- Tailwind CSS 4.
- `@lucide/astro`.
- The stale starter `website/README.md` was removed on 2026-05-06.

### Tooling

- Root package scripts are Bun-oriented.
- `scripts/dev.ts` starts frontend Vite dev server and backend `air` hot reload with prefixed logs.
- Backend hot reload expects `air` to be installed separately.

---

## 5. Deployment Model

### Current intended deployment

- Root `Dockerfile` builds the frontend with Bun, builds the backend with Go, copies frontend static output into `backend/internal/static/dist`, and produces a single Alpine runtime image.
- Runtime server listens on port `80` by default.
- `docker-compose.yml` is configured for Traefik labels and an external `proxy` network.
- Health check hits `/health`.

### Important deployment drift

The deployment configuration is not fully aligned with the backend config loader:

- Backend config loader uses `FM_` env prefix.
- `docker-compose.yml` currently sets `JWT_SECRET`, not `FM_JWT_SECRET`.
- `docker-compose.yml` sets `CONFIG_PATH=/app/config.yaml`, but the server only receives a `-config` flag and default Viper search paths. This works in the image because `/app/config.yaml` exists and the working directory is `/app`, but `CONFIG_PATH` itself is not read by the backend.
- Checked-in `backend/config.yaml` still contains an insecure placeholder JWT secret.
- Compose mounts `/` into the container as `/host_root`, increasing blast radius if auth or configuration is wrong.

Priority: fix env-var alignment and safe defaults before encouraging broader deployment.

---

## 6. Architecture Rules From `CONTRIBUTING.md`

These are still the right development standards for the project.

### Core principles

1. Search before creating new code.
2. Reuse and extend existing utilities before adding new ones.
3. Keep one source of truth for constants, types, file extensions, config, and formatting.
4. Avoid monolithic handlers/components/functions.
5. Extract shared logic once it appears twice.
6. Follow existing project patterns.

### Backend pattern

Use **Handler → Service → Model/Filesystem**:

- Handlers parse HTTP requests, validate input shape, call services, and write responses.
- Services own business logic and filesystem behavior.
- Models define data shapes and error constants.
- Filesystem access should go through `filesystem.FS`, not direct `os` package calls.
- Paths should be validated against configured mount points.
- HTTP errors should use shared helpers and mappings from `internal/handler/response.go` and `internal/handler/errors.go`.
- Magic numbers belong in `internal/config/constants.go`.

### Frontend pattern

- Prefer Svelte 5 runes and `.svelte.ts` stores for new state.
- Use Tailwind classes and design tokens; do not add component-level `<style>` blocks.
- Put constants in `src/lib/config.ts`.
- Put file type logic in `src/lib/utils/fileTypes.ts`.
- Put formatting in `src/lib/utils/format.ts`.
- Put storage access in `src/lib/utils/storage.ts`; avoid direct `localStorage` elsewhere.
- Put reusable UI primitives in `src/lib/components/ui/`.
- Put shared TypeScript types in `src/lib/types/`.

---

## 7. Backend Current State

### Major backend packages

| Package | Role |
|---|---|
| `internal/config` | Viper config loading and constants |
| `internal/model` | Config, file, job, error, drive-name models |
| `internal/handler` | HTTP handlers for auth/files/jobs/search/settings/system/stream/ws |
| `internal/service` | Auth, file, search, job, disk/system/settings logic |
| `internal/middleware` | JWT auth, rate limiting, security headers, mount-point guard |
| `internal/pkg/filesystem` | Afero-backed filesystem abstraction |
| `internal/pkg/fileutil` | Shared file info and MIME detection helpers |
| `internal/pkg/validator` | Path validation and sanitization |
| `internal/websocket` | Hub, client, and message protocol |
| `internal/static` | Embedded frontend assets |

### HTTP API shape

The server exposes:

- `/health` health check.
- Static frontend fallback for the web app.
- `/api/v1/auth/*` for login/refresh/logout/me.
- `/api/v1/roots` and file browsing endpoints.
- `/api/v1/search`.
- `/api/v1/jobs/*`.
- `/api/v1/settings/*`.
- `/api/v1/system/*`.
- `/api/v1/stream/*` for download, preview, chunk upload, and upload status.
- `/api/v1/ws` WebSocket endpoint.

### Config loading

Current facts:

- Viper has `FM_` env prefix configured.
- Manual parsing of `FM_USERS_*` exists.
- Explicit comma splitting for `FM_ALLOWED_ORIGINS` was not found; this remains a real TODO.
- `rate_limit_rps` has a model default but is not set as a Viper default in `config.Load()`. If omitted from config, it can become zero and effectively fall back to the limiter’s permissive path.
- Validation currently requires JWT secret, at least one mount point, and sane numeric upload/server values.
- Validation does not yet enforce users, rate-limit bounds, allowed-origins format, or production-safe secret values.

Updated status of old TODO items:

| Old item | Current truth |
|---|---|
| Viper `FM_` prefix missing | Stale; implemented |
| `FM_USERS_*` parsing missing | Stale; implemented |
| `FM_ALLOWED_ORIGINS` comma parsing missing | Still relevant |
| Go version mismatch | Still relevant for old docs; code uses Go `1.24.0` |
| Config validation for users/rate/origins | Still relevant |

### Auth/JWT state

Current implementation remains security-sensitive:

- Credentials are stored in plaintext config as a username/password map.
- Password comparison is direct string comparison.
- If no users are configured, startup falls back to `admin:admin` with a warning.
- JWT claims include user identity and registered claims, but no token type.
- Access tokens and refresh tokens use the same claim shape; only expiration differs.
- Generic token validation accepts any valid signed token regardless of intended use.
- HTTP auth middleware and WebSocket auth use generic validation.
- A refresh token can therefore be accepted as API authentication.
- Refresh/logout revocation exists for submitted refresh tokens, and cleanup is started, but generic API validation does not enforce token type or session semantics.

This is the highest-priority backend fix area.

### Rate limiting

- Auth routes are wrapped with rate limiting.
- Rate limiting is per client IP using an in-memory map of token-bucket limiters.
- `X-Forwarded-For` and `X-Real-IP` are trusted before `RemoteAddr`.
- `RateLimiter.StartCleanup()` exists but the convenience `RateLimit()` middleware factory does not start it.
- Long-running deployments can accumulate client-IP limiters unless cleanup is wired or storage is changed.

### WebSocket backend

- `/api/v1/ws` is mounted outside the JWT middleware group and authenticates inside the WebSocket handler.
- Token can come from `?token=` or `Authorization: Bearer`.
- Origin policy supports exact origins and wildcard subdomains.
- Empty allowed-origins config allows all origins.
- Missing `Origin` is allowed even when allowed origins are configured.
- The backend write pump can batch multiple JSON messages in one WebSocket text frame separated by newlines.
- Hub supports subscriptions, but job service currently broadcasts job updates broadly rather than only to subscribers.

### Stream/download/upload backend

- Stream routes are protected under `/api/v1/stream/*`.
- Download and preview use `http.ServeContent`, support ranges, and set range-friendly headers.
- Preview responses use inline content disposition and media-friendly headers including `Cache-Control: no-transform`.
- Chunk upload expects `X-Upload-ID`, `X-Chunk-Index`, `X-Total-Chunks`, `X-Chunk-Size`, and `X-Total-Size`.
- `X-Checksum` is optional in current code, despite comments implying final-chunk checksum should be required.
- Upload sessions are in memory and use temp chunk directories under the runtime temp path.
- Upload cleanup is started from main and removes expired sessions.
- Duplicate chunk upload is idempotent.
- Final completion assembles chunks into a temp destination, optionally verifies SHA-256, atomically renames, deletes the session, and returns `receivedChunks`, `totalChunks`, `complete`, and `path`.
- Upload status is available through `/api/v1/stream/upload/status/*` with an `uploadId` query parameter.

Known drift:

- API docs in `docs/` are stale for upload response shape.
- Stream property tests still exercise non-production route prefixes in places.

### Jobs backend

- Supported job types: `copy`, `move`, `delete`.
- Supported states: `pending`, `running`, `completed`, `failed`, `cancelled`.
- Job service uses in-memory maps and a fixed worker pool.
- Completed/failed/cancelled jobs are retained temporarily and cleaned periodically.
- Copy, move, delete jobs report progress.
- Move tries rename first, then copy/delete fallback.
- Job cancellation supports pending/running jobs.
- Job updates are broadcast over WebSocket.
- Job state is memory-only and lost on restart.

### Backend tests

Current backend tests are mostly property-based and cover:

- Stream handler invariants.
- Auth middleware behavior.
- Security headers and mount guards.
- Path validator behavior.
- File service behavior.
- Job service behavior.
- Search service behavior.

Verification run on 2026-05-05:

- `go -C backend test ./...` passed.

Backend testing gaps remain:

- Auth token-type separation tests do not exist because token type separation does not exist yet.
- Config/env contract tests are still needed.
- Rate-limiter lifecycle tests are still needed.
- WebSocket batching/subscription behavior tests are still needed.
- Stream tests should align with production `/api/v1/stream/*` prefixes.

---

## 8. Frontend Current State

### App shape

- SvelteKit app is built as a static SPA with fallback.
- Root layout initializes Svelte Query and handles auth redirects.
- Main app flow includes login, browse, settings, and a manual `/test` component/demo route.
- `browse/+page.svelte` is the main file manager screen and composes the active UI.

### Important frontend directories

| Path | Role |
|---|---|
| `src/lib/api` | API client modules |
| `src/lib/components` | Feature components |
| `src/lib/components/ui` | Reusable base UI primitives |
| `src/lib/components/preview` | File preview components |
| `src/lib/components/settings` | Settings UI components |
| `src/lib/stores` | App state stores |
| `src/lib/types` | Shared TypeScript types |
| `src/lib/utils` | File types, formatting, storage, upload helpers |
| `src/routes` | SvelteKit routes |

### Completed frontend refactor work

Older frontend refactor files are stale in several areas. These are now implemented:

- `src/lib/config.ts` exists and centralizes auth/upload/query/websocket/UI/storage config.
- `src/lib/utils/storage.ts` exists and centralizes storage helpers.
- `src/lib/utils/fileTypes.ts` exists and is the central file-type source.
- `src/lib/utils/format.ts` exists for formatting.
- Design tokens are defined in `src/routes/layout.css`.
- `src/lib/components/ui/` exists with base components including `Button`, `Input`, `Select`, `Toggle`, `Card`, `Modal`, `Spinner`, `Badge`, `ProgressBar`, `ContextMenu`, `Toast`, and `InlineRename`.
- No `<style>` blocks were found under `frontend/src` during this audit.
- `FileBrowser.svelte` referenced by old docs no longer exists.

### Stores

Store migration is mixed:

- Legacy Svelte store files still exist: `auth.ts`, `files.ts`, `jobs.ts`, `settings.ts`, `websocket.ts`.
- Svelte 5 rune stores exist: `clipboard.svelte.ts`, `upload.svelte.ts`, `toast.svelte.ts`.
- New stores should use Svelte 5 runes and `.svelte.ts`.
- Existing legacy stores should be migrated deliberately because many imports depend on them.

### API layer

- API base path is `/api/v1`.
- `apiRequest` injects auth headers, serializes JSON, refreshes on `401`, retries once, and maps backend errors.
- `auth.ts`, `files.ts`, and `jobs.ts` export both named functions and object-style APIs; this is redundant but still present.
- `files.ts` and `jobs.ts` define many domain types inline instead of centralizing all shared types under `src/lib/types`.
- `drive-names.ts` exists but is not re-exported from `src/lib/api/index.ts`.

### Direct `localStorage` drift

- Central storage abstraction exists and is used by the auth/client layer.
- `src/lib/api/files.ts` now uses `tokenStorage.getAccessToken()` when building preview/download URLs.

### File browsing UI

Currently implemented:

- Sidebar and mounted drive/system drive display.
- Toolbar with back/forward/up navigation.
- Path segment navigation/breadcrumb-like quick navigation.
- Search bar.
- File list with sorting, icons, selection state, context menu, compact mode, and keyboard open using Enter/Space.
- Upload button and drag/drop upload on the browse content area.
- Settings page and drive-name customization.
- Status bar.
- Toast notifications.

Older TODO status corrections:

| Feature | Current truth |
|---|---|
| Drag/drop upload | Implemented |
| Search bar | Implemented |
| Breadcrumb quick navigation | Implemented through toolbar path segments |
| Context menu | Implemented |
| File/folder creation | Directory creation exists in APIs/UI areas; new text-file creation is not confirmed |
| Multi-select actions | Selection state exists; full bulk action surface is not complete |
| Rename inline | Inline rename exists for drive names; file rename is modal-based |
| Keyboard shortcuts | Partial keyboard support exists, but global shortcuts like Ctrl+C/V/X, Delete, F2, Backspace are still TODO |

### Upload frontend

Current upload utility supports:

- Chunked upload to `/api/v1/stream`.
- SHA-256 checksum generation.
- Upload progress callbacks.
- `getUploadStatus()`.
- `resumeUpload()`.
- An `UploadManager` helper class.

Upload integration status:

- `uploadStore.addFiles()` generates the upload ID for UI queue tracking.
- The upload ID is passed through `UploadOptions` into chunk upload requests.
- Normal queue flow now calls `resumeUpload()` first and falls back to a fresh upload with the same upload ID if no server session exists.
- `UploadManager` also passes its generated upload ID into `uploadFile()`.

### WebSocket frontend

Current WebSocket store supports:

- Tokenized connection to `/api/v1/ws`.
- Connection state tracking.
- Ping loop.
- Exponential reconnect.
- Re-subscribe on open.
- Job update handling.

WebSocket integration status:

- Incoming text frames are split by newline and each JSON message is parsed independently.
- The authenticated root layout connects the WebSocket on login/auth initialization and disconnects on logout/unmount.
- Active jobs are synced to WebSocket job subscriptions.
- Created copy/move/delete jobs are upserted into the jobs store so live updates can attach immediately.

### Preview system

Current preview components:

- `ImagePreview.svelte`: zoom, rotate, reset, wheel zoom.
- `AudioPreview.svelte`: native audio controls.
- `VideoPreview.svelte`: native HTML5 video only.
- `PdfPreview.svelte`: iframe preview.
- `CodePreview.svelte`: Monaco editor with plain-text fallback.

Current video state:

- No `hls.js` dependency.
- No `mpegts.js` dependency.
- No multi-engine video strategy utility.
- Large video preview is conservative and requires manual start.
- Native browser video support is the only playback path.
- The previous standalone video startup plan was consolidated here and removed with the other old root Markdown files.

### Frontend tests/tooling

- `frontend/package.json` has `dev`, `build`, `preview`, `prepare`, `check`, `check:watch`, `format`, and `lint` scripts.
- There are no frontend test scripts yet.
- No Vitest config was found.
- No Playwright config was found.
- No frontend automated test files were found.
- A manual `/test` route exists as a component/demo page.

Verification status:

- Frontend dependencies were installed on 2026-05-06.
- `bun run --cwd frontend check` passes with two existing Svelte warnings in `SearchBar.svelte` and `InlineRename.svelte`.
- `bun run --cwd frontend build` passes with the same Svelte warnings and existing large chunk warnings.
- Full `bun run --cwd frontend lint` is blocked by pre-existing repo-wide Prettier drift outside the files changed on 2026-05-06.

---

## 9. Website Current State

The `website/` app is an Astro marketing site for BoxBox.

Current facts:

- `website/src/pages/index.astro` is custom and markets BoxBox.
- It links to `https://github.com/jR4dh3y/BoxBox`.
- It uses custom components such as `FeaturesBento` and `AppInterfaceDemo`.
- `website/package.json` uses Astro 6, Tailwind 4, and Lucide Astro icons.
- `website/README.md` was removed on 2026-05-06 because it was still the default Astro starter README.

---

## 10. Cloudreve Comparison Takeaways

The Cloudreve comparison remains useful strategically, but its evidence paths point to an older local path and should not be treated as current path references.

### What Cloudreve does better

- Mature multi-tenant identity model.
- Stronger token/session model.
- Password hashing and account state checks.
- Rich admin/user/group/policy/sharing APIs.
- WebDAV/WOPI/share/public-link feature depth.
- Durable DB/Redis-backed state.
- Release and operations maturity.
- Large frontend feature surface.

### What BoxBox does better for its target niche

- Much simpler deployment and architecture.
- Single service with embedded frontend.
- Smaller dependency and operational footprint.
- Easier to understand and modify.
- Strong property-based testing instincts in backend core areas.
- Clearer fit for a single-admin homelab/NAS file manager.

### Strategic decision

Choose **Path A: homelab-first simple manager**.

Avoid trying to clone Cloudreve’s breadth before fixing BoxBox’s current security, upload, WebSocket, and testing gaps.

---

## 11. Current High-Priority Risk Register

### Critical security

1. **JWT token type boundary is missing**
   - Add token type or equivalent claim.
   - Access tokens must be accepted only for API auth.
   - Refresh tokens must be accepted only by refresh endpoint.

2. **Plaintext credentials**
   - Replace direct plaintext compare with bcrypt or argon2id.
   - Define migration behavior for existing config users.

3. **Unsafe default admin fallback**
   - Remove `admin:admin` fallback.
   - Fail startup if no users are configured, or require an explicit development-mode flag.

4. **Deployment env-var mismatch**
   - Use `FM_JWT_SECRET` in compose/examples, or explicitly support legacy `JWT_SECRET` with clear precedence.
   - Remove insecure runtime defaults from checked-in production-like config.

5. **Broad host-root mount in compose**
   - Avoid mounting `/` by default.
   - Provide safer example mounts.

### High integration risks

1. **Rate limiter cleanup not started**
   - Start cleanup or redesign map entries to expire.

### Recently resolved integration risks

1. Upload store and upload utility now use one upload ID.
2. Standard upload queue now checks status through `resumeUpload()` and falls back to a fresh same-ID upload when needed.
3. Frontend WebSocket parser now handles newline-delimited batched messages.
4. WebSocket connect/disconnect is wired into the authenticated app lifecycle.
5. Active job subscriptions are synced, and newly created background jobs are upserted into the jobs store.
6. Preview/download URL helpers now use `tokenStorage` instead of direct `localStorage`.

### Medium risks

1. Memory-only jobs, upload sessions, and revoked tokens are acceptable for simple homelab use but limit restart recovery and horizontal scale.
2. API docs under `docs/` are stale in places, especially stream upload responses.
3. Stream tests use route prefixes that do not fully match production routing.
4. WebSocket auth errors use `http.Error` plain text rather than shared JSON response helpers.
5. Upload checksum is optional even though comments imply final checksum verification should be required.

---

## 12. Consolidated Roadmap

### Already completed or mostly complete

- Context menu with common file actions.
- Backend refactor foundations: package docs, response helpers, error helpers, constants, filesystem abstraction, path validation, file utilities.
- Configurable credentials support in config/env.
- Auth route rate limiting exists.
- Configurable WebSocket origin list exists at the model/handler level.
- Upload session cleanup exists and is started.
- Auth revoked-token cleanup exists and is started.
- Job history cleanup exists.
- Frontend centralized config.
- Frontend centralized storage helper.
- Frontend centralized file-type utility.
- Tailwind design tokens.
- Reusable UI component library.
- Drag/drop upload.
- Search bar.
- Breadcrumb/path segment navigation.
- Native preview support for image/audio/video/pdf/code.
- Unified frontend upload IDs between queue and uploader.
- Standard frontend upload queue resume path.
- Frontend WebSocket newline-delimited message parsing.
- Authenticated WebSocket lifecycle wiring and active-job subscription syncing.
- Preview/download URL token lookup through centralized storage helper.

### Next 24 hours / next security patch

1. Split JWT access and refresh token types.
2. Enforce access-token-only in HTTP/WebSocket auth middleware.
3. Enforce refresh-token-only in refresh endpoint.
4. Remove `admin:admin` fallback.
5. Align compose/env examples with `FM_` config contract.
6. Remove or neutralize insecure JWT secret in checked-in runtime config.
7. Add tests for the above before or alongside implementation.

### Next 7 days

1. Start rate-limiter cleanup or redesign limiter lifecycle.
2. Add config/env contract tests.
3. Update stream route property tests to match production prefixes.
4. Add frontend tests for upload and WebSocket stores.
5. Fix repo-wide Prettier drift so full frontend `lint` can run cleanly.

### Next 30 days

1. Add password hashing and migration strategy.
2. Add frontend test runner with Vitest and Testing Library.
3. Add first frontend tests for upload and WebSocket stores.
4. Add Playwright smoke tests for login, browse, upload, and jobs/live updates.
5. Decide whether memory-only jobs/upload sessions are acceptable or introduce lightweight persistence.
6. Bring `docs/` API/config/security docs back in sync with implementation.
7. Replace stale starter READMEs in `frontend/` and `website/` or point them to this file.

### Later / optional

1. Advanced video playback with `hls.js` and `mpegts.js`.
2. Video startup telemetry and timeout UX.
3. Transcoding or cached HLS variants for difficult formats.
4. Favorites/bookmarks.
5. Recent files.
6. Dual-pane view.
7. Rich file info panel and EXIF metadata.
8. Theme toggle.
9. Grid/thumbnail view.
10. Clipboard history.
11. Persistent per-folder sort preferences.

---

## 13. Video Preview Plan Status

The video startup plan remains valid as a plan, not as implemented behavior.

### Current behavior

- `VideoPreview.svelte` uses native HTML5 video only.
- Large files are conservative and manually started.
- Backend stream endpoint supports range requests and media-friendly headers.
- File type routing marks many containers as video preview candidates, including formats that may not be browser-native-safe.

### Planned improvements

1. Add timing hooks for `loadstart`, `loadedmetadata`, `canplay`, `playing`, `waiting`, `stalled`, and `error`.
2. Add startup timeout so the spinner does not hang forever.
3. Split video policy into browser-native-safe vs preview-attempt formats.
4. Add engine selection utility.
5. Add dynamic `hls.js` path for `m3u8`.
6. Add dynamic `mpegts.js` path for FLV/TS-like playback where supported.
7. Add fallback CTAs for unsupported formats.
8. Add proxy/range troubleshooting docs after implementation.

### Recommendation

Do not start advanced playback work until critical auth/deployment issues and upload/WebSocket drift are fixed.

---

## 14. Test Roadmap Status

The test roadmap remains mostly aspirational for frontend and partially implemented for backend.

### Backend now

- Property tests already exist in key backend areas.
- `go -C backend test ./...` passed during this consolidation.

### Backend next tests

- `backend/internal/service/auth_test.go` for token type separation, refresh rejection in middleware, revocation, tampering, and signing method cases.
- `backend/internal/config/config_test.go` for `FM_` prefix, `FM_USERS_*`, `FM_ALLOWED_ORIGINS`, missing required config, and env/docs contract.
- `backend/internal/middleware/ratelimit_test.go` for per-IP behavior, forwarded headers, and cleanup.
- `backend/internal/websocket/client_test.go` for newline batching and protocol behavior.

### Frontend now

- No automated frontend test infrastructure found.
- Manual `/test` route exists.

### Frontend next tests

- Add Vitest config and setup.
- Add Testing Library for components/stores/utils.
- Add upload utility/store tests.
- Add WebSocket store parsing/reconnect/resubscribe tests.
- Add API client refresh/error tests.
- Add Playwright smoke tests after basic unit tests are stable.

### CI goal

A pragmatic target:

- Backend: `go test ./...` on every PR.
- Frontend: install dependencies, run `check`, `lint`, and unit tests on every PR.
- E2E: smoke suite on PR or nightly once stable.

---

## 15. Code Audit Status

### Still accurate findings

- API object exports are redundant in several modules.
- Types remain split between API modules and `src/lib/types`.
- Several stores still use Svelte 4 `writable`/`derived` patterns.
- WebSocket handler auth errors use plain `http.Error`.
- `UploadManager` remains in handler package; moving it to service layer could improve separation.
- Rate limiter cleanup exists but is not started by the middleware factory.

### Stale findings

- MIME type duplication in backend stream handler has been fixed.
- Frontend config/storage/file-type/design-system foundation is now implemented.
- Base UI components exist.
- No component `<style>` blocks were found under `frontend/src`.
- `FileBrowser.svelte` referenced by older docs no longer exists.
- More than one store is now migrated to runes: `clipboard`, `upload`, and `toast` are Svelte 5-style stores.
- Direct `localStorage` in `frontend/src/lib/api/files.ts` has been replaced with `tokenStorage`.

---

## 16. Documentation Status

This file should be treated as the updated consolidation of non-`docs/` Markdown.

### Root Markdown files

| File | Status |
|---|---|
| `README.md` | Replaced on 2026-05-06 with a short public BoxBox README that does not link to this local context file |
| `PROJECT_CONTEXT.md` | Local maintainer source of truth; currently untracked |
| `CONTRIBUTING.md` | Kept as the contributor workflow and architecture-guidance entry point |
| `TODO.md` | Removed on 2026-05-06 after consolidation here |
| `CODE_AUDIT.md` | Removed on 2026-05-06 after consolidation here |
| `CLOUDREVE_COMPARISON_REPORT_2026-03-01.md` | Removed on 2026-05-06 after consolidation here |
| `plan.md` | Removed on 2026-05-06 after consolidation here |
| `TEST_ROADMAP_2026-03-01.md` | Removed on 2026-05-06 after consolidation here |

### Nested non-doc Markdown files

| File | Status |
|---|---|
| `backend/REFACTOR_PLAN.md` | Removed on 2026-05-06 after consolidation here |
| `frontend/README.md` | Removed on 2026-05-06 because it was starter/stale content |
| `frontend/REFACTOR_PLAN.md` | Removed on 2026-05-06 after consolidation here |
| `website/README.md` | Removed on 2026-05-06 because it was starter/stale content |

### Recommended doc cleanup

1. Keep this file as the project source-of-truth.
2. Keep the root `README.md` short and public-facing; do not link it to this local-only context file.
3. After implementation fixes, update `docs/` API/config/security docs separately.
4. Audit `docs/` for remaining legacy product-name and `nginx/` deployment drift.

---

## 17. Definition of Done for the Next Stabilization Sprint

A stabilization sprint should be considered done when:

- JWTs have explicit access/refresh token boundaries.
- Refresh tokens are rejected by API/WebSocket auth.
- `admin:admin` fallback is removed or gated behind explicit development-only config.
- Compose and examples use the correct env names.
- Backend auth/config/rate-limit tests exist and pass.
- `go -C backend test ./...` passes.
- Frontend dependencies are installable and `check`/`lint` run in a clean environment.

Already completed from this sprint definition:

- Upload store and upload utility use the same upload ID.
- Standard upload flow can resume an existing session.
- Frontend WebSocket parser handles newline-delimited batched messages.
- WebSocket lifecycle is connected to authenticated app lifecycle.

---

## 18. Maintainer Notes

When changing this project:

- Prefer security fixes before feature expansion.
- Keep the product homelab-focused.
- Do not import Cloudreve-scale complexity without a clear user need.
- Do not add duplicated file-type, formatting, storage, response, or path-validation logic.
- Make backend long-running resources cancellable and cleanable.
- Keep the frontend design system and Tailwind-token approach consistent.
- Treat memory-only state as acceptable only if the user experience after restart is intentionally defined.
- Any new public behavior should get at least one targeted test.
