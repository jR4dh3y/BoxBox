# Desktop review follow-up

The private desktop preferences API persists favorites, appearance and opening behavior across loopback-port changes. Remote root/stat queries run separately from the local catalog. Thumbnail forwarding accepts its version key. Per-pane folder/view scroll positions and page-sized keyboard navigation are restored.

Selection reconciliation runs once per confirmed directory response and compares canonical server paths. Browser regressions cover failed/successful refresh, closing search without a new directory response, recursive results, and aliased folder paths containing `./`. Notification expiry begins when its lazy renderer attaches, so downloading that renderer cannot consume the notification's visible lifetime.

Validation performed on this follow-up:

- Backend vet and full tests passed. Targeted race coverage for desktop preferences/servers, Connect and property tests passed.
- Frontend check, lint, 229 tests, build and performance-harness typecheck passed.
- Desktop check and 40 tests passed.
- Real browser checks passed for scroll/view restoration, page keys, selection refresh/search/canonical-path behavior, offline remote catalogs leaving local writes usable, preferences after a new-origin restart, and notifications after a deliberately delayed renderer download.
- Static bundle: 267728 raw bytes, under the unchanged 268288-byte ceiling.
- **Still failing:** the complete cold/warm browser performance gate. Its local/cold sample downloads 299299 decoded script bytes, exceeding the effective 268288-byte ceiling. No budget was raised, benchmark removed, or navigation assertion dropped. This PR remains unready until the startup-byte failure is fixed.

Native OS app launch, real clipboard/Trash/device operations and external drag/drop were not rerun in this follow-up. CodeQL request-forgery alerts were reviewed as intentional authenticated desktop networking: user-selected saved HTTPS authorities, private loopback cookie plus exact Host/Origin gates, route/header/query restrictions, verified TLS and no redirects.

## Root completion follow-up, 2026-10-25

Greptile reviewed commit `1779624480c3e0334936f957c6c34b47e29b798b` at `2026-10-24T20:32:44Z`. Its current-head review identified the missing roots prop between EditablePathBar and PathEditor. The browse route already supplies the active pane's loaded roots through Toolbar. EditablePathBar now forwards those roots, and PathEditor requires the prop so another omission fails typechecking.

The shared toolbar serves web browsing and local desktop browsing, including remote mounts supplied by the active pane. There are no other PathEditor callers. Child suggestions still use the directory API. API contracts, providers, and native filesystem operations do not change.

- PASS: Chromium against the production Go binary restored empty and case-insensitive root suggestions, Tab and Right Arrow completion, child suggestions, pointer selection, reopening the editor, and Enter navigation, with no page errors. The regression runs before the timing samples in `frontend/scripts/perf/browser.ts` and uses its own small mount so the 10,000-entry timing fixture stays unchanged.
- PASS: frontend typecheck, lint, 229 tests, performance-script typecheck, desktop typecheck, and 40 desktop tests.
- FAIL: the full browser startup gate still measures 299323 decoded JavaScript bytes against the unchanged 268288-byte ceiling. Before this fix it measured 299299 bytes. Both runs used nine script requests, one listing request, and one session refresh. Bundle experiments were discarded because they increased requests or bytes, or broke initialization. No bundling configuration or performance ceiling changed.
- NOT TESTED: physical CEF, real remote-server completion, native OS actions, and the full browser scenarios after the failing startup assertion. The focused completion test uses a real local mount through the shared web toolbar.

The startup-byte failure remains unresolved. This follow-up does not make PR #182 ready to merge.
