# Native media thumbnails

Implementation on `feat/native-media-thumbnails`, based on desktop PR #182 head `49e8f772c99adec219ea4aca56ee2edea7f7a99b`. Validation below was completed before committing the implementation; no push or merge was performed.

## User problem and result

Cold image grids spent most of their time decoding JPEGs and resizing in Go. Desktop now sends the already-confined read-only file descriptor to an isolated Bun renderer, which calls the bundled Rust library through FFI. Rust handles decode, resize and JPEG encoding, plus bounded FFmpeg/Poppler media extraction. The shared cache, ETags, mount checks and two-renderer limit remain.

Native local previews now include JPEG, PNG, GIF, WebP, BMP, TIFF, ICO and QOI; videos use their first frame, audio uses embedded artwork or a waveform of the first 30 seconds, and PDFs use their first page. Unsupported or broken content keeps the file icon. Remote and portable web image rendering continues through Go.

## Measured user journey

2026-10-25, graphical Linux CEF, final stable runtime `3mi4ad5mvli0t`, bundled Bun 1.4.0. Three runs per renderer in alternating order, same production frontend and 24 distinct paths to the same high-detail 3000×2000 JPEG (4,034,153 bytes). Browser HTTP caching disabled; each run starts a fresh app and empty thumbnail cache. OS file pages are warm. Timing starts at location Enter and ends after image load and two animation frames. Baseline temporarily hides the worker in the diagnostic expanded bundle to select the existing Go renderer; no network responses are mocked.

- Go all-24 rendered: 3831.6, 3845.8, 3821.2 ms; median **3831.6 ms**.
- Rust FFI all-24 rendered: 1005.1, 1081.0, 1006.6 ms; median **1006.6 ms**.
- Median improvement: **3.81×**, or **73.7% less time**.
- First rendered image median: Go **392.6 ms**, Rust FFI **153.1 ms**.

These are local profiling observations, not CI wall-clock gates. The deterministic browser gate remains unchanged: **268,263 / 268,288 startup bytes**, nine scripts, 29 rows, one initial listing and one confirming refresh. No ceiling was raised.

The final runtime archive was expanded for graphical testing; only its diagnostic Chromium port and application identifier were changed. All six apps closed normally with backend shutdown completed. The mixed-media CEF check verified the red video first frame, blue embedded audio art and PDF page, colored audio waveform, PNG/BMP previews, and icon fallback for broken video. Raw runs, driver scripts and screenshot are under ignored `perf-results/native-media/` in this worktree.

## Verification

Passed:

- Backend `go vet ./...` and full `go test -race ./...`.
- Frontend Svelte checks, lint, 274 tests, performance harness type checks and full browser gate.
- Desktop TypeScript checks and 51 tests, including the real private desktop API with six media formats, 2 GiB sparse video seeking, conditional caching, changed-source invalidation, mount rejection, playlist network rejection, and cancellation of watch streams during clean shutdown.
- Six Rust tests: advertised raster formats, alpha handling/aspect fit, malformed/oversized input headers, FFI buffer/descriptor boundaries, and actual child-process resource ceilings.
- Rust 1.88 minimum-toolchain check, Rust formatting, five CI scope-selection tests, and `git diff --check`.
- Final `bun run build:desktop`, followed by the actual packaged CEF runs above.

## Runtime limits and dependencies

The installed app bundles the Rust library and Bun worker. Video/audio needs an installed FFmpeg; PDF needs Poppler (`pdftoppm`). Missing tools leave icons. Rust 1.88+ is a build dependency only.

Each renderer has an eight-second deadline. FFmpeg/Poppler children have 512 MiB address space, three CPU seconds, eight MiB thread stacks and no core dumps. Resetting inherited stack size fixes FFmpeg thread allocation under the packaged Bun runtime without increasing the memory cap. Media output is capped at two MiB and final JPEG output at one MiB. Raster inputs retain the 64 MiB, 8192-dimension and 32-megapixel ceilings. Video/PDF reads stay seekable through the held descriptor. Worker environment excludes desktop credentials and GUI preload libraries. Process-group cancellation stops descendant tools. FFmpeg protocol/demuxer allowlists reject playlists and network sources.

Not rerun: physical clipboard, drag/drop, removable-device and OS-association interactions. Non-Linux targets, every installed FFmpeg codec, encrypted PDFs, AVIF, SVG and camera RAW thumbnail decoding are not covered or promised by this change.
