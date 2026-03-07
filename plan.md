# Video Preview Startup Plan (Cloudreve-like)

Date: 2026-03-01
Owner: homelab-filemgr
Status: Planning only (no code changes in this plan)

## Context

Problem statement from product behavior:
- User clicks play and sees long buffering spinner before first frame.
- This is not perceived as a raw network throughput issue.

Current implementation notes:
- Player is native HTML5 video only in `frontend/src/lib/components/preview/VideoPreview.svelte`.
- Large videos are currently more conservative (`preload` behavior and manual start flow).
- File type routing currently marks many containers as direct "video preview" candidates in `frontend/src/lib/utils/fileTypes.ts`.
- Backend stream endpoint already supports range requests in `backend/internal/handler/stream.go`.

## Goals

1. Reduce time-to-first-frame (TTFF) after clicking play.
2. Avoid endless spinner for unsupported/poorly streamable formats.
3. Keep preview UX simple while adding advanced playback where needed.
4. Preserve current auth/token model and stream endpoint compatibility.

## Non-Goals

1. Full media server feature parity with Cloudreve in first iteration.
2. Global transcoding farm from day one.
3. Replacing existing preview modal architecture.

## Proposed Solution Summary

Implement a hybrid video pipeline:

1. Native HTML5 path for browser-friendly formats.
2. HLS path via `hls.js` for `m3u8` streams.
3. FLV/TS-like path via `mpegts.js` where supported.
4. Fast fail/fallback UX for formats likely to decode poorly in browser.
5. Add startup timeout and telemetry so stalls become measurable and actionable.

This mirrors the practical Cloudreve approach (multi-engine player), but keeps integration lightweight for this codebase.

## What To Change / What To Use / Where

| Area | What to change | What to use | Where to do it |
|---|---|---|---|
| Dependencies | Add advanced media playback libs | `hls.js`, `mpegts.js` | `frontend/package.json` |
| Video engine selection | Add strategy layer to choose native/hls/flv path by extension + capability | Custom engine selector utility + `video.canPlayType` | `frontend/src/lib/utils/mediaCapabilities.ts` (new) |
| Video player component | Replace single native path with multi-engine orchestration and fallback state machine | Svelte component logic + dynamic imports | `frontend/src/lib/components/preview/VideoPreview.svelte` |
| Format policy | Split "previewable" from "natively safe" formats | New constants + mapping | `frontend/src/lib/utils/fileTypes.ts` |
| Preview integration | Pass extension/mime/state to video preview for better routing | Existing preview modal flow | `frontend/src/lib/components/FilePreview.svelte` |
| Stream diagnostics | Add optional debug logging for range/startup analysis | Structured logs and request metadata | `backend/internal/handler/stream.go` |
| Documentation | Add reverse-proxy/range requirements and troubleshooting | Markdown ops guide | `docs/video-streaming.md` (new) |
| Backend tests | Extend stream tests for startup-critical headers/range behavior | Existing Go test suite | `backend/internal/handler/stream_property_test.go` |

## Detailed Phases

## Phase 1: Measurement and guardrails (quick win)

1. Add frontend timing hooks for:
- `loadstart`, `loadedmetadata`, `canplay`, `playing`, `waiting`, `stalled`, `error`.
2. Record timing marks from play click to first `playing` event.
3. Add startup timeout (example 8-12s) to stop indefinite spinner and show fallback action.
4. Change large-file preload policy to at least metadata-first for startup responsiveness.

Deliverable:
- Measurable baseline of startup latency and immediate UX improvement for hanging play attempts.

## Phase 2: Multi-engine playback path

1. Add engine routing logic:
- Native engine: `mp4`, `webm`, `ogv` (and only formats that browser indicates playable).
- HLS engine: `m3u8` using `hls.js`.
- FLV engine: `flv` using `mpegts.js` when supported.
2. Use dynamic imports for non-native engines to avoid penalizing users who only play native formats.
3. Ensure proper cleanup on source/file switch:
- destroy hls/flv instances,
- detach media,
- reset state.
4. Keep existing controls UX, do not introduce large new UI framework.

Deliverable:
- Stable multi-engine player that avoids native decode bottlenecks for stream formats.

## Phase 3: Format policy and fallback UX

1. Update file-type routing so "can show video viewer" is not equal to "native-safe decode".
2. For risky containers (`mkv`, `avi`, `wmv`, etc.):
- show warning early,
- attempt engine if available,
- fail fast with download/open-in-system-player CTA.
3. Improve error mapping:
- unsupported codec,
- unsupported container,
- startup timeout,
- stream unavailable.

Deliverable:
- Fewer long spinner experiences and clearer user outcomes.

## Phase 4: Backend and proxy hardening

1. Verify range semantics remain intact under deployed reverse proxy.
2. Ensure media routes are not compressed/transformed by intermediaries.
3. Keep/verify headers for video startup:
- `Accept-Ranges`, `Content-Range`, `Content-Length`, `Cache-Control: no-transform`.
4. Add optional debug mode (env flag) to log:
- range request values,
- status code (`200` vs `206`),
- bytes served and first-byte latency.

Deliverable:
- Reliable end-to-end streaming behavior in production topology, not just local dev.

## Phase 5: Optional parity extension (future)

1. Add on-demand transcoding pipeline for non-browser-native sources.
2. Generate and cache HLS variants for repeated playback.
3. Add cleanup policy for generated assets.

Deliverable:
- Near parity with full media systems for difficult source formats.

## Implementation Notes (Contributing-aligned)

1. Reuse existing utilities and extend instead of duplicating logic.
2. Keep reusable media capability logic in `utils` and keep UI logic in preview components.
3. Avoid introducing parallel type systems; extend existing file-type definitions.
4. Keep each function focused (engine selection, setup, cleanup, fallback handling separated).

## Verification Plan

Functional checks:
1. `mp4` starts quickly and reaches `playing` reliably.
2. `m3u8` plays via `hls.js` where supported.
3. `flv` plays via `mpegts.js` where supported.
4. Unsupported formats fail fast with clear fallback action.
5. File switch in preview modal does not leak players or keep stale streams.

Backend checks:
1. Stream endpoint responds with range headers correctly.
2. `206 Partial Content` behavior confirmed for video seek/startup ranges.
3. No proxy-side buffering/compression interference on media routes.

Regression checks:
1. Existing image/audio/pdf/code preview paths remain unchanged.
2. Auth token query-based preview URLs still function.

## Acceptance Criteria

1. TTFF improves materially for supported streaming formats.
2. Spinner does not continue indefinitely without user-facing resolution.
3. Large-file playback startup behavior is predictable and measurable.
4. No regressions in other preview types or stream endpoint behavior.

## Rollout Strategy

1. Ship behind a frontend feature flag (`newVideoPlayer`).
2. Enable for internal/staging first, then production.
3. Compare startup metrics before and after full rollout.

## Risk Register

1. Browser compatibility differences (Safari/Firefox/Chromium) for HLS/FLV paths.
2. Extra frontend bundle size if dynamic import boundaries are misconfigured.
3. Proxy config mismatch may still cause hidden range/startup issues.
4. Some files may remain unplayable without transcoding (expected, handled by fallback UX).

## Effort Estimate

1. Phase 1: 0.5-1 day
2. Phase 2: 1-2 days
3. Phase 3: 0.5 day
4. Phase 4: 0.5-1 day
5. Phase 5 (optional): 2-4 days

Total (without transcoding): ~2.5 to 4.5 days.
