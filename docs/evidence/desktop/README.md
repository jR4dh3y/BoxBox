# Linux CEF desktop verification

Verified on Linux x64 with Bun 1.4.3. These results cover the CEF desktop and shared frontend/backend changes in this PR.

## Runtime evidence

The installed stable runtime passed `desktop/scripts/native-smoke.ts` in temporary XDG directories, without changing desktop associations:

```text
Native smoke passed: isolated runtime install, CEF folder launch, second-launch handoff without CEF, backend shutdown.
```

The test starts the real packaged executable with `--select`, waits for the requested folder's file-list request, checks that CEF started, and launches a second selection. It verifies that the second invocation exits successfully without starting CEF and that the existing process requests the second folder. Finally it verifies backend shutdown. This is native process and navigation evidence; it does not assert every rendered pixel or interaction.

`desktop/scripts/integration.ts` exercised the real private Go server and filesystem: Trash/restore, restore conflicts without overwrites, symlink preservation, names containing `..`, default opening, Open With through a fixture application, and volume listing. It also rejected an ambiguous percent-containing clipboard selection through HTTP before publishing it. The fixture opener records its input rather than opening a user application. Volume listing does not prove device mount or eject.

`desktop/scripts/smoke.ts` verified private bootstrap, embedded UI, mount confinement, foreign-Origin rejection, authenticated WebSocket upgrade, and server shutdown.

The browser fixture below shows two tabs and split panes. Clicking Split created two file-browser panes; opening another tab created an independent pane. This screenshot is browser layout evidence, separate from the installed CEF test.

![Two tabs and split panes in the desktop-mode browser fixture](split-panes.png)

## Automated checks

| Command | Result |
| --- | --- |
| `bun install --frozen-lockfile` | Passed; lockfile unchanged |
| `bun run check:desktop` | Passed |
| `bun run test:desktop` | 45 passed, 0 failed |
| `bun run --cwd frontend check` | 0 errors, 0 warnings |
| `bun run --cwd frontend lint` | Passed |
| `bun run --cwd frontend test` | 145 passed, 0 failed |
| `bun run --cwd frontend build` | Passed |
| `bun run --cwd frontend perf:budget` | 301308 bytes against 302080 bytes allowed |
| `bun run --cwd website build` | Passed |
| `bun run build:desktop` | Stable Linux CEF package built |
| `go vet ./...` in `backend/` | Passed |
| `go test ./...` in `backend/` | Passed after the clipboard fix |
| `go test -race ./internal/service ./internal/handler ./cmd/server` in `backend/` | Passed; focused clipboard race tests rerun after the clipboard fix |
| `git diff --check` | Passed |

## Reproduce the native and API checks

After installing the Linux prerequisites in [the desktop guide](../../../desktop/README.md), run these commands from the repository root in a graphical session:

```sh
bun install --frozen-lockfile
bun run build:desktop
bun run --cwd desktop scripts/integration.ts
bun run --cwd desktop scripts/smoke.ts
bun run --cwd desktop scripts/native-smoke.ts
```

## Limits

- Docker image build was not run because the Docker daemon is unavailable.
- Real removable-device mount/unmount/eject, remote-server login, and macOS/Windows integration were not tested.
- Archive tools, bulk rename, permission editing, outbound native drag, and remote download-and-open are not implemented.
- Clipboard publication rejects paths containing `%`. Native clipboard imports reject paths that the shared virtual-path resolver would reinterpret as another file. A regression covers both siblings being present and cut cleanup after the original moves.
- No default handlers were changed, and no release was published.
