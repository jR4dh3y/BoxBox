# Test Roadmap (Cloudreve-Inspired, Homelab-Optimized)

Date: 2026-03-01

Goal: take the strongest testing patterns from Cloudreve (broad package-level unit coverage) and combine them with your strongest pattern (property-based invariant tests) to build a high-confidence suite without over-engineering.

## Principles

- Keep from Cloudreve:
  - Small, fast, table-driven unit tests for utility/domain packages.
  - Broad package coverage to catch regressions early.
- Keep from your project:
  - Property-based tests for invariants and security behavior.
  - Behavior-level checks around filesystem safety and streaming correctness.
- Add what both currently lack:
  - Frontend unit tests and minimal E2E coverage for critical flows.

## Target End State (90 days)

- Backend:
  - `go test ./...` stable under 3-5 minutes in CI.
  - Security-critical modules always tested (`auth`, `middleware`, `config`, `stream`, `websocket`).
  - Property tests retained for invariants; deterministic unit tests added for edge-case regressions.
- Frontend:
  - Vitest + Testing Library for component/store/utils tests.
  - Playwright smoke E2E for login, browse, upload, live updates.
- CI:
  - PR gates run lint + tests + coverage threshold on changed areas.

## Phase 1 (Week 1): Fix High-Risk Gaps First

## 1. Backend auth/token hardening tests

Create:
- `backend/internal/service/auth_test.go`

Add tests for:
- Token type separation (`access` token accepted only for API auth).
- Refresh token rejected by auth middleware for protected routes.
- Refresh rotation/revocation behavior correctness.
- Logout revocation behavior.
- Invalid signing method/token tampering cases.

Why first:
- Current critical security risk area.

## 2. Config/env contract tests

Create:
- `backend/internal/config/config_test.go`

Add tests for:
- `FM_` prefix behavior and mapping.
- `FM_USERS_*` parsing correctness.
- `FM_ALLOWED_ORIGINS` comma splitting behavior.
- Missing required config failures.
- Ensure docs examples match loader behavior.

Why first:
- Prevent silent insecure deployment drift.

## 3. Rate limiter lifecycle tests

Create:
- `backend/internal/middleware/ratelimit_test.go`

Add tests for:
- Per-IP limiting behavior.
- `X-Forwarded-For` parsing correctness.
- Cleanup behavior (if cleanup loop is wired in app lifecycle).
- Memory growth guard expectations.

Why first:
- Security/abuse control and stability.

## 4. WebSocket protocol compatibility tests

Create:
- `backend/internal/websocket/client_test.go`
- `frontend/src/lib/stores/websocket.test.ts`

Add tests for:
- Newline-delimited batched server payload parsing.
- Unknown message resilience.
- Reconnect + re-subscribe behavior.
- Ping/pong keepalive handling.

Why first:
- Known integration drift can silently break real-time UX.

## 5. Upload ID + resume integration tests

Create:
- `frontend/src/lib/utils/upload.test.ts`
- `frontend/src/lib/stores/upload.test.ts`

Add tests for:
- Store-generated `uploadId` and utility usage are the same ID.
- Resume path used when session exists.
- Cancellation and error transitions are stable.
- Progress monotonicity and completion states.

Why first:
- Known high-priority bug with user-visible failure mode.

## Phase 2 (Weeks 2-4): Build Baseline Frontend Suite

## 1. Testing infrastructure

Create:
- `frontend/vitest.config.ts`
- `frontend/src/test/setup.ts`

Install/dev deps:
- `vitest`
- `@testing-library/svelte`
- `@testing-library/jest-dom`
- `jsdom`

Update scripts in [frontend/package.json](/home/pico/server/dapps/homelab-filemgr/frontend/package.json):
- `"test": "vitest run"`
- `"test:watch": "vitest"`
- `"test:coverage": "vitest run --coverage"`

## 2. Component tests (highest value first)

Create:
- `frontend/src/lib/components/FileList.test.ts`
- `frontend/src/lib/components/UploadDropzone.test.ts`
- `frontend/src/lib/components/JobMonitor.test.ts`
- `frontend/src/lib/components/Breadcrumb.test.ts`

Cover:
- Rendering states (empty/loading/error/data).
- User interaction events and emitted actions.
- Accessibility-critical keyboard behavior.

## 3. API/client tests

Create:
- `frontend/src/lib/api/client.test.ts`
- `frontend/src/lib/api/files.test.ts`

Cover:
- Auth header behavior.
- Refresh-on-401 flow.
- Error mapping/normalization.

## Phase 3 (Month 2): Add Minimal E2E Guardrails

Create Playwright setup:
- `frontend/playwright.config.ts`
- `frontend/e2e/auth-login.spec.ts`
- `frontend/e2e/browse-navigation.spec.ts`
- `frontend/e2e/upload-and-refresh.spec.ts`
- `frontend/e2e/jobs-live-update.spec.ts`

E2E scope (minimal, not exhaustive):
- Login success/failure.
- Browse mount points and directory navigation.
- Upload file and verify visible completion.
- Start a background job and verify update appears.

## Phase 4 (Month 3): CI and Coverage Policy

Create CI workflow:
- `.github/workflows/ci.yml`

Pipeline steps:
1. Backend lint/build/test
2. Frontend lint/check/unit tests
3. Optional Playwright smoke on PR/nightly

Coverage policy (pragmatic):
- Initial threshold: 50% changed-lines coverage.
- Raise gradually to 70%+ for security-sensitive backend packages.

## File-By-File First Sprint Checklist

Backend (do first):
- [ ] `backend/internal/service/auth_test.go`
- [ ] `backend/internal/config/config_test.go`
- [ ] `backend/internal/middleware/ratelimit_test.go`
- [ ] `backend/internal/websocket/client_test.go`

Frontend (same sprint if time permits):
- [ ] `frontend/vitest.config.ts`
- [ ] `frontend/src/test/setup.ts`
- [ ] `frontend/src/lib/utils/upload.test.ts`
- [ ] `frontend/src/lib/stores/upload.test.ts`
- [ ] `frontend/src/lib/stores/websocket.test.ts`

## Definition of Done for Sprint 1

- Critical auth/config/upload/websocket test files added and passing locally.
- `go test ./...` passes with deterministic results.
- Frontend has executable unit test runner (`vitest run`) with at least 5 meaningful tests.
- CI executes backend tests and frontend unit tests on every PR.

## Notes on Performance and Reliability

- Keep property tests for invariants, but cap runtime for CI.
- Use deterministic seeds for flaky-prone tests where possible.
- Separate “fast suite” (PR blocking) and “extended suite” (nightly).

## What Not to Do

- Do not chase raw coverage percentage without risk prioritization.
- Do not add broad brittle UI snapshot tests as primary guardrails.
- Do not import Cloudreve-scale complexity into tests before your architecture needs it.

