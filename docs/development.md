# Development

This guide covers the local BoxBox development workflow.

## Prerequisites

- Go 1.27+
- Bun 1.1+
- Node.js 22+ for Svelte tooling compatibility
- `air` for backend hot reload:

```bash
go install github.com/air-verse/air@latest
```

Make sure `$GOPATH/bin` is on your `PATH`.

## Repository Layout

```text
backend/      Go API server and embedded static host
frontend/     SvelteKit app compiled to static files
docs/         Markdown project documentation
scripts/      Local development helpers
Dockerfile    Single-container production build
```

## Install Dependencies

```bash
bun install --cwd frontend
cd backend && go mod download
```

## Run the Product App

From the repository root:

```bash
bun run dev
```

This starts:

- SvelteKit frontend through Vite.
- Go backend through `air -c backend/.air.toml`.

The backend uses `backend/config.yaml`. For a safer local setup, edit that file to point at temporary directories instead of broad host paths:

```bash
mkdir -p /tmp/boxbox/media /tmp/boxbox/documents
```

```yaml
mount_points:
  - name: "media"
    path: "/tmp/boxbox/media"
    read_only: false
  - name: "documents"
    path: "/tmp/boxbox/documents"
    read_only: false
```

## Run Pieces Separately

Backend:

```bash
cd backend
go run ./cmd/server -config config.yaml
```

Frontend:

```bash
bun run --cwd frontend dev
```

## Build

Frontend app:

```bash
bun run --cwd frontend build
```

Self-contained executable with the embedded frontend:

```bash
bun run build:single
./dist/boxbox -config backend/config.yaml
```

The build command uses Bun to generate the static frontend and Go to produce the
only runtime artifact. Bun is not required on the machine that runs the binary.

For local testing without login credentials:

```bash
./dist/boxbox --dev -config backend/config.dev.yaml
```

Development mode disables HTTP and WebSocket authentication and always binds to
`127.0.0.1`, even if the configuration specifies another host. Never use it for
a shared or production deployment.

Production container:

```bash
docker build -t boxbox:local .
```

The Dockerfile builds the frontend first, copies `frontend/build` into `backend/internal/static/dist`, and compiles the Go server with embedded assets.

## Test and Validate

Backend:

```bash
cd backend
go test ./...
```

Frontend checks:

```bash
bun run --cwd frontend check
bun run --cwd frontend build
bun run --cwd frontend perf:budget
```

`perf:budget` fails when the raw JavaScript the app loads without a dynamic import grows past `frontend/perf-budget.json`. After shrinking the bundle, run it with `--lower` to lock the gain in. Never raise the budget to pass. Initial vendor and app chunks are grouped by directory to reduce request overhead; the gate also rejects circular chunk imports.

Directory listing and search cost are tracked by `BenchmarkListDirectory` and `BenchmarkSearchWalk`, and gated by `TestListDirectoryAllocationCeiling` and `TestSearchAllocationCeiling` in `backend/internal/service`. Lower a ceiling when you win; never raise one to pass a change.

Cached folder listings are shown at once but never trusted: an open folder always revalidates (`directoryQueryOptions`), and a finished copy, move or delete job invalidates every cached listing and drive stat. A job that ends while the WebSocket is down is caught when it reconnects, because the client reloads the job list then. The server does not push file changes, so a change made outside BoxBox appears on the next open or refresh.

Folder listings are complete, unsorted responses, not pages; one cached listing per folder serves every sort and the hidden-file toggle, so changing them sends no request. `fixedRowWindow.svelte.ts`
renders a bounded window for both list and grid, while selection and actions use
the complete collection. Its scroll anchors are bounded and keyed by folder,
search, sort, view, and hidden-file setting. The 10,000-entry server limit still
applies. Search and public-share folders keep their separate response contracts.

The first load starts the folder listing and the session refresh before the app's JavaScript has arrived. `frontend/static/early-fetch.js` makes both requests, and `src/lib/api/earlyStart.ts` hands their results to the app once; anything that does not match, fails or arrives too late is requested again by the app. The script must ask for exactly what `listDirectory` asks for, so change them together: `earlyStart.test.ts` fails when they differ. It is the only place the refresh happens on a page load, because each refresh rotates the refresh token. The server sends the script with `no-cache`, since it has to match the app that loads it.

`vite.config.ts` puts the `node_modules` and `src/lib` code that loads on the first visit into two chunks, so the first load is about 7 requests instead of about 25 and the browser's six connections per host stop queuing them. Grouping by directory keeps the chunks acyclic, and `perf:budget` fails if they ever form a loop: a loop can run a module before its chunk has initialised and leave a blank page, which no build or unit test shows.

### Performance regression guards

The existing **Backend Go tests** and **Frontend check and build** jobs include the performance gates. Keep their names stable: `master` requires both checks from GitHub Actions, an up-to-date branch and a pull request, including for administrators. No approving review is required by this configuration. The reproducible protection configuration is `.github/master-protection.json`; an administrator can apply it with:

```bash
gh api --method PUT repos/jR4dh3y/BoxBox/branches/master/protection \
  --input .github/master-protection.json
```

Blocking checks:

- The static JS ceiling lives in `frontend/perf-budget.json`. Circular chunks also fail.
- Chromium loads a real, production-mode embedded server with an ephemeral login and a 10,000-file directory. Every cold and warm navigation must expose the complete folder count while rendering at most 150 file elements, make exactly one listing and one session refresh, stay within the JS resource/decoded-byte ceilings in `frontend/scripts/perf/budget.json`, and produce no uncaught browser errors. Logged-out folder and root visits must redirect to login with one refresh and no private file/mount requests; protected pages must not mount before authentication succeeds.
- Backend benchmarks use identical fixtures and the same runner/compiler for the PR base and candidate. Directory benchmarks request the complete 5,000-entry fixture on both revisions, including historical paginated baselines. Process-wide MIME tables initialize before timing so their one-time allocations do not depend on the benchmark iteration count. Ten samples per revision alternate their execution order. More than 1% median allocation/byte growth fails; absolute ceilings in `backend/internal/service/perf-budget.json` also prevent cumulative small regressions. Existing allocation tests read that same file. Missing benchmarks, sample sets or ceilings fail closed.
- All three budget JSON files are compared with the PR base before measurement. Existing limits may only decrease; deleting a limit or raising it fails. A change to these guards or their fixture sizes deserves explicit review, not an automatic baseline update.

Wall-clock results are **advisory**, as required by the repository's performance policy. `benchstat` reports backend timing significance; the browser reports five samples and medians for cold/warm caches on both local networking and a throttled **150 ms RTT, 1.6 Mbit/s download** profile. Timing is marked after the first visible file rows render, not when the spinner disappears or the network becomes idle. Cold/warm refer to the browser's HTTP cache, not the OS filesystem cache. This is a Chromium/HTTP/1.1 fixture, not a physical-device or HTTP/2 deployment test. A sustained slowdown still needs investigation even when deterministic gates pass.

Run locally from the repository root:

```bash
bun install --cwd frontend --frozen-lockfile
(cd frontend && bunx playwright install chromium)
bun run --cwd frontend perf:browser --base origin/master

go install golang.org/x/perf/cmd/benchstat@v0.0.0-20261009192801-be2c69fb417e
BENCHSTAT="$(go env GOPATH)/bin/benchstat" bun run --cwd frontend perf:backend --base origin/master
```

Omit `--base` for a browser-only candidate check. In stacked PRs CI compares against the immediate target branch; after the stack lands that target is `master`. The baseline is a temporary Git archive, so comparisons never switch or modify the working checkout. Browser runs build both embedded binaries and use isolated loopback servers, private temporary mounts and in-memory credentials; they do not reuse local configuration or cookies. Chromium, Bun, benchstat and the CI runner image are pinned.

CI uploads raw samples, benchmark summaries, browser byte/request counts and failure diagnostics as 14-day artifacts, even when a gate fails. Local results are ignored under `perf-results/`. No automatic review requests, budget resets or baseline commits are performed.

## Check concurrent browser refresh

From `frontend/`, run `bun scripts/auth-session-browser.ts` with Playwright Chromium installed. This builds the real API client and Go server, sends overlapping refreshes from two pages sharing one cookie jar, and delivers the stale response after the replacement cookie. Both pages must recover without losing their cookie. It also verifies that explicit logout still ends the other page's session.

## Check active-session authentication

On Linux, run the production-mode auth smoke check from the repository root:

```bash
bun scripts/check-auth-sessions.ts
```

The script builds the Go server when you omit `--binary`. It starts the server with temporary storage and prints a results table.

The entry point owns CLI handling, orchestration, and cleanup. Focused helpers and session, storage-failure, strict-store, backup-rollback, and missing-store scenarios live in `scripts/auth-smoke/`. Strict-store checks require the corresponding auth-session-store initialization diagnostic; an unrelated startup error is not a pass.

The smoke check covers legacy tokens without a session ID, unknown session IDs, independent logins, access tokens across refresh rotations and logout, concurrent refresh, private streaming credentials in headers and query parameters, WebSocket authentication, login/refresh/logout storage failures and retry, previous-session preservation, a missing registry file or a valid snapshot with an empty `sessions` map after restart, strict startup rejection of malformed or unsafe registries, and file permissions. It also checks that the registry contains no raw JWTs.

The script labels two outcomes as expected limitations, not passes. Deleting the registry file while the server runs leaves its in-memory sessions active, and a later write can persist them again. Restoring a valid older backup can revive its sessions and current refresh tokens. The smoke check does not claim to test physical power-loss durability.

To check an existing server binary, pass its path:

```bash
bun scripts/check-auth-sessions.ts --binary ./dist/boxbox
```

The `--dev` server mode still initializes `data_dir`; configure a writable directory even though authentication is disabled.

## Backend Patterns

Use the existing Handler -> Service -> Model/Filesystem shape:

- Handlers parse requests and write responses.
- Services own business logic and filesystem work.
- Models define request, response, and domain types.
- Shared constants belong in `backend/internal/config/constants.go`.
- File operations should go through `internal/pkg/filesystem` and existing validators.

## Frontend Patterns

- Use Svelte 5 patterns already present in the repo.
- API calls belong in `frontend/src/lib/api`.
- Shared UI primitives belong in `frontend/src/lib/components/ui`.
- Formatting belongs in `frontend/src/lib/utils/format.ts`.
- File type logic belongs in `frontend/src/lib/utils/fileTypes.ts`.
- Central config belongs in `frontend/src/lib/config.ts`.
