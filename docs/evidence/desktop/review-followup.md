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
