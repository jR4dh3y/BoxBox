# Media FFI follow-up measurements

2026-10-26. One-off profiling in the existing native-media worktree. Production source was unchanged during this investigation; the prototypes and generated fixtures are ignored under `perf-results/native-media/`. This report records experiments performed before committing the thumbnail implementation; no push was performed.

## Direct library calls

A small C++ shared-library prototype exposes a C ABI to Bun FFI. FFmpeg's libavformat/libavcodec/libswscale opens the held descriptor, decodes the first video/artwork frame and produces an aspect-fitted RGB buffer. Poppler's C++ renderer produces an RGB first page. A Rust wrapper can call the same library APIs; rewriting the codecs in Rust is unnecessary.

The comparison uses the bundled Bun 1.4.0, 256-pixel outputs, three warmups and twelve measured samples per path, alternating command/FFI order. Inputs are tiny synthetic fixtures on warm local filesystem pages. Command paths also produce uncompressed pixels (raw RGB for FFmpeg, PPM for pdftoppm), so this does not credit FFI for eliminating PNG compression. Both paths verify output dimensions/byte counts and red/blue pixels. The CLI measurement includes process startup; the FFI measurement reuses loaded libraries. Each FFI call still opens/parses/decodes the document rather than reusing a decoded frame.

- MPEG-4 video: command **54.25 ms**, warm FFI **1.65 ms** median.
- H.264 video: command **54.85 ms**, warm FFI **1.81 ms** median.
- VP9 video: command **55.10 ms**, warm FFI **1.71 ms** median.
- MP3 embedded cover image: command **53.08 ms**, warm FFI **1.80 ms** median.
- PDF first page: command **12.95 ms**, warm FFI **1.52 ms** median.

Loading the prototype's libraries took **34.81 ms** in this run (49.04 ms in the first run). The warm gains depend on reusing the isolated renderer. These figures exclude JPEG encoding, API transport, cache lookup, queueing and the rendered grid. They are not end-to-end application speedups, broad media-corpus results or measurements of a production-safe renderer. The prototype does not reproduce all production limits/cancellation/format handling.

## Current thumbnail worker overhead

A separate experiment used the production Rust library and Bun worker. The persistent path calls the same production renderer directly; it still launches FFmpeg/Poppler internally. The fresh path starts the existing Bun worker for each request. Twelve samples per mode after three warmups, alternating order; JPEG signatures verified.

- PNG: persistent **3.70 ms**, fresh **37.07 ms**.
- MPEG-4 video: persistent **131.32 ms**, fresh **157.89 ms**.
- MP3 cover art: persistent **57.72 ms**, fresh **73.78 ms**.
- WAV waveform: persistent **118.92 ms**, fresh **133.50 ms**.
- PDF: persistent **16.14 ms**, fresh **31.39 ms**.

An empty Bun process alone took **11.88 ms** median. These are separate pipeline observations, not additive timings or a clean attribution of all difference to process creation. The current waveform path starts one FFmpeg attempt for artwork and a second for the waveform when artwork is absent. Direct decoding could avoid that repeated initialization.

## Actual packaged CEF capability

The final packaged application (`3mi4ad5mvli0t`) was launched with its private authenticated Go API and inspected through the repository's native CEF/CDP profiling workflow. Fresh H.264 and VP9 fixtures were generated from the same red video. No thumbnail or media API responses were mocked.

- VP9 actually decoded to the expected red canvas pixel; fetch-to-JPEG plus two animation frames had a median **18.6 ms** over five samples.
- H.264 actually failed in all five attempts with MediaError code 4.
- `canPlayType` reported VP9, AV1 and MP3 as supported, and H.264, HEVC and AAC as unsupported. AV1, HEVC, AAC and MP3 decoding were not exercised by this probe.
- The owned native window and backend closed normally, exit code 0.

The CEF timing includes browser/API/paint stages and uses VP9, so it cannot be compared directly with the decoder-only H.264/FFI timings as a browser-versus-FFI speedup. PDF.js performance was not measured. The shipped `libcef.so` has no dynamic exports named `avcodec_*`, `avformat_*` or `FPDF_*`; Bun cannot directly bind those internal decoder APIs from this binary. A separate FFmpeg/Poppler/PDFium library build would still be needed for those FFI interfaces.

## Other core operations

Existing real-disk profiles from October 25 remain relevant:

- 10,000-file listing: **32.1 ms/op**; **68.1%** self CPU in syscalls and **74.3%** cumulative in metadata lookup. A Rust rewrite that makes the same calls would retain that cost.
- 10,000-file search: **6.8 ms/op**; visible search median **341.8 ms**, including a **300 ms debounce**. A native matcher is not the main visible delay at that scale. Profile also contains path-cleaning/allocation work that can be improved within Go if larger workloads justify it.
- Bun app-thread sampling: only **39 active 1 ms samples in approximately 14 seconds**. These samples exclude native CEF CPU and do not justify moving broad desktop orchestration into FFI.

ZIP compression is the next plausible CPU-heavy target to profile: the directory-download path currently selects DEFLATE for every regular file, including already-compressed media. No compression throughput, representative-corpus results or native replacement gain has been measured. File copying/remote transfers and SHA-256 should be profiled before replacement; filesystem/network throughput, buffering and duplicate I/O can matter more than the language. Keep checksum semantics, confinement, cancellation and atomic publication contracts intact.

Recommended order: reusable isolated thumbnail workers; direct FFmpeg decoding and Poppler/PDFium rendering into owned pixel buffers; then profile ZIP compression. These require a production design and end-to-end tests before replacing the current implementation.

References: [FFmpeg demux/decode example](https://www.ffmpeg.org/doxygen/trunk/demuxing__decoding_8c_source.html), [Poppler renderer API](https://poppler.freedesktop.org/api/cpp/classpoppler_1_1page__renderer.html), [PDFium embedding API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdfview.h).

Raw measurements: `overhead.json`, `direct-media.json`, `capabilities.json` and `capabilities-lifecycle.json` in `perf-results/native-media/`. The matching `.ts`, `.cpp` and `.py` files contain the replay steps. No CI ceiling was changed.
