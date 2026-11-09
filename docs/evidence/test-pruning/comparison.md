# Test pruning comparison and completion record

Reference: https://git.cafe/pingdotgg/t3code/pull/22557
BoxBox PR: https://github.com/jR4dh3y/BoxBox/pull/228
Reviewed head: 44975185750e5f433031a3699942fbb70387a566 plus the local patch.

T3 deletes tests that repeat types, trivial implementation, mock calls, timer mechanics, duplicate cases, or wall-clock performance assertions. It keeps protocol translation, authentication and permissions, persistence, orchestration, parsing, and deterministic race regressions. Its review restored security expiry tests incorrectly classified as timing checks. A timeout alone is not a reason to delete a boundary test.

## Review coverage

Claude Opus 5.5 at medium effort reviewed all 164 tracked test/support files in the initial inventory, including seven files deleted by the first local patch. The ledger has 165 rows because it also records the 15 performance scenario scripts as a group. The actual test files were read in full; the scenario scripts were reviewed assertion by assertion. A local inventory check confirmed no tracked test file was absent from the ledger. See `opus-audit.md` and `opus-audit.tsv` for the original findings; the decisions below supersede recommendations that were not applied.

This follows T3's selection criteria. It does not reproduce its swarm voting process, target its deletion percentage, or claim manual native UI acceptance.

## Applied decisions

- Delete ten whole files: seven initial frontend helper tests, settings sections, toast timer tests, and the backend upload-config arithmetic test.
- Remove duplicate backend auth/share/file/volume tests and the trash stub-call mirror. Retain the broader HTTP, production-router, real filesystem, and service boundary checks identified by the audit.
- Trim frontend duplicate parsing, backoff-formula, shortcut-table, details-default, and filename-default tests. Remove the elapsed-time assertion from request recovery while retaining its behavioral assertions. Removing that assertion does not remove the backoff wait itself.
- Remove seven unused public functions, two private helpers that only served the dead mount mapper, and their imports. Remove the now-unused `FileCategory` alias. Make `FILE_EXTENSIONS`, `DateFormatOptions`, and `DriveUsageLevel` private. Caller searches and the frontend typecheck/build confirm no remaining consumer needs them.
- Drop the extra local Download range test. `TestFileDownloadsAreNotRecompressed/range` already checks 206, Content-Range and exact bytes through the production router. This supersedes the first-pass claim that range coverage was missing.
- Add real auth-service expiry coverage for access and refresh tokens, signed from otherwise valid session claims.
- Add actual hub delivery tests for broadcast, explicit subscriptions, and broadcast fallback, for running and terminal jobs. A subscription to someone else's job must not expose its updates.
- Preserve JWT expiry and event overflow regressions already on the remote PR head.

## Findings retained or declined

- Preserve the missing shared-directory listing case as `TestShareFolderListingRejectsMissingDirectory`. The HTTP test checks a missing download, not this method. The original audit overstated their equivalence. Remove only the overlapping traversal/write cases from the former combined service test.
- Retain all low-confidence candidates. Their integration status handling, error messages, owner scoping, actual filesystem behavior, launcher traversal, selection state, older-server compatibility and display contracts can fail independently of the proposed replacements. Brevity alone is insufficient grounds for deletion.
- Do not consolidate the duplicated desktop tar fixture in this patch. Both active suites still use it, and it is unrelated to orphan cleanup.
- Do not add the optional recursive-delete test as part of test pruning. The two security gaps warranted explicit coverage; the optional feature-coverage expansion is outside this cleanup.
- Existing gopter removal remains. No new dependency removal is warranted: remaining tests use the Svelte preload and fake-indexeddb.

## Applicable packages

Backend HTTP, filesystem and services; frontend shared by browser and desktop; Electrobun desktop; website release parsing; and Python CI/release checks were included. Native hardware/visual acceptance and live deployment do not apply to removal of tests and unreachable code. Untracked experimental clients were excluded and preserved.

## Validation

Passed locally after the pruning:

- Frontend: 327 tests, `bun run check` with zero errors/warnings, `bun run build`, and ESLint/Prettier on changed retained TS files.
- Backend: focused service/hub/handler tests, full `go test ./...`, `go vet ./...`, and `go test -race ./internal/websocket -count=1`.
- Desktop: 64 tests via `bun run test:desktop`.
- Website: 3 tests via `bun test`.
- CI scripts: 7 Python unittest tests.
- `git diff --check`.

Mutation proof used temporary Go overlays, without editing production source. The new tests failed as expected when either WebSocket owner check was removed independently and when JWT expiry was mapped to the wrong error. This proves sensitivity to those regressions at the service/hub boundaries; it does not prove WebSocket network transport behavior.

The frontend started at 349 passing tests and now has 327. No speedup claim: full runs remain around 14–15 seconds. The original PR's cited timing failures did not reproduce here.

These checks were run locally before commit. They do not establish remote CI results. The remote PR description was not updated as part of this audit.
