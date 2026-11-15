# BoxBox UI → GPUI component map

Every Svelte file in `frontend/src` (master `d1bc979`), mapped to the native
file manager. The app looks like BoxBox but behaves like a Linux file manager:
files open in their native apps, and the web-only surfaces are dropped.

The Electrobun rows below say what a feature must do, not how to build it.
Build each one as a native file manager would: through the system's own
interfaces (UDisks2, GVfs, Wayland, freedesktop specs), not by copying the Go
server or shelling out to tools such as `gio`.

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

- Tabs and status bar on shared components.
- Shared freedesktop thumbnails (`~/.cache/thumbnails`, with `fail/boxbox`
  records), and an icon when a thumbnail fails. Images decode in-process;
  videos and other types run the installed `.thumbnailer` entries
  (`local/thumbnailers.rs`, type from the shared-mime-info globs in
  `local/mime.rs`), three at a time with a 10 s limit. Not sandboxed with
  bwrap yet, as GNOME does.
- Location editor: suggestions (120 ms, folders only, hidden only when shown
  or typed `.`), Up/Down, Tab or Right to complete; `~` and `file://` input; a
  typed file path opens its folder and selects the file; click on the blank
  path bar, or F2 or Enter on it, opens the editor.
- "Show in folder" and launch arguments: select the item, accept several
  paths and `--select PATH`, reject bad input.
- System file picker through the xdg portal (`desktop_chooser_portal.go`):
  open, save, save-files, filters, choices, overwrite confirmation, and an
  explicit `--enable`/`--disable` portal route.
- Register as the default folder handler (`inode/directory`,
  `x-scheme-handler/file`), opt-in.
- Delete in Trash deletes: the Trash page has per-row buttons, no selection.
- After a cut is pasted, clear the OS clipboard if it still holds that cut.
- Clipboard on GNOME: data-control is not offered there, so copy and paste
  with other apps fail.
- Transfers: per-item progress, dismiss and clear finished jobs.
- Folder load errors through `LoadError` with Retry; a delayed spinner (150 ms)
  and a shared `Spinner`; an indeterminate `ProgressBar`.
- Modals close the top dialog on Escape and on a backdrop click, and restore
  focus; context menus close on Escape or an outside click and stay inside
  the window.
- Refuse Trash on network filesystems.
- Wallpaper: preview before applying, SVG and AVIF, test the picker.
- Collapse the sidebar into a drawer on narrow windows.
- Later: a Servers page with per-server status, and icons.

### Native file manager (from Thunar)

Most valuable first.

- Bulk rename for several items: replace (regex), case, insert, numbering,
  remove, date, with a preview that catches collisions.
- Properties: image dimensions, EXIF date and camera, MIME type,
  free space, a group picker; values selectable. (Folder size and counts, all dates, link target and the permissions grid are done.)
- Reorder tabs; restore tabs and split panes on launch.
- Details view: choose, resize and reorder columns (owner, permissions, MIME
  type, created, accessed), remembered per folder.
- Ask on a right-button drag: GPUI 0.2.2 starts drags only with the left
  button and keeps its drag state private, so Alt-drag asks instead.
- Middle-ellipsis for long names, keeping the extension.
- Undo for copy, move, create and link as well as Trash and rename; Redo.
- Transfer pause and resume, current file, rate and time left.
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
| `SearchBar.svelte` | search field | keep | ✓ `search.rs` | ✓ `TextField` per pane; matching, ranking and the walk are in `search.rs` and `local/search.rs` (see Search below) |
| `AddMenu.svelte` | **+** menu: new file, folder, upload | keep (upload → copy in) | ◐ context menu only | ✓ `ui/toolbar.rs` on `ContextMenu`; local folders say “Copy files here…” |
| `Sidebar.svelte` | drives, places, devices, Trash, favorites | keep | ✓ `ui/sidebar.rs`, `devices.rs` | ✓ `components/sidebar.rs` (`SidebarSection`, `NavItem`), one Drives section: Overview, the system drive, then disks, phones and shares from UDisks2 and GVfs (`ui/devices.rs`), then each paired server's drives with the server's name, the same in every tab; Home is a place; rows take drops |
| `StatusBar.svelte` | counts, selection size, view switch | keep | ✓ `ui/status_bar.rs` | ✓ `ui/status_bar.rs` |
| `LoadError.svelte` | folder load failure | keep | ◐ inline text | ◐ inline text |

## Files and drives

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `FileList.svelte` | list view | keep | ✓ `ui/file_list.rs` | ✓ `components/file_list.rs` (`FileListHeader`, `FileRow`) |
| `FileGrid.svelte` | grid with thumbnails | keep | ✓ `ui/file_grid.rs` | ✓ `components/file_grid.rs` (`FileTile`, `GridMetrics`); the name is always two lines tall, so sizes line up across a row (issue #186, still open on the web grid) |
| `FileThumbnail.svelte` | unused on master | — | — | — |
| `DrivesView.svelte`, `DriveList.svelte`, `DriveRow.svelte`, `DriveCard.svelte` | Overview | keep | ✓ `ui/drives.rs` | ✓ `components/drive_card.rs` (`DriveCard`, usage bar on `ProgressBar`); list rows still in `ui/drives.rs`. Cards and rows cover the system drive and every device (`workspace/overview.rs`); an unmounted or locked one mounts or unlocks on open |
| `DriveContextMenu.svelte` | drive menu | keep | ✓ `context_menu.rs` | ✓ `workspace/drive_names.rs`, `ui/place_menu.rs`: every drive (this computer's, a disk, phone or share, a server's) has Rename and, once renamed, Reset Name, from the Overview cards and table and the sidebar. The name shows on the Overview, in the sidebar, the path bar and device notices; the drive itself is not relabelled. Kept in `drive-names.json` by filesystem UUID (the encrypted one for LUKS, so it holds locked or open), GVfs location, or provider and drive name |
| `FileContextMenu.svelte`, `ui/ContextMenu.svelte` | file and empty-space menus | keep | ✓ `context_menu.rs` | ✓ `ui/overlays.rs` on `ContextMenu`, items as in `fileContextMenu.ts` |
| `BrowseDialogs.svelte` | create, rename, delete, properties | keep | ✓ `dialogs.rs` | ✓ create, rename, delete and properties on `Modal`; every confirmation uses `workspace/confirm.rs` |
| `ui/InlineRename.svelte` | rename in place | keep | ◐ dialog instead | ✓ `workspace/rename.rs`: the row's or tile's name becomes a field (`rename_box`); Enter or a click elsewhere renames, Escape cancels |
| `KeyboardShortcutsDialog.svelte` | F1 shortcuts | keep | ✓ claimed | ✓ `ui/shortcuts.rs`, listing the keys this app binds, including the movement keys of `workspace/keyboard.rs` |

## Previews: none; every file opens in its app

The in-app preview is gone (menu entry, Space, "Preview on single click"):
it showed text files and, for everything else, only a low-resolution
thumbnail, which the grid already shows. Files open in their native app.

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `FilePreview.svelte`, `preview/*.svelte` | in-app previews | — native app | ✓ claimed | — removed |

Thumbnails come from `desktop/native` (linked as a crate) through the shared
freedesktop cache (`~/.cache/thumbnails`, `local/thumbnails.rs`), on their own
threads (`thumbnail_queue.rs`). Installed freedesktop thumbnailers are still to add.

## Desktop integration

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `desktop/TrashPage.svelte` | Trash, restore, delete forever | keep | ✓ `trash.rs` | ✓ `local/trash.rs`, `ui/browser.rs` |
| `desktop/TransferJobs.svelte` | transfer progress, cancel | keep | ✓ `jobs.rs`, `sse.rs` | ◐ coarse status |
| `desktop/TransferConflictDialog.svelte` | Skip or Keep both | keep | ✓ `clipboard.rs` | ✓ `transfer.rs`, `workspace/transfers.rs`, `ui/transfer_question.rs`: a paste or drop runs on its own thread and asks as it goes, as Thunar does. A taken name: Skip, Rename (to `name (copy N)`, shown), Replace, or Merge for two folders, which asks again for names that meet inside; the dialog compares both sides' size and date. A failed item: Retry or Skip, and the rest go on. "Do this for the rest" remembers the answer for later items of the same kind. Before starting, too little free space asks to go on anyway. On FAT and exFAT drives, names lose `"*/:<>?\|`, control characters and trailing dots or spaces, inside folders too. A copy pasted into its own folder is named without asking; a move to another filesystem on the same provider copies, then removes the original. The ✕ or Escape cancels the transfer |
| `desktop/OpenWithDialog.svelte`, `desktop/ApplicationIcon.svelte` | Open With, set default | keep | ✓ `desktop.rs` | ✓ `open_with.rs`, `ui/open_with.rs`; icons from the desktop's icon theme and its parents (`icon_theme.rs`, theme name from the settings portal); "Use as default" beside Open |
| `desktop/DesktopUndoShortcut.svelte` | Ctrl+Z | keep | ✓ `undo.rs` | ✗ |
| `desktop/FileChooser.svelte`, `desktop/ChooserLocations.svelte`, `routes/chooser/+page.svelte` | system file picker | keep (portal) | ✗ | ✗ |
| `Notifications.svelte`, `ui/Toast.svelte` | toasts | keep | ✓ `notifications.rs` | ✓ `workspace/notifications.rs`, `ui/notifications.rs` on `Toast` |

## Remote BoxBox servers: a later network location

| Svelte | Role | Native scope | PR | Local |
|---|---|---|---|---|
| `desktop/ServersPage.svelte`, `desktop/RemoteCatalog*.svelte`, `desktop/DeploymentIcon.svelte` | saved servers | later | ✗ | ◐ connections in settings: a server pairs once by the code from its Settings → Account (`workspace/servers.rs`), the credential lives in the keyring over the Secret Service (`secrets.rs`), a refused one asks to pair again, and removing revokes it and forgets it. Saved servers connect in the background at launch; their drives join the Overview and the sidebar (`workspace/overview.rs`) and open in the same tab, which moves to that server (`pane::Place` history keeps each step's provider). Run against a local server and GNOME Keyring |
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
| `Modal` | `ui/Modal.svelte` | ✓ | ✓ dialogs, properties |
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
thumbnails as well as images, and files and folders use the desktop icon theme instead of Lucide glyphs.

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
| Drag out | ◐ X11 only | ✓ `drag_out/`: XDND on X11; on Wayland `drag_out/wayland.rs` starts a `wl_data_device` drag on GPUI's own connection once the pointer leaves the window (run on sway only; no drag icon outside the window); it takes a new pointer when the seat regains one |
| Drag inside the window | ✓ `dnd.rs` | ✓ `workspace/drag_drop.rs`: onto a folder row or tile, a pane or a sidebar entry; moves on one filesystem and copies to another, Ctrl copies, Shift moves, Ctrl+Shift links (by full path, under the item's name or a free one; local only) and Alt asks with a Move Here, Copy Here, Link Here, Cancel menu at the drop; resting 0.7 s on a folder row, tile, crumb or sidebar place opens it and the drag goes on; within 40 px of a list's or grid's top or bottom it scrolls, faster nearer the edge; an item refuses itself, its own inside and the folder it is in; the target under the pointer is highlighted |
| Drop in from other apps | ✓ `dnd.rs` | ✓ the same targets and highlight as a drag inside the window; files are copied, since GPUI does not report the action the other app offered |
| OS file clipboard (GNOME copied-files, uri-list) | ✓ claimed, through the server | ✓ `os_clipboard.rs`: owns the Wayland clipboard via data-control (not GNOME); `xclip` on X11 |
| Devices: mount, unlock, eject | ✓ `devices.rs` (server) | ✓ `devices/udisks.rs`: UDisks2 over D-Bus (`zbus`), live, with LUKS unlock and lock. `devices/gvfs.rs`: GVfs mounts and volume monitors over D-Bus; `smb://`, `sftp://`, `ftp://` and `mtp://` typed in the location bar mount and open, and a backend's password or host-key question opens a dialog. Rows show a usage bar, a read-only mark and a dimmed unmounted state. Run against a Samba share, an SSH login and a LUKS loop image; a phone mounted by location is merged with its MTP volume (one row, the volume's name) |
| Wayland rendering | ◐ untested | ✓ native on Niri, Hyprland and sway; `render_gpu.rs` draws on the compositor's GPU (`linux-dmabuf` main device) instead of the first Vulkan device. Run on a live Hyprland session and in nested niri; a Niri session on real hardware was not run. GPUI 0.2.2 still panics if a new keyboard sends modifiers before its keymap (`wayland/client.rs:1267`), and drops the first key of a newly added keyboard; both seen only with `wtype` virtual keyboards |
| Keyboard focus movement, type-ahead | ✗ | ✓ `workspace/keyboard.rs`: arrows (by row in the grid), Home, End, Page Up/Down; Shift extends, Ctrl moves the cursor alone, Ctrl+Space toggles; Menu and Shift+F10; type-ahead with a 1 s pause; the cursor scrolls into view |
| Read-only folders | — | ✓ checked with `access(2)` on load; New, Rename, Cut, Paste and Delete say "This location is read-only or unavailable" |
| History | — | ✓ `workspace/history.rs`: right-click Back or Forward for the places; places that no longer exist are skipped and dropped; the mouse's side buttons go back and forward in the pane under the pointer; Back and Forward return to the folder's selection, cursor and scroll position (`pane::ViewMemo`), and a newly opened folder starts at its top |
| Tabs from a folder | — | ✓ a middle-click on a folder row, tile or crumb opens it in a new tab; with Ctrl, behind the current one |
| Start location | — | ✓ Settings → General: the Overview, the home folder or the last location (kept in `$XDG_STATE_HOME/boxbox-gpui/last-location`); a remembered folder that is gone starts on the Overview |
| Per-folder view and sort | — | ✓ `workspace/folder_views.rs`: List or Grid picked in the status bar, and the sort picked from a column header, stick to that folder (by server and path, in `$XDG_STATE_HOME/boxbox-gpui/folder-views.json`, read before the first frame); other folders open with the defaults from Settings, a new default leaves folders that have their own, and picking the default forgets it. Search results do not change the folder's sort |
| Theme icons | — | ✓ `file_icons.rs`, `workspace/file_icons.rs`, `icon_theme.rs`: rows and tiles draw the desktop icon theme's icons, as Thunar does: `folder`, the home and XDG user folders' own (`user-home`, `folder-download`, …, by every virtual path that reaches them), and for files the MIME type from the name (shared-mime-info `icons` and `generic-icons`, then `text-x-generic`). Each theme folder is listed once into a name index; SVGs are drawn with resvg at device pixels in size buckets (16–256), PNGs only scaled down, all off the UI thread and kept per type and size. Grid icons stand without a frame (thumbnails keep theirs) and folder names keep the text colour; the Lucide glyph stays as the fallback. Settings → General → File icons picks System theme, BoxBox tinted or BoxBox grey. BoxBox's set (`format::Kind`) has a Lucide glyph per kind (script, config, package, disc image, font, key, database, diff, slides and the rest) and marks special folders with a small emblem inside the folder; tinted colours each group (code blue, images teal, video purple, audio orange, PDFs red, sheets green, slides rose, archives amber), folders take the accent, and every hue turns with the accent, also one taken from the wallpaper (`palette::follow_accent`, HCT). Not yet: following a theme change while running, `.desktop` files' own icons, a link emblem, drive icons from UDisks2 |
| Zoom | — | ✓ `workspace/zoom.rs`: Ctrl+plus, Ctrl+minus, Ctrl+wheel (capture phase, so the list does not scroll), Ctrl+0 resets; the list (4 steps) and the grid (5) keep their own step in the preferences, and replace the old Compact rows setting; large grid steps ask for 256 px thumbnails. Ctrl+wheel tested only with a simulated event |
| Rubber-band selection | — | ✓ `workspace/band.rs`, `ui/pointer.rs`: a drag from empty space selects every row or tile it touches (hit-tested by arithmetic on the uniform layout), Ctrl or Shift adds, past the edge it scrolls; a click on empty space clears the selection. Toasts and transfers occlude, so clicks on them stay theirs |
| Select by pattern | — | ✓ Ctrl+S: `*` and `?` match the whole name, plain text anywhere, case ignored (`search::matches_pattern`); Ctrl+I inverts the selection |
| Delete permanently | — | ✓ Shift+Delete, always after asking; Delete and Shift+Delete do nothing while the Trash page shows |
| Server drives at launch | — | ✓ each server's drives as last seen (`$XDG_CACHE_HOME/boxbox-gpui/server-drives.json`) stand in place with a pulsing usage placeholder (`components/skeleton.rs`) until the server answers; one opened meanwhile opens when it does |
| Breadcrumbs | — | ✓ `ui/toolbar.rs` `render_crumbs`: menu (Open in New Tab, Paste Into Folder, Properties), middle-click, drops, open after 0.7 s of dragging, "…" for crumbs that do not fit, full path on hover |
| Places | — | ✓ Recent (`recently-used.xbel`), Templates, Network (Avahi over D-Bus; untested here, as Avahi is disabled on this machine), Desktop; every sidebar entry has a menu (`ui/place_menu.rs`) |
| Favorites | — | ✓ local favorites are the GTK bookmarks file, shared with other file managers and watched; server favorites stay in BoxBox's file; drag to reorder |
| Trash | — | ✓ `workspace/trash.rs`: Empty Trash, a full or empty icon kept current by watching Trash, drops onto Trash, Restore offers Show and recreates folders deleted since; folders list first, then the most recently deleted |
| Templates | — | ✓ `workspace/templates.rs`: "New from Template" in the folder and + menus; the copy opens for renaming |
| Item menu | — | ✓ `workspace/item_actions.rs`: Open in New Tab, Compress (ZIP); no "Create Link" beside the item (links are made where they are dropped), and Open With, Open in Terminal and Properties each have their own icon; Send To (Desktop, removable drives, mail through the Email portal; mail untested, as it opens the mail app) |
| Running files | — | ✓ `launch.rs`, `workspace/run.rs`: executable scripts and programs ask first; launchers run when trusted (executable or installed) and ask otherwise; a `Terminal=true` launcher runs in the default terminal (`xdg-terminal-exec`, then `$TERMINAL`, then a common one) |
| Properties | — | ✓ `workspace/properties.rs`, `ui/properties.rs`: the name and icon in the title; rows with the label left and the value right, grouped: type, size, contents, location (its end kept when long), link target; modified, accessed, created; then owner, group and others each with a Read · Write · Run control (Open for folders) whose parts toggle one bit each; Run shows only for things that can run (already executable, a program, script, AppImage or launcher type, or a file starting with `#!` or ELF), so a video has none; "Apply to contents" for folders (files lose execute unless they were programs), and the mode as `ls` and octal show it. Changes chmod at once; for anyone but the owner the controls are dimmed and inert. For several items, counts and total size, read off the UI thread |

## Search

`search.rs` owns the rules and `local/search.rs` the walk. The field waits
300 ms after the last key and needs two characters; Escape clears it, in the
field or in the list. Every word of the query must be in the name, and a word
with `*` or `?` is a pattern for the whole name. Results come best first: an
exact name, then a name that starts with the text, then a word that does,
then any other place; nearer folders break ties. They keep that order until a
column header is clicked. Each result shows the folder it is in.

The local walk is breadth-first, shows its matches while it runs, stops when
the text changes, skips hidden folders unless hidden files are shown, does not
enter linked folders, and keeps the best 1000. A server is asked for the
longest word and its answer is matched and ranked the same way.

## Joining the two lines

The Local line's Rust provider is the base. The PR line's UI modules sit on
the server API, so each needs rewiring onto the Rust provider: `dialogs.rs`,
`context_menu.rs`, `clipboard.rs` (conflicts), `jobs.rs` (progress becomes
in-process), `undo.rs`, `desktop.rs` (Open With), `devices.rs`,
`notifications.rs`, `instance.rs` and `ui/text_input.rs`.
