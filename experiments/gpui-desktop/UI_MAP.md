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
| `EditablePathBar.svelte` | breadcrumbs ↔ text location | keep | ✓ `location.rs`, `ui/text_input.rs` | ◐ `render_path_bar`, custom field |
| `PathEditor.svelte` | location suggestions | keep | ✗ | ✗ |
| `SearchBar.svelte` | search field | keep | ✓ `search.rs` | ✓ `search.rs` |
| `AddMenu.svelte` | **+** menu: new file, folder, upload | keep (upload → copy in) | ◐ context menu only | ◐ context menu only |
| `Sidebar.svelte` | drives, places, Trash, favorites | keep | ✓ `ui/sidebar.rs`, `devices.rs` | ✓ `ui/sidebar.rs`, `workspace/favorites.rs` |
| `StatusBar.svelte` | counts, selection size, view switch | keep | ✓ `ui/status_bar.rs` | ✓ `ui/status_bar.rs` |
| `LoadError.svelte` | folder load failure | keep | ◐ inline text | ◐ inline text |

## Files and drives

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `FileList.svelte` | list view | keep | ✓ `ui/file_list.rs` | ✓ `ui/file_list.rs` |
| `FileGrid.svelte` | grid with thumbnails | keep | ✓ `ui/file_grid.rs` | ✓ `ui/file_grid.rs` |
| `FileThumbnail.svelte` | unused on master | — | — | — |
| `DrivesView.svelte`, `DriveList.svelte`, `DriveRow.svelte`, `DriveCard.svelte` | This PC | keep | ✓ `ui/drives.rs` | ✓ `ui/drives.rs` |
| `DriveContextMenu.svelte` | drive menu | keep | ✓ `context_menu.rs` | ◐ `ui/overlays.rs` |
| `FileContextMenu.svelte`, `ui/ContextMenu.svelte` | file and empty-space menus | keep | ✓ `context_menu.rs` | ✓ `ui/overlays.rs` |
| `BrowseDialogs.svelte` | create, rename, delete, properties | keep | ✓ `dialogs.rs` | ✓ `ui/operations.rs`, `ui/overlays.rs` |
| `ui/InlineRename.svelte` | rename in place | keep | ◐ dialog instead | ◐ dialog instead |
| `KeyboardShortcutsDialog.svelte` | F1 shortcuts | keep | ✓ claimed | ✗ |

## Previews: a quick look only; everything else opens in its app

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `FilePreview.svelte` | preview frame, ←/→ stepping | keep, read-only | ✓ claimed | ◐ `ui/overlays.rs` |
| `preview/ImagePreview.svelte` | image with zoom | keep | ✓ claimed | ◐ no zoom |
| `preview/CodePreview.svelte` | text and code | keep, read-only | ✓ claimed | ◐ edits without IME |
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
| `Notifications.svelte`, `ui/Toast.svelte` | toasts | keep | ✓ `notifications.rs` | ◐ settings notices |

## Remote BoxBox servers: a later network location

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `desktop/ServersPage.svelte`, `desktop/RemoteCatalog*.svelte`, `desktop/DeploymentIcon.svelte` | saved servers | later | ✗ | ◐ connections in settings, `api/http.rs` |
| `ShareModal.svelte`, `ShareLinkList.svelte`, `settings/ShareLinksSettings.svelte` | share links (server only) | later | ✗ | ✓ `ui/settings.rs` |
| `settings/PairingCodeSettings.svelte` | pair a device | later | ✗ | ✗ |

## Settings

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `settings/SettingsModal.svelte`, `settings/SettingsRow.svelte` | settings frame | keep | ✓ `settings.rs` | ✓ `ui/settings.rs` |
| `settings/GeneralSettings.svelte` | browse preferences | keep | ✓ `settings.rs`, `preferences.rs` | ✓ `settings_state.rs` |
| `settings/AppearanceSettings.svelte` | accent, wallpaper, fit, frost, tint | keep | ✗ | ✓ `workspace/appearance.rs`, `workspace/palette.rs` |
| `settings/wallpaper/WallpaperPickerModal.svelte` | wallpaper picker | keep (native chooser) | ✗ | ◐ chooser untested |
| `settings/AccountSettings.svelte` | sign out | — web only | — | — |

## Primitives

| Svelte | Native scope | PR | Local |
|---|---|---|---|
| `ui/Input.svelte` | real text input with selection, IME and clipboard | ✓ `ui/text_input.rs` | ◐ custom key handling, no IME |
| `ui/Modal.svelte`, `ui/Button.svelte` | keep | ✓ `ui/modal.rs` | ✓ inline in `ui/settings.rs` |
| `ui/Select.svelte`, `ui/Toggle.svelte` | keep | ✓ `segmented`, `toggle` | ✓ `cycle_button`, `toggle` |
| `ui/ProgressBar.svelte`, `ui/Spinner.svelte` | keep | ◐ in `jobs.rs` | ◐ drive usage bar |

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
| OS file clipboard (GNOME copied-files, uri-list) | ✓ claimed, through the server | ✗ internal only |
| Removable volumes: mount, eject | ✓ `devices.rs` (server) | ✗ |
| Wayland rendering | ◐ untested | ◐ falls back to XWayland on Niri (stale frames) |
| Keyboard focus movement, type-ahead | ✗ | ◐ arrow selection |

## Joining the two lines

The Local line's Rust provider is the base. The PR line's UI modules sit on
the server API, so each needs rewiring onto the Rust provider: `dialogs.rs`,
`context_menu.rs`, `clipboard.rs` (conflicts), `jobs.rs` (progress becomes
in-process), `undo.rs`, `desktop.rs` (Open With), `devices.rs`,
`notifications.rs`, `instance.rs` and `ui/text_input.rs`.
