# Browse startup and request ownership

Browse startup at PR #182's `5fe662e31334e96a3f43ccdf325af62a67687852` downloaded 299,323 decoded JavaScript bytes. The unchanged ceiling is 268,288 bytes. The completed combined build downloads 267,560 bytes, saving 31,763 bytes and leaving 728 bytes below the ceiling.

## Ownership decision

Use the source-qualified directory owner from candidate A with candidate B's shared list/grid presentation. A alone omitted lifecycle contracts; B alone saved only 2,503 bytes. The combined implementation was measured after completing those contracts, rather than adding the candidates' estimated savings.

The layout owns one browser lifecycle and forwards job completion. `BrowseRequests` owns shared folders, searches, roots, and drive-space leases, confirmation generations, cancellation, retry, offline waits, and bounded inactive caches. `useBrowseData` retains the existing user-facing action API, filtering, sorting, and selection policy. Only a successful confirmation after pane acquisition may reconcile selection. Superseded responses cannot publish, and invalidation during a pending request queues one follow-up that callers can await.

Session reset disposes old resources, aborts requests and waits, and clears the lazy remote catalog client. Root catalogs share leases across panes. Roots poll every 15 seconds; remote directories and searches retain 15-second polling. Focus, visibility, and reconnect confirm active data. Inactive folder/search caches are pruned after five minutes or beyond 32 inactive entries, with at most one new entry awaiting the next prune. Active panes are never evicted.

The private request state machine replaces the general query engine on the eager Browse path. The remote catalog keeps its shared QueryClient behind the existing lazy desktop boundary and joins domain invalidation/reset explicitly. No query compatibility facade, compiler casts, API weakening, feature removal, or byte-ceiling change was introduced. Application source increases by 263 lines and 7,180 bytes against `5fe662e`; this buys explicit domain lifecycle ownership. Tests and browser harness source increase by 430 lines and 13,226 bytes.

## Production Chromium measurements

The baseline and final asset probes each run 12 samples: three repetitions of cold/warm browser cache over local and 150 ms network profiles. Each served JavaScript asset is compared byte-for-byte with the production build. The official browser suite independently runs 20 samples. Every final sample has 267,560 decoded script bytes, 9 scripts, 1 listing, 1 auth refresh, 29 rendered rows, and no page errors.

Per-asset decoded bytes, baseline to final:

- Early fetch: 1,952 to 1,952.
- Kit start entry: 445 to 445.
- Shared application chunk: 212,199 to 192,170.
- Common chunk: 1,116 to 1,116.
- Application entry: 1,687 to 1,687.
- Layout node: 6,156 to 6,028.
- Error node: 297 to 296.
- Browse route node: 9,176 to 9,167.
- Immediately loaded Browse/Toolbar chunk: 66,295 to 54,699.

The static bundle gate passes at 247,559 raw bytes. That proxy is reported separately from the actual browser measurement. Official files-visible medians are advisory: local cold 162 ms, local warm 123 ms, slow cold 1,434 ms, and slow warm 755 ms. No causal latency improvement is claimed.

## Verification

Passed:

- 239 frontend tests, including shared leases, hover handoff, queued confirmation, retry, offline/session reset, source identity, superseded searches, eviction, shared catalogs, real-HTTP job invalidation, and lazy remote confirmation/reset.
- Svelte check with zero errors/warnings, frontend lint, performance-harness types, production frontend/embedded Go build, and static bundle budget.
- 40 desktop tests and desktop types.
- `go vet ./...` and `go test ./...`.
- Five CI affected-code selector tests.
- Full official `perf:browser`: startup samples, keyboard/windowed list and grid, native watcher, desktop preference persistence, folder/view scroll restoration, shared-pane requests, queued refresh, focus/reconnect, cancellation, logged-out access, all lazy-module recoveries, navigation, selection confirmation, and paste race.
- Chromium through the production desktop Go gateway to a certificate-verified TLS upstream fixture: pairing, qualified listing, split panes, 15-second directory/search polling, focus refresh, and remote catalog refresh. This exercises real network routing with a controlled upstream, not a deployed user's server.

Navigation fixtures previously left extra entries next to the 10,000 startup files. Later root requests then failed with HTTP 413. The original navigation failure was reproduced on unchanged `5fe662e`; the isolated function passed on that same binary. Navigation and ownership fixtures now clean up only their own directories. The 10,000-entry fixture and every behavior assertion remain.

Physical CEF, OS clipboard/drag, live external servers, and a five-minute browser eviction wait were not tested. The Svelte autofixer accepted the new view and remote components; its standalone module analysis failed on the request class, so Svelte check, lint, tests, and real runtime evidence establish that module's result.

## Master integration

Normal merge commits integrate master `1af6e55ccac87d279f609e3121e04b87856c778f` and then `15c51b5256a6cedc581191e7541a14382aa5d9f6`. No rebase or force push is used. Connect conflicts preserve desktop routing and the reviewed persistence/checksum fixes. The reviewed affected-code CI workflow remains, and its desktop job uses the same fail-open selection policy.

The #189 overlap preserves compact listings, successful-confirmation selection, remote qualification, hidden reveal, pane routing, and all desktop assertions. The stricter existing 268,288-byte ceiling wins over master's older 319,488-byte value. The reviewed exact pin/unpin notification assertion is retained. Master was fetched again before final validation and remained `15c51b5`.

## Reproduction and artifacts

Set `BUN_RUNTIME_TRANSPILER_CACHE_PATH=0` for every Bun invocation. From the repository root run frontend tests/check/lint/perf:check, desktop tests/types, backend vet/tests, then `bun run --cwd frontend perf:browser`. Run the official gate after the build, without concurrent rebuilds.

The isolated implementation checkout stores full evidence under `perf-results/domain-owner/`: `design.md`, `surface-ledger.md`, `integration.md`, `baseline.json`, `final.json`, `official-browser.log`, `fixture-proof.log`, `remote-check.log`, check/test logs, and `final.patch`. These temporary diagnostics are not part of the source commit. The official samples are `perf-results/browser.json`.

```sh
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run --cwd frontend test
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run --cwd frontend check
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run --cwd frontend lint
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run --cwd frontend perf:check
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run test:desktop
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run check:desktop
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run --cwd frontend perf:browser
BUN_RUNTIME_TRANSPILER_CACHE_PATH=0 bun run --cwd frontend perf:budget
```

Backend commands run from `backend/`: `go vet ./...` and `go test ./...`. CI selector validation runs from the root with `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`.
