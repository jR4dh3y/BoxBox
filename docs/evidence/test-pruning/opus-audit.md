# Test pruning audit (Opus review)

Reviewed tree: branch `chore/remove-low-value-tests` at `4497518` plus the uncommitted local patch (seven frontend test deletions, `TestDownloadServesRequestedRange` restored). Base: `origin/master`. Rubric: T3 PR 22557, as summarised in `comparison.md`.

Per-file ledger: `opus-audit.tsv` (165 rows: 157 present test files, 7 deleted by the local patch, plus one row for the 15 perf scenario scripts). Columns: file, top-level test count, disposition, candidates, rationale and the coverage that remains.

## Scope and limits

- Inventory: every tracked Go `_test.go`, TS `.test.ts`/`.test.svelte.ts` (frontend, `frontend/scripts`, desktop, website) and `.github/scripts/test_*.py`. Untracked `experiments/` was excluded.
- Every file in that inventory was read in full, and the relevant production code was checked where a claim depended on it.
- One partial review: the 15 `frontend/scripts/perf/*.ts` browser scenario scripts were reviewed at assertion level (every `assert`/`throw` grepped), not read line by line. They contain no wall-clock gate: `filesVisibleMs` is printed as an advisory median.
- No suite was run. This is a read-only audit. Counts are top-level `Test*`/`test()` declarations; subtests and generated loop tests are not counted.

Dispositions: 124 keep, 26 trim, 3 delete whole file, 4 support files (no tests), 7 PR deletions agreed.

## PR diff check

**No over-aggressive removals.** Each removed backend test was checked against what remains:

- **Property suites (stream, auth, security, file, job, search):** their behaviour is covered by the focused replacements and the integration tests (`remote_integration_test`, `cmd/server/compress_test`, `upload_test`, `job_test`, `symlink_entry_test`).
- **Removed `job_property` notification tests:** they used a test-local `capturingHub` and never exercised the real hub, so nothing real was lost.
- **`walker_test`, `TestBPFIsNotAStorageFilesystem`, `TestLoadStillReturnsConfigOnly`:** smoke or constant checks, safe to drop.
- **The 7 frontend deletions:** agreed. `format.test.ts` is borderline: the `formatRelativeTime` past/future rounding feeds share-link expiry text. Losing it is acceptable.

**The local restore is not needed.** `TestDownloadServesRequestedRange` duplicates `cmd/server/compress_test.go` `TestFileDownloadsAreNotRecompressed/range`, which already checks 206, exact bytes and `Content-Range` through the real `initializeServer` router. Share-download range and the remote preview range cover the rest. Drop the restore, or keep it as a cheap handler-level check, but do not count it as restored coverage.

**One export was orphaned by the deletions.** `export const FILE_EXTENSIONS` in `frontend/src/lib/utils/fileTypes.ts` was only imported by the deleted `fileTypes.test.ts`, so the export can go. These are not caused by the PR but are in the same T3 cleanup class:

- **Dead functions** in `utils/format.ts` (`formatFileSizeFixed`, `parseFileSize`, `formatDuration`, `formatTransferSpeed`, `truncateFilename`, `mapSystemMountToBrowsePath`) and `utils/fileTypes.ts` (`getFileCategory`). They have no callers in production or tests on `origin/master` either.
- **Exported types used only inside their own module:** `FileCategory`, `DriveUsageLevel` and `DateFormatOptions` do not need to be exported.

**Go dependencies:** `gopter` is gone from `go.mod`/`go.sum`. No Go helper became test-only. `fake-indexeddb` stays in use (`wallpaperSync.test.ts`).

## Restore

None required.

## Add (gaps T3's restoration lesson points at; pre-existing, not caused by this PR)

1. **Real JWT expiry.** `service/auth.go:317` maps `jwt.ErrTokenExpired` to `ErrTokenExpired`, and nothing tests it. The middleware "expired" case runs against `testAuthService`, which re-implements that mapping, so it is a mock mirror. Add one case to `service/auth_test.go` that signs real claims with a past `ExpiresAt` and expects `ErrTokenExpired`. Ideally, also drive `middleware.JWTAuth` with the real service and assert the `"Token expired"` body.
2. **WebSocket hub owner filtering.** `internal/websocket/hub.go` `BroadcastJobUpdate`/`SendJobUpdateToSubscribers` skip clients whose `Username() != job.Owner`. This is a permission guard with no test anywhere. The SSE path has one (`job_events_test` owner isolation).
3. **Optional, low.** Nothing tests a confirmed recursive folder delete (`DELETE …?confirm=true` on a non-empty writable folder). Only the unconfirmed 400 and the read-only denials are covered.

## Delete whole file

| File | Why | Coverage that remains |
|---|---|---|
| `backend/internal/handler/system_test.go` | Asserts `5<<20` and `DefaultChunkSizeMB<<20`: arithmetic/constant mirror | Upload config is consumed in `stream_create_only_test` and `remote_integration_test`; `createOnly` in `cmd/server/connect_test` |
| `frontend/src/lib/settings/sections.test.ts` | Literal section-id arrays plus a find-or-General fallback (same class as the deleted `options`/`addMenu` tests) | None needed |
| `frontend/src/lib/stores/toast.test.ts` | Real 20–30 ms sleeps around a `setTimeout` lifetime: timer-only (same class as the deleted `hoverIntent`/`delayedFlag` tests) | None needed |

## Trim: high or medium confidence (about 19 tests)

- **`handler/auth_test.go` `TestAuthRejectsCrossOriginCookieRequests`.** Exact duplicate of the `cross-origin refresh` subtest of `TestAuthCookieSecurityAndOriginBoundaries`, which also checks that no cookie is set.
- **`handler/share_test.go` `TestShareUploadRejectsInvalidRequests`.** Each of its three cases is asserted elsewhere:
  - Viewer 403: `TestShareFolderViewerAndReadOnlyMountCannotUploadViaAPI`.
  - Unknown token 404: `TestShareRecipientEndpointsAreUniformlyNotFound`.
  - Empty new file 400: `share_folders_test` `TestShareUploadCanEmptyExistingFileOnly`.
- **`service/share_test.go`: five tests that duplicate handler tests.** The handler tests drive the same service over HTTP and also check the status mapping:

  | Service test | Handler test that covers it |
  |---|---|
  | `TestShareRevokeUnknownID` | `TestShareListAndRevokeViaAPI` |
  | `TestShareResolveForRecipientIsUniform` | `TestShareRecipientEndpointsAreUniformlyNotFound` (unknown, expired and revoked tokens across 4 routes) |
  | `TestShareFolderRejectsInvalidRecipientPaths` | `TestShareFolderRejectsInvalidPathsViaAPI` (identical 4 cases) |
  | `TestShareFileUploadIsAlwaysDenied` | `TestShareFileUploadIsRejectedViaAPI` |
  | `TestShareFolderListsNestedItemsAndKeepsPathsRelative` | `TestShareFolderItemsAndNestedDownloadViaAPI` |
- **`service/desktop_volumes_test.go` `TestGioMountListFiltersRemovableDrives` and `TestGioMountListIncludesInternalDataVolumes`.** They use the same fixtures and the same fields as `TestDesktopVolumesListDescribesMounted{RemovableDrive,InternalVolume}` through the public service.
- **`handler/file_test.go` `TestCreateItemCreatesFileViaAPI` and `TestSaveFileContentViaAPI`.** Happy paths repeated by:
  - `service/file_test` (create without overwrite, write existing only).
  - `cmd/server` `TestPairedClientOperationsOnReservedMountNames` (create, PATCH and rename through the production router).
- **`handler/desktop_trash_test.go` `TestDesktopTrashHandlerRoutesRestoreAndDelete`.** Only checks that the stub saw the id and returned `{"ok":true}`: a stub-call mirror.
- **`frontend/src/lib/api/desktopVolumes.test.ts` `"parses the unavailable handler body without masking its reason"`.** Same input and assertions as the first case of the previous test.
- **`frontend/src/lib/stores/websocket.test.ts` `"delays double on each failed attempt and stop at the maximum"`.** The expected array recomputes `min(first*2^n, max)` from `CONFIG`: a formula mirror. Keep the attempt-limit and self-retry regressions.
- **`frontend/src/lib/browse/paneShortcuts.test.ts` `"maps tab, split, and navigation shortcuts"`.** A literal key-to-action table. Keep the editable-target guard test.
- **`frontend/src/lib/browse/details.test.ts` `"details handle root locations and unknown timestamps"`.** Trivial branches. Keep the directory-size exclusion test.
- **`frontend/src/lib/utils/uploadEntries.test.ts` `"uses the file name when there is no folder path"`.** Trivial default. Keep the parents-first ordering test.
- **`frontend/src/lib/browse/requests.test.ts`: one assertion.** Drop only `Date.now() - started >= 990`. It checks the backoff timer duration and costs about 1 s. Keep every behavioural assertion.

## Trim: low confidence (judgement calls)

| File | Candidate | Reason |
|---|---|---|
| `cmd/server/compress_test.go` | `TestGzipIsSkippedWhenTheClientRefusesIt` | q=0 table already in `middleware/compress_test` |
| `cmd/server/compress_test.go` | `TestErrorResponsesKeepTheirStatus` | Status passthrough only |
| `handler/desktop_apps_test.go` | `TestDesktopTerminalLaunchErrorNamesRecovery` | Sentinel-to-message mirror |
| `handler/desktop_clipboard_test.go` | Final `errors.Is` | Re-checks the test's own setup |
| `handler/share_test.go` | `TestCreateFolderShareViaAPI` | Repeats the folder half of the service test |
| `service/share_test.go` | `TestShareFolderArchiveContainsRelativeZipEntries` | Handler archive test already asserts the exact entry name |
| `service/share_test.go` | `TestShareManagementIsScopedToOwner` | Handler owner-scoping test covers it; the only extra check is `List("")` |
| `service/desktop_volumes_test.go` | `TestDesktopVolumeReadOnlyFlagComesFromMountStats` | Echoes a stubbed value |
| `service/desktop_transfers_test.go` | `TestDesktopTransferHTTPBoundary` | Smoke check; also done by `remote_integration` |
| `service/mounts_test.go` | `TestIsMalformedPath` | Sentinel-membership mirror |
| `service/connect_test.go` | Alphabet-length and code-format asserts | Constant mirrors inside the race test |
| `pkg/filesystem/fs_test.go` | `TestAferoFSReadsNamesWithDotSubstrings` | `desktop_filesystem_test` covers it on disk |
| `desktop/src/launch.test.ts` | `"maps the filesystem root to the root mount"` | Trivial branch; already exercised by the parse and `targetArguments` tests |
| `desktop/src/launch.test.ts` | `"returns no target for an ordinary desktop launch"` | Trivial branch |
| `desktop/src/launch.test.ts` | `"walks past wrapper processes but stops at the launcher"` | Overlaps `"recovers packaged arguments from the waiting launcher process"` |
| `frontend/src/lib/browse/selection.test.ts` | `"a checkbox changes only its own path"` | Covered by `fileInteractions` |
| `frontend/src/lib/desktop/connectionView.test.ts` | `"an unprobed server is checking"` and the address-formatting test | Trivial branch and formatting |
| `frontend/src/lib/stores/jobs.test.ts` | `"ignores updates for unknown jobs"` | Trivial guard |
| `frontend/src/lib/utils/jobProgress.test.ts` | `"older servers without a size still show their percentage"` | One-line passthrough |
| `frontend/src/lib/settings/wallpaperFile.test.ts` | `WALLPAPER_FILE_ACCEPT.includes` assert | Constant mirror |

Not a deletion: `createTar`/`tarHeader`/`writeTarOctal` are copied verbatim in `desktop/src/launch.test.ts` and `desktop/scripts/runtimeArchive.test.ts`. Move them into a shared fixture module.

## Keep (explicitly checked against the rubric)

- **Auth and session:**
  - Backend: `service/auth*`, `handler/auth_test` (apart from the duplicate above), `middleware/auth|connect|ratelimit|logging`, `cmd/server/connect|desktop`.
  - Frontend: `api/client`, `api/connect`, `api/earlyStart`, `stores/auth`.

  These are the T3 "restored 22 auth tests" class. Their timeouts and fake servers are not a reason to delete them.
- **Path confinement and permissions:**
  - `validator`, `mounts`, `mount_policy`, `mount_read_only`, `path_safety`, `symlink_entry`.
  - The desktop chooser/clipboard/apps/trash/watch/thumbnail tests.
  - `share_*` minus the duplicates above.
  - `remoteCapabilities`, `remotePaths`, desktop `runtimeArchive`/`target`/`renderer-target`/`instance`.
- **Persistence and migrations:** `auth_sessions`, `connect`/`connect_store`, `desktop_servers`, `desktop_preferences`, `wallpaper`, `config`, `model/share`, `wallpaperSync`, `desktopPreferences`.
- **Deterministic race regressions:**
  - Backend: `job_paths` (`-race`), `job_noreplace`, `upload_test`, share revocation races.
  - Frontend: `jobs.test`, `requests.test`, `remote.test`, `clipboardPublication`.
- **Protocol parsing and translation:** gio/clipboard codecs, `api/websocket`, `directory`, `desktopVolumes`, error-mapping tables, `releases`, `ci_scope`.
- **Performance:** the Go allocation ceilings and `frontend/scripts/perf/gates.test.ts` are deterministic proxies required by `AGENTS.md`. There is no wall-clock gate anywhere.
- **Suite-time note:** `frontend/src/lib/desktop/jobEvents.test.ts` spends about 9 s in real sleeps waiting on the recovery interval. That is a reason to inject the interval, not to delete the tests.
