# BoxBox GPUI desktop (experiment)

A native Rust rebuild of the BoxBox desktop browse UI with [GPUI](https://gpui.rs) 0.2.2, Zed's GPU UI framework. It reproduces the Svelte desktop/web layout and talks to a running BoxBox server over the same `/api/v1` endpoints. It does not replace `desktop/`.

## What it reproduces

| Svelte source | GPUI module |
| --- | --- |
| `routes/layout.css` tokens | `src/theme.rs` |
| `Sidebar.svelte` (Drives, Places, Trash, Favorites) | `src/ui/sidebar.rs` |
| `desktop/BrowseWorkspace.svelte` tab strip | `src/ui/tabs.rs` |
| `Toolbar.svelte`, `EditablePathBar.svelte`, compact `SearchBar.svelte` | `src/ui/toolbar.rs` |
| `FileList.svelte` | `src/ui/file_list.rs` |
| `FileGrid.svelte` | `src/ui/file_grid.rs` |
| `DriveCard.svelte`, `DriveList.svelte` | `src/ui/drives.rs` |
| `StatusBar.svelte` | `src/ui/status_bar.rs` |
| `utils/format.ts`, `utils/fileTypes.ts`, `browse/sortFiles.ts` | `src/format.rs`, `src/sort.rs` |

Icons are the frontend's own Lucide glyphs, extracted from `frontend/node_modules/lucide-svelte` into `assets/icons/` by `scripts/extract-icons.ts`. Fonts resolve through fontconfig, as the browser resolves the CSS `sans-serif` and `monospace` stacks.

Working behaviour: drive and folder navigation, back/forward history, breadcrumbs, tabs (open, close, switch), list/grid views, sortable columns, click/Ctrl/Shift selection with the status-bar summary, and refresh. Shortcuts: Alt+Left or Backspace (back), Alt+Right (forward), F5 or Ctrl+R (refresh), Ctrl+T or Ctrl+W (open or close a tab), Ctrl+A (select all), Esc (clear the selection).

### Dragging files to other apps

Drag a row or tile out of the window to drop it into another app, such as a file manager, browser, or editor. Dragging a selected item carries the whole selection. GPUI 0.2 can only receive drops from other apps, so `src/drag_out/` implements the X11 drag-and-drop protocol (XDND v5) itself, offering the files as `text/uri-list` with the copy action.

The other app needs local files, so the dragged items are downloaded into `$TMPDIR/boxbox-gpui-drag/` once the pointer reaches another app's window. Folders arrive as the server's ZIP archive, the same as a folder download in the web UI. Large files are only handed over after they finish downloading. The staged copies are not deleted automatically.

This only works on X11. Under a Wayland session GPUI uses its Wayland backend, and dragging out is not available there. Start the app with `WAYLAND_DISPLAY=` to run it through XWayland instead. macOS and Windows are not implemented. The existing Electrobun app cannot drag files out either.

Not implemented yet: the search input (it is drawn but inactive), thumbnails (tiles show the file-type icon, like the web UI while loading), file operations, context menus, previews, uploads, settings, trash, split panes, wallpapers and accent colours, and the desktop app's private `--desktop` cookie session.

## Screenshots

| List | Grid |
| --- | --- |
| ![List view](screenshots/list.png) | ![Grid view](screenshots/grid.png) |
| ![Folder opened](screenshots/navigate.png) | ![This PC](screenshots/this-pc.png) |

`screenshots/web-grid-reference.png` is the web app's grid view for comparison.

## Run

Like the Electrobun app, the client starts its own private BoxBox server. It writes a fresh config that mounts `/`, your home folder, and whichever standard home folders exist (Desktop, Downloads, Documents, Music, Pictures, Videos). It then launches `boxbox --desktop` with a random 64-character token and waits for the server to report its loopback address. Requests carry the token as the `boxbox_desktop` cookie. The server keeps its data in `$XDG_DATA_HOME/boxbox-gpui` and stops when the last window closes.

The server binary is looked up next to the executable. `BOXBOX_SERVER_BIN` overrides that path during development:

```sh
go build -C backend -o ../dist/boxbox ./cmd/server
cd experiments/gpui-desktop
BOXBOX_SERVER_BIN=../../dist/boxbox cargo run --release
```

To attach to a server that is already running, set `BOXBOX_URL` instead, and `BOXBOX_TOKEN` if that server needs a bearer token. An optional argument opens a path such as `media/movies`.

On Linux, GPUI needs Vulkan and the development packages for xkbcommon, wayland, X11/xcb, fontconfig and freetype. On Debian/Ubuntu, install them with:

`sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan1 libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev`

## Tests

```sh
cargo test
```

These test the ported formatting, sorting, selection and history logic.

## Regenerating icons

```sh
bun install --cwd frontend
bun run experiments/gpui-desktop/scripts/extract-icons.ts
```

Add the icon name to `ICONS` in the script and to the `icons!` list in `src/assets.rs`.
