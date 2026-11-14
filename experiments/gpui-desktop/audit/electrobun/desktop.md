# Electrobun desktop host

## Coverage
| File | Lines | Read |
|---|---:|---|
| `/home/radhey/code/BoxBox/desktop/electrobun.config.ts` | 38 | 1–38 |
| `/home/radhey/code/BoxBox/desktop/native/.gitignore` | 1 | 1–1 |
| `/home/radhey/code/BoxBox/desktop/native/Cargo.lock` | 387 | 1–387 |
| `/home/radhey/code/BoxBox/desktop/native/Cargo.toml` | 19 | 1–19 |
| `/home/radhey/code/BoxBox/desktop/native/src/lib.rs` | 129 | 1–129 |
| `/home/radhey/code/BoxBox/desktop/native/src/media.rs` | 165 | 1–165 |
| `/home/radhey/code/BoxBox/desktop/native/src/render.rs` | 183 | 1–183 |
| `/home/radhey/code/BoxBox/desktop/scripts/build-native.ts` | 41 | 1–41 |
| `/home/radhey/code/BoxBox/desktop/scripts/build-server.ts` | 22 | 1–22 |
| `/home/radhey/code/BoxBox/desktop/scripts/install.ts` | 615 | 1–615 |
| `/home/radhey/code/BoxBox/desktop/scripts/integration.ts` | 255 | 1–255 |
| `/home/radhey/code/BoxBox/desktop/scripts/native-smoke.ts` | 178 | 1–178 |
| `/home/radhey/code/BoxBox/desktop/scripts/portal.ts` | 178 | 1–178 |
| `/home/radhey/code/BoxBox/desktop/scripts/prepare-launcher.ts` | 43 | 1–43 |
| `/home/radhey/code/BoxBox/desktop/scripts/runtimeArchive.ts` | 273 | 1–273 |
| `/home/radhey/code/BoxBox/desktop/scripts/smoke.ts` | 82 | 1–82 |
| `/home/radhey/code/BoxBox/desktop/scripts/thumbnail-fixtures.ts` | 96 | 1–96 |
| `/home/radhey/code/BoxBox/desktop/src/app.ts` | 213 | 1–213 |
| `/home/radhey/code/BoxBox/desktop/src/assets.d.ts` | 8 | 1–8 |
| `/home/radhey/code/BoxBox/desktop/src/chooser.ts` | 76 | 1–76 |
| `/home/radhey/code/BoxBox/desktop/src/connections-host.ts` | 59 | 1–59 |
| `/home/radhey/code/BoxBox/desktop/src/desktop-entry.ts` | 13 | 1–13 |
| `/home/radhey/code/BoxBox/desktop/src/index.ts` | 66 | 1–66 |
| `/home/radhey/code/BoxBox/desktop/src/instance.ts` | 612 | 1–612 |
| `/home/radhey/code/BoxBox/desktop/src/launch.ts` | 320 | 1–320 |
| `/home/radhey/code/BoxBox/desktop/src/launcher.ts` | 111 | 1–111 |
| `/home/radhey/code/BoxBox/desktop/src/renderer-target.ts` | 15 | 1–15 |
| `/home/radhey/code/BoxBox/desktop/src/rendering.ts` | 61 | 1–61 |
| `/home/radhey/code/BoxBox/desktop/src/server.ts` | 154 | 1–154 |
| `/home/radhey/code/BoxBox/desktop/src/target.ts` | 41 | 1–41 |
| `/home/radhey/code/BoxBox/desktop/src/thumbnails/native.ts` | 65 | 1–65 |
| `/home/radhey/code/BoxBox/desktop/src/thumbnails/request.ts` | 37 | 1–37 |
| `/home/radhey/code/BoxBox/desktop/src/thumbnails/worker.ts` | 39 | 1–39 |

## Behaviours
| # | Behaviour | Source | GPUI | GPUI evidence | Notes |
|---:|---|---|:---:|---|---|
| 1 | Bundle identity is BoxBox, ID `io.boxbox.desktop`; version is read from `VERSION` and must match the release-version pattern. | electrobun.config.ts:4 | — | none found | Different native executable identity; packaging metadata is not part of this GPUI source. |
| 2 | Desktop bundle uses CEF on Linux/macOS/Windows; Linux enables GPU and compositing by default and includes CEF. | electrobun.config.ts:16 | — | none found | Native GPUI replaces the webview runtime. |
| 3 | Build copies the Go server and native thumbnail worker/library into resources; native build is Linux-only and fails with explicit errors on target/build failure. | build-server.ts:5; build-native.ts:4 | — | none found | Packaging/build behavior, no counterpart found in GPUI source. |
| 4 | Installer defaults to stable bundle path; `--source` may be used once, as separated or `=` form; `--set-default`, `--help`/`-h`; unknown options and missing values error. | install.ts:25,127 | — | none found | Installer CLI not found in GPUI source. |
| 5 | Installer uses absolute `XDG_DATA_HOME`, else `$HOME/.local/share`; missing/relative home configuration errors. It installs under `boxbox`, not user file-manager roots. | install.ts:195 | — | none found | No equivalent installation workflow found. |
| 6 | Rejects bundle source/install directory overlap; accepts only expanded runtime or self-extracting archive with stable BoxBox metadata and executable launcher/server files. | install.ts:61,235,313 | — | none found | Installer validation behavior. |
| 7 | Rejects malformed/missing metadata, wrong identifier/channel, invalid archive hash, absent runtime payload/version, and non-executable launcher with specific diagnostics. | install.ts:251,275,286,309,324 | — | none found | Installer diagnostics. |
| 8 | Existing app directory is replaced only when it is a real directory with valid BoxBox install marker; symlinks, unrelated directories, malformed markers, unrelated desktop entries and non-regular icon paths are refused. | install.ts:412,443,465 | — | none found | Preserves unrelated files; GPUI experiment has no install routine. |
| 9 | Installer stages to a temporary directory, atomically swaps managed app with rollback on failure, and leaves a failed-cleanup stage/backup rather than risking unrelated deletion. | install.ts:90,481,505 | — | none found | Filesystem update/recovery contract. |
| 10 | Writes a managed desktop entry and scalable hicolor SVG icon atomically with mode `0644`; Exec argument is quoted and percent/quote/backslash/backtick/dollar/control characters are handled or rejected. | install.ts:90,475; desktop-entry.ts:2 | — | none found | No desktop-entry installer found in GPUI source. |
| 11 | Desktop entry registers folder and `file://` handling (`%U`); setting defaults is explicit opt-in and sets `inode/directory` plus `x-scheme-handler/file` via `xdg-mime`; command failure includes status/stderr. | install.ts:106,467,523 | ◐ | workspace.rs:1130 | GPUI accepts folder/file launches, but no default-handler registration found. |
| 12 | Linux app targets stable single-instance behavior; repeated launches forward activation, local targets or remote launch requests to the running app and exit without starting CEF twice. | index.ts:28; instance.ts:67; native-smoke.ts:101 | ✓ | instance.rs:1-5; main.rs:89-93 | GPUI forwards a string launch location and opens it in its current window. |
| 13 | Instance socket is per-user, under valid private XDG runtime directory or temp; private directory mode `0700`, socket mode `0600`, socket path length checked. | instance.ts:40,61,204 | ◐ | instance.rs:25-29,53-56 | GPUI uses XDG runtime/temp and sets directory mode, but source uses a simpler socket ownership/recovery design; no exact privacy/ownership checks evident. |
| 14 | Instance handoff accepts only validated versioned JSON envelope, exact request shapes, local virtual paths or HTTPS remote URLs; caps message at 8192 bytes and times out after 2 seconds. | instance.ts:24,122,343,377 | ◐ | instance.rs:16-20,42-50 | GPUI line sends a JSON string with 8KiB read cap, without the Electrobun typed envelope/target validation. |
| 15 | Concurrent stale-socket recovery uses process-shared guard, ownership/inode checks, retries up to 8 attempts, waits up to 3s for guard; launch handoff waits up to 30s for owner. | instance.ts:27,72,284; launcher.ts:31 | ◐ | instance.rs:42-50 | GPUI removes the socket after failed connect without equivalent guarded recovery. |
| 16 | Launch args accept one or more paths, `--`, `--select PATH` / `--select=PATH`; duplicate select, unknown switches, empty/control-containing paths error. | launch.ts:143,247 | ◐ | workspace.rs:1130-1145 | GPUI reads only first arg as initial location; verify behavior for flags, multiple paths and control chars. |
| 17 | Relative paths resolve against cwd; local absolute paths and local `file://` only; remote hosts, other URL schemes, credentials, query/fragment, malformed escapes and nonabsolute file URLs are rejected. | launch.ts:32,279 | ◐ | workspace.rs:1130-1145 | GPUI supports simple `file://` conversion, but source evidence does not show equivalent validation/percent decoding. |
| 18 | Directory launch opens directory; regular file opens parent and selects item; `--select` rejects directories and requires selection directly inside supplied directory; nonexistent/special files have explicit errors. | launch.ts:72,290 | ◐ | workspace.rs:1130-1155 | UI_MAP says “Show in folder” does not select item yet (`known`). |
| 19 | Multiple URL arguments open only the first target; remaining items are reserved for future tab behavior. | launch.ts:79 | ◐ | instance.rs:1-5 | GPUI has launch forwarding/new-tab support, but no evidence here that a list is accepted and later targets retained. |
| 20 | Packaged Linux launcher recovers args from up to six process ancestors because Electrobun does not forward them; unsupported/unrecoverable process command line falls back to direct args. | launch.ts:130,168 | — | none found | Electrobun-specific process-tree workaround. |
| 21 | Pre-native launcher handles chooser enable/disable commands and forwards launches before native CEF startup; launch arbitration waits 30s, retries every 50ms; managed shutdown closes stdin and escalates to process-group SIGKILL after 5s. | launcher.ts:13,31,46,76 | — | none found | GPUI is a single native process, with no separate CEF launcher. |
| 22 | Rendering preference defaults to enabled, reads app/channel `preferences.json` (128KiB max), warns and falls back to enabled on read/parse errors, then rewrites build Chromium flags before CEF starts; unwritable bundle gives actionable error. | rendering.ts:5,10,33 | — | none found | GPUI is not Chromium-backed. |
| 23 | Local server binds loopback only, creates random 32-byte token, temporary working/config paths, root/home and existing standard XDG folders as mounts, and private data/config permissions. | server.ts:9 | ✓ | main.rs:61-78 | GPUI local provider mounts filesystem directly; runtime architecture is replaced. |
| 24 | Server environment strips `BOXBOX_*`, `FM_*` and `CONFIG_PATH` overrides; starts with a 30s readiness timeout and validates emitted URL is loopback HTTP; unexpected exit quits desktop. | server.ts:31,76,120 | — | none found | Local backend lifecycle is Electrobun-specific. |
| 25 | App opens a 1280×800 CEF window, sandboxed, denying appData/views protocols; starts server lazily; pending external targets wait for DOM readiness and are queued without navigating away from existing tabs. | app.ts:14,54,99,122,157 | ◐ | main.rs:118-146 | GPUI opens a native 1280×800 window; GPUI launch behavior uses app workspace, while the CEF sandbox and queued renderer bridge are absent/replaced. |
| 26 | Closing the last app/chooser window quits; quit aborts chooser polling, stops server and closes instance socket; SIGINT/SIGTERM and managed launcher pipe closure trigger shutdown. | app.ts:16,42,71,89; index.ts:45 | ◐ | main.rs:118-146 | GPUI window shutdown exists through framework; explicit server/chooser/instance cleanup sequence is not applicable, though instance cleanup is a known implementation concern. |
| 27 | Remote launch URL must be HTTPS without credentials/query/fragment; backend saves/opens the profile with a 10s timeout; user gets a specific message on failure; response must be UUID. | target.ts:1; connections-host.ts:16 | ✓ | main.rs:63-70; workspace.rs:1130 | Native provider supports remote API profiles; inspect app behavior separately for UI parity. |
| 28 | `org.freedesktop.FileManager1` chooser service polls private server every 500ms on transient errors, validates token and 32-hex request IDs, opens one sandboxed CEF chooser window per request, closes stale windows; closing a chooser posts cancel. | chooser.ts:2; app.ts:34,53,69 | ◐ | main.rs:95-103; filemanager1.rs:1-49 | GPUI FileManager1 is present, but portal FileChooser replacement/request/cancel flow is absent (`known`: system file picker through xdg portal). |
| 29 | Portal install is reversible explicit `--enable`/`--disable`, modifies only effective FileChooser route, preserves prior line and refuses edited/duplicate managed routes; requires installed `boxbox.portal`, absolute paths, limits config to regular files ≤128KiB, atomic mode-0600 writes. | portal.ts:8,67,116 | ✗ | none found | Known gap: xdg portal file picker. |
| 30 | Portal routing discovers desktop-specific and generic config in XDG/system roots; D-Bus service is managed-only and execs launcher `--portal`; prints that portal restart/login is required. | portal.ts:79,104,160 | ✗ | none found | Known gap. |
| 31 | Native thumbnail requests accept image/video/audio/pdf only, exactly three args and dimensions 16–512; source must be an open regular file descriptor; errors have stable exit codes/messages. | request.ts:1; worker.ts:6 | ◐ | media.rs:60-71; workspace/preview.rs:117 | GPUI can produce inline thumbnails but no shared freedesktop thumbnailer/cache contract found (`known`: shared thumbnails). |
| 32 | Raster thumbnails support JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/QOI; preserve aspect ratio, Catmull-Rom resize, alpha black matte, JPEG quality 82; reject oversized input/dimensions/decode allocation and panic safely. | render.rs:6,24,41; lib.rs:18,40 | ◐ | media.rs:60-71 | GPUI native media supports thumbnails; audited source does not establish shared freedesktop cache/thumbnailer integration. |
| 33 | Video/audio/PDF thumbnails use FFmpeg/Poppler subprocesses with 3s CPU, 512MiB address-space, 8MiB stack and output/pixel/probe limits; PDF is first page; audio waveform fallback after unsupported video decode. | media.rs:8,21,63 | ◐ | media.rs:60-71 | GPUI thumbnail generation exists. No system thumbnailer registration or shared cache found (`known`). |
| 34 | Thumbnail library ABI is checked; unavailable/unsupported library and invalid source/limits map to fixed error codes; worker writes JPEG bytes to stdout and logs errors to stderr. | lib.rs:10; native.ts:15; worker.ts:26 | ◐ | media.rs:60-71 | Similar result capability exists; no equivalent packaged worker/FFI ABI surfaced in GPUI. |
| 35 | Test-only scripts exercise server bootstrap, security/confinement, websocket, trash recovery, Open With/GIO associations, chooser and second-process handoff; fixture generator makes media/PDF/corrupt samples. | smoke.ts:12; integration.ts:57; thumbnail-fixtures.ts:5; native-smoke.ts:88 | — | none found | These are checks/fixtures, not product runtime behaviors. |
| 36 | Cargo lock pins raster decoder/resizer dependencies; `.gitignore` excludes native `target/`; release library is LTO, one codegen unit, stripped. | Cargo.lock:1; Cargo.toml:1; .gitignore:1 | — | none found | Build metadata only. |

## Missing or partial, ranked

1. **xdg-desktop-portal FileChooser (rows 28–30, ✗/◐, known):** GPUI lacks portal routing and request/cancel windows, so GTK/libadwaita apps cannot use BoxBox as the system picker.
2. **Shared freedesktop thumbnails (rows 31–34, ◐, known):** GPUI renders thumbnails in its own cache, but no system thumbnailer registration or shared `~/.cache/thumbnails` behavior is present.
3. **Launch target validation and item selection (rows 16–19, ◐, known):** external file launches need robust URI/argument handling and selecting the requested item; the UI map explicitly notes selection for “Show in folder” is unfinished.
4. **Desktop registration/default handler (rows 8–11, ✗/◐):** no native install path registers a `.desktop` entry, icon, folder handler or `file://` default, limiting discoverability and OS launch integration.
5. **Single-instance hardening (rows 13–15, ◐):** GPUI has handoff, but its implementation lacks Electrobun's guarded stale-socket recovery and strict request validation/privacy checks.
6. **FileManager1 semantics (row 28, ◐):** GPUI claims the bus name only if unowned and handles the three methods, but forwards URIs as opens and does not yet select the requested item (`known`).
