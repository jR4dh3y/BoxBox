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

## Still to do

From a line-by-line audit of the Electrobun app (Go server, desktop host and
Svelte UI at master `d1bc979`) and of Thunar (`e3b357c`) against this branch.
Each report in `audit/` lists every behaviour with its source line and GPUI
evidence; `audit/BRIEF.md` is the method. Rows marked partial only because a
detail was "not verified" are not repeated here. Thunar rows are what a mature
Linux file manager does, not commitments.

### Parity with the Electrobun app

- Tabs and status bar on shared components. Status bar: selected count of
  total and selected size.
- Phone and network mounts (MTP, SMB) through GVfs, unlocking encrypted
  volumes, and device rows with capacity, read-only state and an unmounted
  look. Electrobun used `gio mount` (`desktop_volumes_gio.go`).
- Shared freedesktop thumbnails (`~/.cache/thumbnails`, installed
  thumbnailers), and an icon when a thumbnail fails.
- Location editor: suggestions (120 ms, folders only, hidden only when shown
  or typed `.`), Up/Down, Tab or Right to complete; `~` and `file://` input; a
  typed file path opens its folder and selects the file; click on the blank
  path bar, or F2 or Enter on it, opens the editor.
- Preview: previous/next, image zoom (25–500%, wheel) and rotate, Escape and
  backdrop close, a "Preview not available" notice for other types.
- "Show in folder" and launch arguments: select the item, accept several
  paths and `--select PATH`, reject bad input.
- System file picker through the xdg portal (`desktop_chooser_portal.go`):
  open, save, save-files, filters, choices, overwrite confirmation, and an
  explicit `--enable`/`--disable` portal route.
- Register as the default folder handler (`inode/directory`,
  `x-scheme-handler/file`), opt-in.
- Dragging inside the app: onto a folder or the other pane moves, Ctrl+Drag
  copies, a dragged item refuses its own drop; drop targets highlight.
- Dropping from other apps onto a folder row, tile or sidebar entry, not only
  the pane.
- Drive context menu: Rename and Reset Name, with stored drive names.
- Shift+Delete deletes permanently outside Trash; Delete in Trash deletes.
- Keys: Home, End, PageUp, PageDown, Ctrl+Arrow moves focus, Ctrl+Space
  toggles, Shift+Left/Right extends in the grid, Ctrl+Shift+click adds a
  range, the Menu key and Shift+F10 open the context menu, type-ahead. List
  them in F1.
- Right-click on empty space clears the selection before the folder menu.
- Alt+Enter on several items summarises count, folders and size.
- Properties: permissions and owner.
- Create dialog prefills `New folder` / `untitled.txt` and selects the
  basename; rename selects the basename; names are trimmed and `/` is refused
  with a message.
- Rename in place instead of a dialog.
- Delete confirmation names the item, or gives the count and list.
- Notices for a read-only location and for pasting with no files on the
  clipboard.
- After a cut is pasted, clear the OS clipboard if it still holds that cut.
- Clipboard on GNOME: data-control is not offered there, so copy and paste
  with other apps fail.
- Transfers: per-item progress, dismiss and clear finished jobs.
- Search: 300 ms debounce, two-character minimum, Escape clears.
- Keep the selection and scroll position across refresh and Back.
- Folder load errors through `LoadError` with Retry; a delayed spinner (150 ms)
  and a shared `Spinner`; an indeterminate `ProgressBar`.
- Modals close the top dialog on Escape and on a backdrop click, and restore
  focus; context menus close on Escape or an outside click and stay inside
  the window.
- At most four toasts at once.
- Restore from Trash recreates missing parent folders; Trash lists folders
  first, then newest; refuse Trash on network filesystems.
- Open With: app icons from the current icon theme, not only hicolor.
- Wallpaper: preview before applying, SVG and AVIF, test the picker.
- Collapse the sidebar into a drawer on narrow windows.
- Native Wayland on Niri (XWayland now shows stale frames).
- Later: Servers page and device pairing.

### Native file manager (from Thunar)

Most valuable first.

- Transfer conflicts: Replace, Rename, Merge folders, Apply to all; retry or
  skip a failed item instead of stopping the batch; warn when the target lacks
  space; names valid on FAT/exFAT targets.
- Empty Trash, a Trash full/empty icon kept current, drop onto Trash, and
  Restore that offers to open the restored item.
- Bulk rename for several items: replace (regex), case, insert, numbering,
  remove, date, with a preview that catches collisions.
- Permissions and owner editor in Properties, optionally recursive.
- Properties: folder size and counts (cancellable), image dimensions, EXIF date
  and camera, MIME type, link target, all dates, free space; values selectable.
- Mouse Back/Forward buttons; middle-click opens a folder in a new tab; Ctrl
  with it opens in the background.
- Open selected folders in new tabs; reorder tabs; restore tabs and split
  panes on launch.
- Breadcrumbs: context menu, middle-click, drop onto a crumb, hover to open
  while dragging, overflow for long paths, full name on hover.
- Back/Forward history menus; skip history entries that no longer exist.
- Places: Recent, Desktop, Templates, Network; read and write GTK bookmarks;
  reorder favorites by drag; sidebar context menu (Open in new tab, Properties,
  Mount, Eject, Empty Trash).
- New file from XDG Templates.
- Details view: choose, resize and reorder columns (owner, permissions, MIME
  type, created, accessed), remembered per folder.
- Per-folder view and sort; zoom (Ctrl+wheel, Ctrl+plus/minus) in grid and list.
- Select by pattern and invert selection; rubber-band selection.
- Hover a folder while dragging to open it; autoscroll at the edges; Copy,
  Move or Link on drop, with Ask on right-drag.
- Executable scripts and `.desktop` launchers: ask before running, honour the
  execute bit and trust.
- Middle-ellipsis for long names, keeping the extension.
- Undo for copy, move, create and link as well as Trash and rename; Redo.
- Transfer pause and resume, current file, rate and time left.
- Context-menu entries: Create link, Open in new tab, Send to (removable
  drive, mail), Compress several items into one archive.
- Custom actions: user commands filtered by file type, in the context menu.
- Folder tree in the sidebar.
- Accessibility: names and roles for screen readers.

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
| `DrivesView.svelte`, `DriveList.svelte`, `DriveRow.svelte`, `DriveCard.svelte` | This PC | keep | ✓ `ui/drives.rs` | ✓ `components/drive_card.rs` (`DriveCard`, usage bar on `ProgressBar`); list rows still in `ui/drives.rs` |
| `DriveContextMenu.svelte` | drive menu | keep | ✓ `context_menu.rs` | ◐ `ui/overlays.rs` |
| `FileContextMenu.svelte`, `ui/ContextMenu.svelte` | file and empty-space menus | keep | ✓ `context_menu.rs` | ✓ `ui/overlays.rs` on `ContextMenu`, items as in `fileContextMenu.ts` |
| `BrowseDialogs.svelte` | create, rename, delete, properties | keep | ✓ `dialogs.rs` | ✓ create, rename, delete and properties on `Modal`; every confirmation uses `workspace/confirm.rs` |
| `ui/InlineRename.svelte` | rename in place | keep | ◐ dialog instead | ◐ dialog instead |
| `KeyboardShortcutsDialog.svelte` | F1 shortcuts | keep | ✓ claimed | ✓ `ui/shortcuts.rs`, listing the keys this app binds |

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
| `DriveCard` | `DriveCard.svelte` | full-window shot (0.1% differs: label and live sizes) | ✓ `ui/drives.rs` |
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
| "Show in folder" from other apps (`org.freedesktop.FileManager1`) | ✗ | ✓ `filemanager1.rs`, only when no other file manager owns the name; opens the folder, does not select the item yet |
| Single instance, launch opens a tab | ✓ `instance.rs` | ✓ `instance.rs` (from PR #226); real paths and `file://` URIs open their folder |
| Live folder refresh | ✓ `watch.rs` (server stream) | ✓ `workspace/watching.rs` (inotify) |
| Drag out | ◐ X11 only | ✓ `drag_out/`: XDND on X11; on Wayland `drag_out/wayland.rs` starts a `wl_data_device` drag on GPUI's own connection once the pointer leaves the window (run on sway only; no drag icon outside the window) |
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
