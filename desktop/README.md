# BoxBox Desktop

Linux desktop client using Electrobun 2.0.4, Bun and bundled Chromium (CEF). It shares the Svelte frontend and Go API with the self-hosted edition.

From the repository root:

```sh
bun install --frozen-lockfile
bun run dev:desktop
```

Go, Bun, `xdg-open`, GIO, GTK 3, WebKitGTK 4.1, Ayatana AppIndicator and librsvg must be installed. Native file clipboard support uses `wl-clipboard` on Wayland or `xclip` on X11. Publishing interoperable cut/copy formats also needs Python 3 with PyGObject and GTK 4; when unavailable, copy can use URI-list fallback and cut reports the missing capability. Electrobun downloads its pinned native SDK, Bun runtime and CEF on the first build. Its Linux native wrapper still links WebKitGTK even when CEF is selected; the app explicitly requires and renders with CEF.

`dev:desktop` builds the shared UI and Go binary before opening the desktop window. Restart it after changing the UI or backend; it does not start Vite or Air. The packaged app includes the backend and does not require Go or Bun on the destination machine.

```sh
bun run build:desktop   # stable Linux bundle + release artifacts
bun run check:desktop  # prepare SDK + TypeScript
bun run test:desktop
bun run --cwd desktop scripts/integration.ts # real local desktop API checks, after build
bun run --cwd desktop scripts/smoke.ts # after a desktop build
bun run --cwd desktop scripts/native-smoke.ts # requires a graphical Linux session
```

The stable release wrapper is `desktop/build/stable-linux-x64/BoxBox/bin/launcher`; distributable files are in `desktop/artifacts/`. Generated SDKs and binaries are ignored by Git. Linux is the supported application target; macOS and Windows integration is not implemented.

The installed runtime checks for an existing BoxBox process before initializing CEF. Opening another folder forwards it to that process. Concurrent launches use a kernel-held guard, which is released if the launcher exits or crashes. Closing the last window stops the local backend and exits the app.

## Local files

The app launches one private Go child process, bound to an OS-assigned `127.0.0.1` port. A random per-launch token establishes an HttpOnly, SameSite=Strict cookie; all static, API and WebSocket requests require it. Foreign Host and Origin values are rejected. This mode is distinct from the unauthenticated `--dev` flag.

Root (`/`), home, and existing standard home folders are mounted with write access, subject to the user's normal OS permissions. No root privileges are requested. Settings and wallpapers persist under Electrobun's app-scoped user data directory; temporary launch configuration is removed on shutdown. Deployment `BOXBOX_*` and legacy `FM_*` environment overrides are excluded from the child process.

Activating a local file uses its OS default application through `xdg-open`. The backend resolves the virtual path through the existing mount confinement checks before launching it. The web version keeps its embedded previews.

## Remote BoxBox servers

```sh
BOXBOX_DESKTOP_URL=https://files.example.com bun run dev:desktop
```

This opens the remote BoxBox server's login page in a sandboxed CEF window without starting a local backend. Only HTTP(S) URLs without embedded credentials are accepted. The remote server receives no desktop API or local filesystem bridge. Remote file activation retains the server's web behavior; download-and-open is not implemented.

## Daily file operations

- List and grid views support checkboxes, Ctrl/Cmd selection, Shift ranges, and Ctrl/Cmd+A. Double-click opens files and folders by default; Settings can switch to single-click. Modifier clicks always select.
- Details shows the active pane's selection, image preview, location, size, modified time, permissions, and available actions. Multiple selections show combined file bytes, excluding folder contents.
- JPEG, PNG, GIF and WebP thumbnails load lazily, keep the full image visible, and fall back to file icons. Generation accepts sources up to 64 MiB and 32 megapixels, runs at most two renders, and caches up to 128 previews or 16 MiB. Video and PDF thumbnails are not generated.
- Create, rename and delete dialogs show progress and prevent duplicate submissions. Delete counts appear in the heading; failures retain the remaining items for retry.
- Tabs keep independent folders, history, sorting and selection; F3 opens a second pane. Only the active tab keeps its directory queries and live watches mounted.
- Drag items within one source onto a folder, the other pane, or a sidebar location to move them; hold Ctrl to copy. Cross-source drops default to copy; cross-source moves are rejected. Files dragged in from other apps use local paths. Drops use the same conflict checks and background jobs as paste.
- Local folders use native actions and refresh through directory watches (F5 also reloads). Remote folders offer server previews, upload, download, share links and confirmed permanent deletion; they refresh on focus and periodically. Native actions are gated by each pane's path.
- For local locations, the address bar accepts absolute Linux paths and `~/` paths. Local directory changes made by other programs invalidate the visible view automatically; disconnected watchers periodically revalidate until they reconnect.
- Delete moves local items into the freedesktop Trash. Trash, Removable drives and Servers open as pages in the pane, with back and forward history like folders. Trash lists deleted items and supports restore and confirmed permanent deletion. Shift+Delete is always confirmed. Restore and transfer publication never overwrite an existing destination.
- Undo reverses the last rename or move to Trash. It does not undo permanent deletion or completed transfers.
- Local copy/cut/paste uses the desktop file clipboard, including GNOME copied-files and URI-list formats. A cut clipboard is cleared only after all submitted move jobs are confirmed successful and the clipboard still matches that selection.
- Paste checks the destination, then asks whether to skip a conflict or keep both with a new name. Transfers remain background jobs with their existing progress and cancellation controls. No replace option is offered.
- Open With lists installed applications associated with the file's MIME type and launches the chosen application through GIO. Terminal opens the current folder using an installed terminal.
- Removable drives lists actual removable-volume state and exposes mount, unmount and eject through the user's GIO services and permissions. An unavailable service or busy volume is reported by the UI.
- Valid names containing `..` work. The desktop removes the server's 10,000-entry directory cap while retaining paginated rendering; self-hosted instances keep the default cap.

## Keyboard shortcuts

Press F1 or click the toolbar's ? button for the in-app reference. Shortcuts act on the active pane. Text fields keep their normal editing shortcuts, and dialogs block file actions.

| Keys | Action |
| --- | --- |
| Arrow keys, Home, End | Move focus and selection in list or grid |
| Shift + Arrow keys | Extend a selection |
| Ctrl/Cmd + Arrow keys | Move focus without changing selection |
| Space / Ctrl/Cmd + Space | Select / toggle the focused item |
| Ctrl/Cmd + A / Escape | Select all loaded items / clear selection |
| Enter / Alt + Enter | Open / show Details |
| Ctrl/Cmd + C, X, V | Copy, cut, paste |
| Ctrl/Cmd + Shift + N / F2 | New folder / rename |
| Delete / Shift + Delete | Trash / confirmed permanent delete |
| Ctrl/Cmd + F / L | Focus search / location |
| Alt + Left, Right, Up / F5 | Back, forward, parent folder / refresh |
| Ctrl/Cmd + H | Toggle hidden files |
| Ctrl/Cmd + T, W | New tab, close tab |
| Ctrl/Cmd + Tab / Ctrl/Cmd + Shift + Tab | Next / previous tab |
| F3 | Toggle split pane |
| Ctrl/Cmd + Z | Undo the last local rename or move to Trash |
| Ctrl/Cmd + Shift + O | Open Servers |

The shared web browser supports these file and navigation shortcuts too. Web deletion uses the server's delete flow. Browsers and host window managers can reserve some combinations before the app receives them.

## Install as a Linux file manager

After building the stable bundle:

```sh
bun run install:desktop
# Explicitly opt in to changing your default folder/file-URL handler:
bun run install:desktop --set-default
```

The installer installs the runtime bundle into the user's XDG data directory and installs a desktop entry and icon. It replaces only an install marked as managed by BoxBox. It never changes default handlers unless `--set-default` is supplied. Opening a folder or file URL from another application selects its local directory; `--select /path/to/file` reveals a file without opening it.

## Saved servers

Open Servers in the sidebar, or press Ctrl+Shift+O in the local window, to pair a named HTTPS BoxBox server using a single-use code from its Settings → Connect page. Browse its mounts alongside local files in Locations, including independent tabs and split panes. Saved credentials remain in the private Go backend; Servers supports rename, re-pair, retry and removal. Cross-source copies stream through that backend with checksum verification and no overwrite; cancellation retains sources and may leave completed files in a partial destination. See [Remote desktop integration](../docs/remote-desktop.md) for setup, security, API details, limitations and dependent release ordering.

## Remaining desktop gaps

Dragging files out to other apps is not available in the current Electrobun SDK. Archive creation/extraction, bulk rename, permission editing, remote download-and-open, full network-protocol mounting, and complete Nautilus/Dolphin parity are not implemented. Default desktop associations and removable-device actions depend on the host desktop services. Only Linux is verified.

BoxBox rejects clipboard publication of paths containing `%`, and rejects native clipboard imports whose literal percent escapes would resolve to a different file. This prevents a selection named `report%20.txt` from selecting a sibling named `report .txt`; full support for literal percent escapes requires changing the shared virtual-path contract.
