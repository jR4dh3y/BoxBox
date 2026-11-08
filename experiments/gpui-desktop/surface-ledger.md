# GPUI desktop parity ledger

Target: reproduce Electrobun appearance and behavior with native Rust local files and the Go remote service. The recreation is unfinished.

## Verified behavior

`src/workspace/parity_test.rs` drives rendered controls and checks actual filesystem bytes. `BOXBOX_PARITY_SERVER` runs the same workflow against a freshly built, isolated Go server. The workflow automatically isolates and restores `XDG_CONFIG_HOME`.

| Behavior | Local Rust | Remote HTTP | Evidence and limits |
|---|---|---|---|
| Create, navigate, rename | Workflow passed | Workflow passed | Keyboard input, rendered rows, actual paths |
| Text preview/save | Workflow passed | Workflow passed | Context menu, visible preview, saved bytes |
| Copy/paste | Workflow passed | Workflow passed | Renamed selection retained, destination bytes checked |
| Delete | Trash workflow passed | Permanent deletion passed | Actual prompt and file removal |
| Restore | Workflow passed | Does not apply | Remote service has no Trash contract |
| Pin/unpin folder | Workflow passed | Workflow passed | Context menu and favorites state |
| Share create/revoke | Does not apply | Workflow passed | Rendered controls and server share list |
| Split panes | Bounds checked | Bounds checked | Both panes drawn; cross-pane drag unverified |
| Wallpaper | Async install/remove passed | Same client-owned settings | Persistence, custom accent, fit/frost/tint controls; native chooser untested |
| Reset | Confirmation/cancel passed | Same client settings | Clears favorites and preserves connections |
| External changes | Linux inotify workflow passed | Missing | Actual external create/remove updates local listing |
| Browse/search/list/grid | Focused tests pass | Browse launched | Matched visual review and full grid interaction open |

Native image/video/audio/PDF thumbnail probes passed earlier. Thumbnail evidence does not establish playback or full document-preview parity.

## Remaining requirements

- Matched Electrobun/GPUI screenshot review at the same size, folder, settings and selection. Current appearance has not passed exact visual comparison.
- Full wallpaper tint/text/icon roles and viewport blur/saturation. Current tint is approximate.
- Native chooser, all five fits, invalid replacement, rapid replacement and restart restoration in the native window. Async persistence is tested without the chooser.
- OS file clipboard interoperability. Current clipboard is internal.
- Open With application picker.
- Transfers with real byte progress, cancellation, conflict decisions, partial-failure recovery and undo. Current status is coarse and conflicts fail the operation.
- Remote live updates, deployment pairing and the reference Connections protocols. Current connections store BoxBox URLs with session bearer tokens.
- Image zoom, media playback, PDF paging, office/notebook previews and full editor interaction. Media/PDF currently display static thumbnails; text editing lacks IME and mouse selection.
- Read-only runtime verification. Context menu disables mutation using drive metadata, while the server remains authoritative.
- Details, multi-selection properties, permanent local delete and complete keyboard/menu parity.
- Cross-pane/external drag/drop, cut across providers, recursive uploads, authentication failures, disconnected servers and share permission/expiry verification.
- Hardware acceleration switching. The appearance panel explicitly explains that GPUI requires GPU rendering and this build has no switch.

## Platform boundary

GPUI 0.2.2 retained stale frames on this Niri host's native Wayland backend. XWayland displayed wallpaper correctly. Startup prefers XWayland on Niri when DISPLAY exists; `BOXBOX_NATIVE_WAYLAND=1` opts into native Wayland. This is a workaround. Other compositors and platforms remain unverified.

The Go server remains the self-hosted backend. Its replacement and the Electrobun private cookie/session protocol are separate from the native-local provider. No commit, push or merge is authorized.
