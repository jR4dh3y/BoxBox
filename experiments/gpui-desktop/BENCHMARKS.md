# GPUI client vs. Electrobun app: performance

Measured on 2026-11-05 with `scripts/bench.py`, three runs per app. The table shows the median, with the individual runs in brackets.

## Setup

- **Apps:** the GPUI client at commit `1068ea6`, built with `cargo build --release`. The Electrobun app is the published `boxbox-desktop-v0.3.2-x86_64.AppImage` (Electrobun 2.0.4, CEF), extracted and run through its `AppRun`.
- **Same server for both:** both apps ran the Go server bundled in that AppImage (`Resources/app/boxbox-server`). The GPUI client used it through `BOXBOX_SERVER_BIN`. The numbers therefore compare the clients, and the server is reported separately.
- **Workload:** each app was launched directly into a folder of 10,050 entries (10,000 files and 50 folders), passed as an absolute path.
- **Machine:** a cloud sandbox with 2 vCPUs (AMD EPYC 9654) and 7 GiB RAM, running Xvfb with no GPU. GPUI rendered through Mesa lavapipe (software Vulkan). CEF rendered in software too. The Electrobun app's missing system libraries (WebKitGTK 4.1, libsoup 3, AppIndicator) came from Ubuntu 24.04 packages unpacked into a private directory.

## Results

| Metric | GPUI client | Electrobun app | Ratio |
| --- | --- | --- | --- |
| Window mapped after launch | **0.15 s** (0.15 / 0.15 / 0.15) | 0.83 s (1.10 / 0.82 / 0.83) | 5.4× faster |
| 10k-entry folder painted and settled | **1.03 s** (0.99 / 1.03 / 1.15) | 2.14 s (2.41 / 2.11 / 2.14) | 2.1× faster |
| Client memory (PSS, folder open) | **98 MiB** in 1 process | 577 MiB in 11 processes | 5.9× less |
| Go server memory (PSS, same binary) | 24 MiB | 24 MiB | same |
| Idle CPU over 10 s | **0.01 s** (0.01 / 0.02 / 0.01) | 0.09 s (0.09 / 0.09 / 0.10) | 9× less |
| CPU for 150 wheel-scroll steps | 1.31 s (1.35 / 1.29 / 1.31) | 1.28 s (1.27 / 1.31 / 1.28) | about the same, see caveats |
| Download size | 24 MiB client (stripped) + 23 MiB server | 196 MiB AppImage | about 4× smaller |
| Installed size | 47 MiB | 714 MiB extracted bundle | about 15× smaller |

## Caveats

- **What "settled" means:** the time until the window's pixels stopped changing for 1.5 s, measured from launch. It includes server startup, the folder listing and the first paint, and it is the same check for both apps. The two apps sorted differently: the GPUI client sorts newest first (PR #211), while the v0.3.2 release sorts by name.
- **First Electrobun run:** it was slower (2.4 s settled) because its runtime directory starts cold. The median uses all three runs.
- **Scrolling:** both apps rendered on the CPU, so the scroll-CPU figures mostly measure software rasterization (lavapipe or SwiftShader), not the UI frameworks. Repeat that row on a machine with a GPU before drawing conclusions. The wall time of the scroll test reflects the script's fixed event pacing, so it isn't reported.
- **Small machine:** these results come from a 2-vCPU sandbox with one display, so treat them as relative, not absolute.
- **Reproducing:** `GPUI_BIN=… SERVER_BIN=… ELECTROBUN_APPRUN=… scripts/bench.py gpui|electrobun 3 /path/to/folder`. The script needs `python-xlib` and Pillow.
