# BoxBox UI → GPUI component map

Every Svelte file in `frontend/src` (master `d1bc979`), mapped to the native
file manager. The app looks like BoxBox but behaves like a Linux file manager:
files open in their native apps, and the web-only surfaces are dropped.

Two GPUI lines split at `492ecc7` and both need folding in:

- **PR** — `origin/hoplite/emporion-ca08fa2f` (`cc284b3`, PR #226). Every file
  operation goes through the Go server; this line has most of the desktop UI.
- **Local** — local `hoplite/emporion-ca08fa2f` (`48c1fc7`). Local files are
  handled in Rust (`local/`), plus wallpaper, settings and shares.

Status: ✓ present · ◐ partial · ✗ missing · — not needed.
"Claimed" means the PR text says so and the code was not run.

## Shell and layout

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `routes/+layout.svelte` | shell, theme tokens, wallpaper layers | keep | ◐ `theme.rs` | ◐ `theme.rs`, `workspace/appearance.rs` |
| `routes/+page.svelte`, `routes/browse/+page.svelte` | entry, redirect | `main.rs` | ✓ | ✓ |
| `desktop/BrowseWorkspace.svelte` | tabs, split panes | keep | ✓ `ui/tabs.rs`, `workspace.rs` | ✓ same |
| `BrowseBrowser.svelte` | one pane: toolbar, view, drops, keys | keep | ✓ `ui/browser.rs` | ✓ `ui/browser.rs` |
| `Toolbar.svelte` | back/forward, location, search, actions | keep | ✓ `ui/toolbar.rs` | ✓ `ui/toolbar.rs` |
| `EditablePathBar.svelte` | breadcrumbs ↔ text location | keep | ✓ `location.rs`, `ui/text_input.rs` | ✓ `render_path_bar` with `TextField` |
| `PathEditor.svelte` | location suggestions | keep | ✗ | ✗ |
| `SearchBar.svelte` | search field | keep | ✓ `search.rs` | ✓ `TextField` per pane |
| `AddMenu.svelte` | **+** menu: new file, folder, upload | keep (upload → copy in) | ◐ context menu only | ✓ `ui/toolbar.rs` on `ContextMenu`; local folders say “Copy files here…” |
| `Sidebar.svelte` | drives, places, devices, Trash, favorites | keep | ✓ `ui/sidebar.rs`, `devices.rs` | ✓ `components/sidebar.rs` (`SidebarSection`, `NavItem`), Devices from UDisks2 |
| `StatusBar.svelte` | counts, selection size, view switch | keep | ✓ `ui/status_bar.rs` | ✓ `ui/status_bar.rs` |
| `LoadError.svelte` | folder load failure | keep | ◐ inline text | ◐ inline text |

## Files and drives

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `FileList.svelte` | list view | keep | ✓ `ui/file_list.rs` | ✓ `components/file_list.rs` (`FileListHeader`, `FileRow`) |
| `FileGrid.svelte` | grid with thumbnails | keep | ✓ `ui/file_grid.rs` | ✓ `components/file_grid.rs` (`FileTile`, `GridMetrics`) |
| `FileThumbnail.svelte` | unused on master | — | — | — |
| `DrivesView.svelte`, `DriveList.svelte`, `DriveRow.svelte`, `DriveCard.svelte` | This PC | keep | ✓ `ui/drives.rs` | ✓ `ui/drives.rs` |
| `DriveContextMenu.svelte` | drive menu | keep | ✓ `context_menu.rs` | ◐ `ui/overlays.rs` |
| `FileContextMenu.svelte`, `ui/ContextMenu.svelte` | file and empty-space menus | keep | ✓ `context_menu.rs` | ✓ `ui/overlays.rs` on `ContextMenu`, items as in `fileContextMenu.ts` |
| `BrowseDialogs.svelte` | create, rename, delete, properties | keep | ✓ `dialogs.rs` | ◐ create, rename, properties on `Modal`; delete asks with the system prompt |
| `ui/InlineRename.svelte` | rename in place | keep | ◐ dialog instead | ◐ dialog instead |
| `KeyboardShortcutsDialog.svelte` | F1 shortcuts | keep | ✓ claimed | ✗ |

## Previews: a quick look only; everything else opens in its app

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `FilePreview.svelte` | preview frame, ←/→ stepping | keep, read-only | ✓ claimed | ◐ `ui/overlays.rs` on `Modal`, read-only; no stepping |
| `preview/ImagePreview.svelte` | image with zoom | keep | ✓ claimed | ◐ no zoom |
| `preview/CodePreview.svelte` | text and code | keep, read-only | ✓ claimed | ✓ read-only text |
| `preview/PdfPreview.svelte` | PDF | first page only | ✓ claimed | ◐ thumbnail |
| `preview/VideoPreview.svelte`, `preview/AudioPreview.svelte` | playback | — native app | — | — |
| `preview/OfficePreview.svelte`, `preview/NotebookPreview.svelte` | documents | — native app | — | — |

Thumbnails come from `desktop/native` (linked as a crate) and should add the
freedesktop thumbnailers and shared cache (`~/.cache/thumbnails`).

## Desktop integration

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `desktop/TrashPage.svelte` | Trash, restore, delete forever | keep | ✓ `trash.rs` | ✓ `local/trash.rs`, `ui/browser.rs` |
| `desktop/TransferJobs.svelte` | transfer progress, cancel | keep | ✓ `jobs.rs`, `sse.rs` | ◐ coarse status |
| `desktop/TransferConflictDialog.svelte` | Skip or Keep both | keep | ✓ `clipboard.rs` | ✗ conflicts fail |
| `desktop/OpenWithDialog.svelte`, `desktop/ApplicationIcon.svelte` | Open With, set default | keep | ✓ `desktop.rs` | ✗ |
| `desktop/DesktopUndoShortcut.svelte` | Ctrl+Z | keep | ✓ `undo.rs` | ✗ |
| `desktop/FileChooser.svelte`, `desktop/ChooserLocations.svelte`, `routes/chooser/+page.svelte` | system file picker | keep (portal) | ✗ | ✗ |
| `Notifications.svelte`, `ui/Toast.svelte` | toasts | keep | ✓ `notifications.rs` | ✓ `workspace/notifications.rs`, `ui/notifications.rs` on `Toast` |

## Remote BoxBox servers: a later network location

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `desktop/ServersPage.svelte`, `desktop/RemoteCatalog*.svelte`, `desktop/DeploymentIcon.svelte` | saved servers | later | ✗ | ◐ connections in settings, `api/http.rs` |
| `ShareModal.svelte`, `ShareLinkList.svelte`, `settings/ShareLinksSettings.svelte` | share links (server only) | later | ✗ | ✓ `ui/settings.rs` |
| `settings/PairingCodeSettings.svelte` | pair a device | later | ✗ | ✗ |

## Settings

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `settings/SettingsModal.svelte`, `settings/SettingsRow.svelte` | settings frame | keep | ✓ `settings.rs` | ✓ `ui/settings.rs` on `Modal`, `SettingsTab`, `SettingsRow` |
| `settings/GeneralSettings.svelte` | browse preferences | keep | ✓ `settings.rs`, `preferences.rs` | ✓ `settings_state.rs` |
| `settings/AppearanceSettings.svelte` | accent, wallpaper, fit, frost, tint | keep | ✗ | ✓ `workspace/appearance.rs`, `workspace/palette.rs` |
| `settings/wallpaper/WallpaperPickerModal.svelte` | wallpaper picker | keep (native chooser) | ✗ | ◐ chooser untested |
| `settings/AccountSettings.svelte` | sign out | — web only | — | — |

## Primitives

| Svelte | Native scope | PR | Local |
|---|---|---|---|
| `ui/Input.svelte` | real text input with selection, IME and clipboard | ✓ `ui/text_input.rs` | ✓ `components/input.rs` frame + `components/text_field.rs` (from PR #226, scrolls to the caret, masks tokens), used by every field |
| `ui/Modal.svelte`, `ui/Button.svelte` | keep | ✓ `ui/modal.rs` | ◐ `components/modal.rs`, `components/button.rs`; dialogs not moved yet |
| `ui/Select.svelte`, `ui/Toggle.svelte` | keep | ✓ `segmented`, `toggle` | ✓ `components/select.rs` with an option menu (`ui/select_menu.rs`), `components/toggle.rs` |
| `ui/ProgressBar.svelte`, `ui/Spinner.svelte` | keep | ◐ in `jobs.rs` | ◐ `components/progress_bar.rs`; no spinner |
| `ui/ContextMenu.svelte`, `ui/Toast.svelte` | keep | ✓ | ✓ `components/context_menu.rs`, `components/toast.rs` |

## Shared components

`src/components/` holds one Rust component per Svelte component, with the same
names, props and states. Views call them and do not style controls
themselves. Colours come from `frontend/src/routes/layout.css` (`build.rs`
generates `theme::*`), and icons from the Lucide set `scripts/extract-icons.ts`
copies, so a token or icon change on the web side reaches the native app on
the next build.

`cargo run -- --gallery` renders `frontend/src/routes/dev/gallery/gallery.json`;
the web app renders the same manifest at `/dev/gallery` (dev server only).
`scripts/compare-gallery.py` diffs the two cell by cell.

| Component | Svelte | Gallery | Used by views |
|---|---|---|---|
| `Button` | `ui/Button.svelte` | ✓ 8 variants and sizes | ✓ name dialog |
| `Input`, `TextField` | `ui/Input.svelte` | ✓ | ✓ name dialog |
| `Toggle` | `ui/Toggle.svelte` | ✓ (disabled knob is an exception: GPUI has no group opacity) | ✓ settings |
| `Select` | `ui/Select.svelte` | ✓ | ✓ settings |
| `ProgressBar` | `ui/ProgressBar.svelte` | ✓ | not yet |
| `ContextMenu`, `MenuItem` | `ui/ContextMenu.svelte` | ✓ | ✓ file menu, **+** menu |
| `Modal` | `ui/Modal.svelte` | ✓ | ✓ dialogs, properties, preview |
| `Toast` | `ui/Toast.svelte` | ✓ | ✓ notices |
| `FileListHeader`, `FileRow` | `FileList.svelte` | full-window shot | ✓ `ui/file_list.rs` |
| `SidebarSection`, `NavItem` | `Sidebar.svelte` | full-window shot | ✓ `ui/sidebar.rs` |
| `FileTile`, `GridMetrics` | `FileGrid.svelte` | full-window shot | ✓ `ui/file_grid.rs` |
| `SettingsRow`, `SettingsTab` | `settings/SettingsRow.svelte`, `SettingsModal.svelte` | full-window shot (0.6% of the modal differs) | ✓ `ui/settings.rs` |

Next: tabs, toolbar, status bar and drive cards, then move
dialogs, menus, settings and notices onto the primitives above.

Known differences from the web app, kept on purpose: the home folder is
labelled "Home" as in other Linux file managers, and list rows show icons,
not thumbnails (thumbnails belong in the grid), and grid tiles show video and PDF
thumbnails as well as images.

## Web only: not part of the native app

`routes/login/+page.svelte`, `routes/s/[token]/+page.svelte` (public share
page), `UploadPanel.svelte` and `UploadProgress.svelte` (browser uploads;
copying in uses transfers), `AnimatedLogo.svelte`, `PicoLogo.svelte`.

## Native work with no Svelte counterpart

| Behaviour | PR | Local |
|---|---|---|
| Single instance, launch opens a tab | ✓ `instance.rs` | ✗ |
| Live folder refresh | ✓ `watch.rs` (server stream) | ✓ `workspace/watching.rs` (inotify) |
| Drag out | ◐ X11 only | ◐ X11 only |
| Drop in from other apps | ✓ `dnd.rs` | ◐ |
| OS file clipboard (GNOME copied-files, uri-list) | ✓ claimed, through the server | ✓ `os_clipboard.rs`: owns the Wayland clipboard via data-control (not GNOME); `xclip` on X11 |
| Removable volumes: mount, eject | ✓ `devices.rs` (server) | ✓ `devices.rs`: UDisks2 over D-Bus (`zbus`), live; GVfs mounts (MTP, network) and LUKS unlock not yet |
| Wayland rendering | ◐ untested | ◐ falls back to XWayland on Niri (stale frames); GPUI 0.2.2 panics if a new keyboard sends modifiers before its keymap (`wayland/client.rs:1267`), seen with a `wtype` virtual keyboard |
| Keyboard focus movement, type-ahead | ✗ | ◐ arrow selection |

## Joining the two lines

The Local line's Rust provider is the base. The PR line's UI modules sit on
the server API, so each needs rewiring onto the Rust provider: `dialogs.rs`,
`context_menu.rs`, `clipboard.rs` (conflicts), `jobs.rs` (progress becomes
in-process), `undo.rs`, `desktop.rs` (Open With), `devices.rs`,
`notifications.rs`, `instance.rs` and `ui/text_input.rs`.
