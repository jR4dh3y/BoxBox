# Folder-listing follow-up evidence

Measured on 2026-10-23 with Go 1.27.1, Bun 1.4.4 and headless Chromium through the repository's Playwright workflow. Wall-clock timings are advisory. No existing ceiling was raised.

## Web listing and refresh

The focused follow-up compares against `e400f8a` (master after #169). `bun run perf:browser --base origin/master` alternates base/head order across five samples per profile and cache state, at 1440×1200 with 10,000 files. The slow profile uses 150 ms latency and 200,000 bytes/s download throughput. Warm means HTTP assets are cached; the folder is still confirmed by the server on each navigation.

First rows rendered, medians:

- Local cold: 167 → 161 ms.
- Local warm: 140 → 141 ms.
- Slow cold: 1,375 → 1,365 ms.
- Slow warm: 901 → 764 ms (15% faster).

A same-fixture comparison of the file-info directory response and the listing response retained identical metadata after path reconstruction: 1,918,881 → 1,628,881 raw bytes and 62,229 → 32,814 gzip bytes at level 5. The listing shares the canonical parent path instead of repeating each full child path. Named fields and complete metadata remain; file-info, search, and public-share contracts stay as before.

The HTTP fixture gates 1,000 deterministic entries at 150,000 raw bytes and 4,096 gzip bytes (observed 148,027 and 3,067). Fixed timestamps keep this proxy independent of fixture creation speed. The web static-JS ceiling decreased from 321,536 to 319,488 bytes (measured 319,208).

The browser verifies that a failed refresh preserves selection and cached rows; retrying against the server removes the deleted selected entry and retains the surviving selection. Unit coverage also preserves selected hidden entries and avoids replacing an unchanged selection. Notification UI loads when needed on browse and public-share pages; the navigation flow exercises pin/unpin notifications and dismissal.

## Desktop integration

The desktop branch integrates #169's directory reader, cancellation checks, allocation ceilings and sorting helpers while retaining panes, range selection, transfers and the unlimited local-desktop listing configuration.

The startup row ceiling remains 32 and is tested at an explicit 1440×900 viewport. The old test incorrectly combined that ceiling with a 1200 px viewport that requires more than 32 visible rows. Large-viewport coverage separately exercises the 10,000-entry grid at 1440×1200: 81/99/73 tiles at the top/middle/bottom, each within the 150-element ceiling, with one listing request.

Against an actual private desktop-mode Go server and temporary OS directory, 20 writes plus deletion of the selected seed file triggered four sequential refreshes with no overlapping requests. The last file appeared, the deleted selection disappeared, and split panes opened and closed. F1, selection details, navigation, settings, public shares, lazy-chunk recovery and delayed-paste clipboard handling passed.

## Checks and limits

- Both branches: frontend type checking, lint, unit tests, production embedded builds, browser timing/behavior checks, performance-script type checks, bundle budgets, Go tests and Go vet passed.
- Web: 120 tests; desktop integration: 197 frontend tests and 46 desktop tests.
- Desktop integration: desktop TypeScript check and focused Go race checks for listing behavior passed.
- No CEF GUI/package launch, real removable-drive mount/eject, remote-server login, network-mounted filesystem or screen-reader session was exercised in this round. The desktop-mode browser test uses the real server and filesystem, without the CEF host.

The next measured performance target is cold startup: the slow cold web median is still 1,365 ms, versus 764 ms with assets cached. Inspect the script/request waterfall before adding more protocol complexity.
