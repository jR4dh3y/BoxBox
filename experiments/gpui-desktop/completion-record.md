# GPUI desktop work record

Branch: `hoplite/emporion-ca08fa2f`. Changes are uncommitted. Preserve inherited `desktop/native/` and `.pi/` work.

Exit condition: Electrobun behavior and visuals reproduced and verified at the native runtime boundary. **Not met.** See [surface-ledger.md](surface-ledger.md).

## Repairs

- Rendered wallpaper, transparent browser layers, managed image storage, async decode/generation guards, five fits, frost, tint, custom accent and perceptual wallpaper accent extraction.
- Rendered preview/properties/context menus and both split panes. Connected activation, visible-row thumbnails, checkboxes, operation actions and editable locations.
- Preserved renamed selection before copy/paste.
- Fixed settings input routing/focus. Reset confirms, removes favorites and preserves saved servers.
- Added context pin/unpin, remote share shortcut, provider-specific delete labels and disabled mutation controls.
- Added Linux folder watches and reference shortcuts for split, tabs, parent navigation, hidden files, connections and properties.
- Added the Niri XWayland workaround after native screenshots exposed stale Wayland content.

## Evidence

- Development `cargo check` passed.
- Full Rust suite passed 30 tests, including rendered local filesystem operations and external create/remove refresh.
- Same workflow passed against a freshly built Go server, including share create/revoke and permanent deletion.
- Final release build and 30-test suite passed after watcher, shortcut and tab-strip fixes. The remote workflow also passed on this source.
- Current release launched on Niri with default backend selection. `/tmp/boxbox-gpui-current.png` shows fixture wallpaper and thumbnails. `/tmp/boxbox-gpui-real-home.png` confirms the user's existing wallpaper renders over the real local home. Window 38 remains open. Existing settings were preserved.
- Native chooser, OS clipboard, full previews, transfer UX, read-only runtime and exact visuals remain incomplete or unverified.

## Next actions

Continue the open requirements in the parity ledger, starting with OS clipboard, Open With, transfer UX and exact wallpaper/theme roles. Do not call this a complete recreation based on compilation or the workflow.
