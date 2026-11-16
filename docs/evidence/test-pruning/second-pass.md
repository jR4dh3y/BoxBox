# Test pruning: second pass

Base: `f3618aa` on `chore/remove-low-value-tests`. This pass re-read the largest suites with their production code, compared service tests against handler tests, and decided each low-confidence candidate from `opus-audit.md` separately instead of keeping them all.

## Result

| Measure | Before | After |
|---|---|---|
| Lines in tracked test files (`_test.go`, `.test.ts`, `.test.svelte.ts`, `test_*.py`) | 26,348 | 25,806 (−542) |
| New test-only fixture `desktop/scripts/tar-fixture.ts` | — | 62 |
| Net test code | 26,348 | 25,868 (−480) |
| Diff | | 7 insertions, 549 deletions in 14 files, plus 1 new file |
| Frontend tests | 327 | 323 |
| Desktop tests | 64 | 62 |

The cut is about 2% of the remaining test code. The evidence does not support a reduction on the scale of T3 PR 22557. See "Why not more" below.

## Deletions and consolidations

### Backend: share service against share handler

`handler/share_test.go` drives the real `ShareService` through HTTP. Eight tests repeated scenarios that the other layer already covers:

| Removed | Still covered by |
|---|---|
| service `TestShareFolderEditorCanCreateFileButViewerCannot` | service `TestShareUploadOnlyCannotReplaceOrDeleteAndEnforcesLinkLimit`: an upload-only link creates `new.txt`, and a view-only update gets `ErrPermissionDenied` |
| service `TestShareFolderArchiveContainsRelativeZipEntries` | handler `TestShareFolderArchiveViaAPI` requires the exact entry `shared/nested/note.txt`, so a mount-prefixed or absolute name fails |
| service `TestShareManagementIsScopedToOwner` | handler `TestShareManagementIsScopedToOwnerViaAPI` (list scoping, 404 on cross-owner revoke, share survives) and `TestShareManagementRequiresUsername` (no username gets 401 before the service is called) |
| handler `TestCreateFolderShareViaAPI` | service `TestShareCreateSupportsFoldersAndSimplifiesFilePermissions` asserts the same folder normalization |
| handler `TestShareStreamingRequiresPermission` | It only asserted 200 on routes that file-share normalization always allows. That normalization is asserted in the service test, and the download/preview responses in the CSP/sandbox handler tests |
| handler `TestShareFolderViewerAndReadOnlyMountCannotUploadViaAPI` | The read-only case created a share without upload permission, so it repeated the viewer case and never exercised a read-only mount. Read-only revalidation: service `TestShareRecipientRevalidatesLiveMounts`. Mapping to 403: `TestShareFileUploadIsRejectedViaAPI`, `TestShareCreateFolderRequiresFullAccess` |
| handler `TestShareFolderUploadOverwritesFileViaAPI` | service `TestShareFolderUploadIsAtomicAndBounded` (overwrite, byte count, file name, no leftover temp files, on a real disk). HTTP overwrite: `TestShareUploadCanEmptyExistingFileOnly` |
| handler `TestShareUploadOnlyRespectsPerLinkLimitAndCannotOverwriteViaAPI` | service `TestShareUploadOnlyCannotReplaceOrDeleteAndEnforcesLinkLimit` asserts the same three outcomes and more. 403 mapping: see above. 413 for the per-link limit: `TestSharePermissionOnlyUpdatePreservesUploadLimit` |

### Backend: other

- **`service/desktop_volumes_test.go`**
  - Merged `TestDesktopVolumePollsUntilTheUnmountedStateIsVisible` into the unmount test, now `TestDesktopVolumeUnmountUsesReportedURIAndWaitsForUnmountedState`. The two tests ran the same flow. The merged test keeps the gio argv assertion, the four-list polling assertion and the final unmounted state.
  - Removed `TestDesktopVolumeReadOnlyFlagComesFromMountStats`. It only checked that a stubbed `space()` value was copied into a field.
- **`service/desktop_transfers_test.go` `TestDesktopTransferHTTPBoundary`.** `cmd/server/remote_integration_test` already POSTs `/transfers` (202) and polls the job through the real router. The removed test only added an unconditional list GET.
- **`cmd/server/compress_test.go` `TestGzipIsSkippedWhenTheClientRefusesIt`.** Its q=0 header variants are a subset of the `middleware/compress_test` `TestAcceptsGzip` table. `TestDirectoryListingIsGzippedWhenAccepted` still proves the router wiring.
- **`service/mounts_test.go` `TestIsMalformedPath`.** It restated the three sentinels in `isMalformedPath`.
- **`pkg/filesystem/fs_test.go` `TestAferoFSReadsNamesWithDotSubstrings`.** `service/desktop_filesystem_test` `TestFileServiceHandlesLegalDotSequenceNamesOnDisk` reads the same names through `OsFS`.
- **`handler/desktop_clipboard_test.go`.** Removed a final `errors.Is` that checked the test's own fixture.

### Frontend

- **`stores/jobs.test.ts`**
  - Removed `"ignores updates for unknown jobs"`. The `!existing` guard shares its early return with the terminal-state guard, which is covered.
  - Removed `"the first load, with nothing known yet, fires nothing"`. The parameterized `"a delayed creation cannot regress a … snapshot"` tests already assert zero notifications after an initial load for three terminal states.
- **`utils/wallpaperSync.test.ts` `"completed publication overrides a pending marker when cleanup keeps failing"`.**
  - The test blocked `removeItem` on pending keys. In the current `wallpaperPublication.ts`, that flow calls `remove` only when a new intent replaces a completed one, which this test never triggers, so the fault injection was dead.
  - Its observable outcome is asserted at the end of `"a failed local upload survives login…"`: a confirmed remote deletion wins without a re-upload.
  - The `remove`-throws path is still exercised by `"successful wallpaper upload and deletion survive blocked pending-key cleanup"`.
- **`desktop/connectionView.test.ts` `"an unprobed server is checking"`.** A single trivial branch.
- **`settings/wallpaperFile.test.ts`.** Removed the `WALLPAPER_FILE_ACCEPT.includes` constant mirror and its import. The acceptance table that matches server sniffing stays. The constant is still used by `WallpaperPickerModal.svelte`.

### Desktop

- **Shared tar fixture.** Moved the identical `TarFixtureEntry`/`createTar`/`tarHeader`/`writeTarOctal` from `src/launch.test.ts` and `scripts/runtimeArchive.test.ts` into `scripts/tar-fixture.ts`, following the existing `thumbnail-fixtures.ts` pattern. Each suite keeps its own `runtimeEntries`, because their contents differ.
- **`"maps the filesystem root to the root mount"`.** `virtualPath` is exercised by every parse test, and `/` ↔ `root` by `targetArguments`. Removed the now-unused `virtualPath` import.
- **`"walks past wrapper processes but stops at the launcher"`.** It repeated the walk loop of `"recovers packaged arguments from the waiting launcher process"`, which already covers a parent hop, the stop at a non-launcher ancestor, and an unknown PID.

## Low-confidence candidates declined after verification

| Candidate | Why it stays |
|---|---|
| `cmd/server` `TestErrorResponsesKeepTheirStatus` | Small bodies take the buffering path of the gzip writer. A writer that drops `WriteHeader` would turn a 400 into a 200. No other test checks a non-200 status there |
| `handler/desktop_apps` `TestDesktopTerminalLaunchErrorNamesRecovery` | The only check that the launch-failed/unavailable distinction (asserted in the service) reaches the user as a distinct message |
| `service/connect` alphabet and format asserts | The length-32 check guards against modulo bias in code generation, a security invariant rather than a mirror |
| `frontend/browse/selection` `"a checkbox changes only its own path"` | The audit was wrong. `fileInteractions` tests `applyFileSelection`, a separate implementation. `withPathSelected`, used by `FileList`/`FileGrid`, has no other test. (Two implementations of the same gesture is a possible production simplification, outside this test-only pass.) |
| `frontend/utils/jobProgress` `"older servers without a size…"` | The only case for the progress>0, no-size branch |
| `frontend/desktop/connectionView` address formatting | Covers the fallback for a non-URL value |
| `desktop/launch` `"returns no target for an ordinary desktop launch"` | The default path on every normal app start |
| Share `TestShareFolderDefaultOrder` against trash `TestDesktopTrashDefaultOrder` | Two independent sort implementations (`share.go:500`, `desktop_trash_linux.go:52`) |

## Suites re-read and kept whole

- **`service/auth_sessions_test.go` (818).** Session-store persistence, a fault-injection matrix (9 stages × login/refresh/logout, pre- and post-publication), fail-closed loading, a concurrent refresh race and rollback ordering. These are the auth/persistence class that T3 restored. The 17-case store-validation table could lose two or three near-duplicate rows, but they are strict-schema boundaries, and the saving would be cosmetic.
- **`frontend/utils/wallpaperSync.test.ts` (1,029 now).** Apart from the one removed above, each test is a distinct cross-account or ordering race against a real `Bun.serve` and fake IndexedDB. Pairs that look similar were checked for different preconditions. For example, a failed restore with no cache would DELETE, while one with stale cache would PUT.
- **`handler/auth_test.go`, `service/desktop_apps_test.go`, `service/desktop_trash_test.go`, `service/desktop_servers_test.go`, `handler/mount_read_only_test.go` with `service/mount_policy_test.go`.** These cover confinement, auth and persistence. No scenario repeats another.
- **`earlyStart`, `client`, `auth` store, `requests`, `remotePaths`.** Auth handoff, refresh races and protocol qualification.
- **Files under 50 lines.** Read in full. They are already one or two assertions on a real boundary.

## Why not more

The first pass already removed the classes T3 deletes: property-test mirrors, constant tests, timer-only tests, mock-call mirrors and duplicate files (≈5.6k lines). What remains is mostly in the classes T3 explicitly keeps: auth and sessions, path confinement, persistence with fault injection, protocol parsing and deterministic races. Reaching a T3-like percentage would mean deleting those, or packing lines, which this pass was told not to do. Recommendation: do not pursue a larger line target. If suite time matters, inject the recovery interval in `desktop/jobEvents.test.ts` (≈9 s of real sleeps), as `opus-audit.md` already noted.

## Checks run

- `go vet ./...` (backend): clean.
- `go test ./internal/service/ ./internal/handler/ ./cmd/server/ ./internal/pkg/filesystem/ -count=1`: pass.
- `bun run test` (frontend): 323 pass, 0 fail. `bun run check`: 0 errors and 0 warnings. ESLint and Prettier on the four changed frontend files: clean.
- `bun run test:desktop`: 62 pass. `tsc --noEmit` in `desktop/`: clean.
- `git diff --check`: clean.

## Parent verification

After reviewing the patch, the parent ran the full backend `go test ./...`, `go test -race ./internal/service ./internal/websocket`, all 323 frontend tests, all 62 desktop tests, and `git diff --check`. All passed.

## Not run

- Website and Python CI tests (untouched).
- No mutation testing of the merged volume test beyond its existing list-count assertion.
- Remote CI.
