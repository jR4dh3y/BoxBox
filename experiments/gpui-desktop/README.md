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

Not implemented yet: the search input (it is drawn but inactive), thumbnails (tiles show the file-type icon, like the web UI while loading), file operations, context menus, previews, uploads, settings, trash, split panes, wallpapers and accent colours, and the desktop app's private `--desktop` cookie session.

## Screenshots

| List | Grid |
| --- | --- |
| ![List view](screenshots/list.png) | ![Grid view](screenshots/grid.png) |
| ![Folder opened](screenshots/navigate.png) | ![This PC](screenshots/this-pc.png) |

`screenshots/web-grid-reference.png` is the web app's grid view for comparison.

## Run

Start a BoxBox server without authentication. `--dev` binds to 127.0.0.1.

```sh
cd backend && go run ./cmd/server --dev -config /path/to/config.yaml
```

Then run the experiment. The default server is `BOXBOX_URL=http://127.0.0.1:8080`. `BOXBOX_TOKEN` sends a bearer token, and an optional argument opens a path such as `media/movies`.

```sh
cd experiments/gpui-desktop
cargo run --release
```

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
