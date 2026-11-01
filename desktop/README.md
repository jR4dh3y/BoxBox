# BoxBox Desktop

Linux desktop client using Electrobun 2.0.4, Bun and bundled Chromium (CEF). It shares the Svelte frontend and Go API with the self-hosted edition.

For a published package, use [Downloads](https://boxbox.radhey.dev/download/) and the [desktop installation guide](../docs/desktop.md). The original desktop archive supports Linux x86_64; ARM64 server archives do not contain a desktop app. The separate AppImage workflow adds native x86_64 and ARM64 desktop packages.

From the repository root:

```sh
bun install --cwd frontend --frozen-lockfile
bun install --cwd desktop --frozen-lockfile
bun run dev:desktop
```

Go, Bun, Rust 1.88 or newer, `xdg-open`, GIO, GTK 3, WebKitGTK 4.1, Ayatana AppIndicator and librsvg must be installed. Native file clipboard support uses `wl-clipboard` on Wayland or `xclip` on X11. Publishing interoperable cut/copy formats also needs Python 3 with PyGObject and GTK 4; when unavailable, copy can use URI-list fallback and cut reports the missing capability. Install FFmpeg and Poppler (`ffmpeg` and `poppler-utils` on Debian/Ubuntu) for video/audio previews and PDF first pages. These optional media tools are discovered at runtime; missing tools leave the file icon. Electrobun downloads its pinned native SDK, Bun runtime and CEF on the first build. Its Linux native wrapper still links WebKitGTK even when CEF is selected; the app explicitly requires and renders with CEF.

Linux builds allow CEF to use GPU compositing and rasterization. BoxBox removes Electrobun's default `disable-gpu` and `disable-gpu-compositing` flags; those defaults force wallpaper blur and hover repaints onto the CPU. Chromium still decides which graphics features the driver supports.

Appearance → Hardware acceleration is available only in the desktop app. The choice is saved per app/channel and takes effect after fully closing and reopening BoxBox. The launcher applies it to the SDK startup configuration before CEF loads; changing modes requires a writable bundle directory, as created by the desktop installer.

Appearance → Tint components from wallpaper colours cards, menus, dialogs and controls using the sampled wallpaper hue. It is independent of the accent colour, preserves status colours, and is saved on this client. Removing the wallpaper restores neutral surfaces. Scrollbars follow the surface theme without replacing native scrolling.

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

The installed runtime checks for an existing BoxBox process before initializing CEF. Opening another folder or deployment forwards it to that process and opens a new tab, preserving existing tabs and split panes. Concurrent launches use a kernel-held guard, which is released if the launcher exits or crashes. Closing the last window stops the local backend and exits the app.

## Local files

The app launches one private Go child process, bound to an OS-assigned `127.0.0.1` port. A random per-launch token establishes an HttpOnly, SameSite=Strict cookie; all static, API and WebSocket requests require it. Foreign Host and Origin values are rejected. This mode is distinct from the unauthenticated `--dev` flag.

Root (`/`), home, and existing standard home folders are mounted with write access, subject to the user's normal OS permissions. No root privileges are requested. Settings and wallpapers persist under Electrobun's app-scoped user data directory; temporary launch configuration is removed on shutdown. Deployment `BOXBOX_*` and legacy `FM_*` environment overrides are excluded from the child process.

Activating a local file uses its OS default application through `xdg-open`. The backend resolves the virtual path through the existing mount confinement checks before launching it. The web version keeps its embedded previews.

## Remote BoxBox servers

```sh
BOXBOX_DESKTOP_URL=https://files.example.com bun run dev:desktop
```

This starts the bundled local app, saves or reuses the HTTPS deployment in its backend profile store, and opens that deployment's native remote location. Use the deployment base URL without credentials, a query or a fragment. New deployments remain unpaired until you enter a single-use code in Settings → Connections. Existing paired profiles retain their name, icon and device key across launches. The remote server receives only allowlisted API requests with the device key in an Authorization header; local cookies and filesystem APIs stay on loopback. Remote file activation retains the server previews; download-and-open is not implemented.

## Daily file operations

- List and grid views support checkboxes, Ctrl/Cmd selection, Shift ranges, and Ctrl/Cmd+A. Double-click opens files and folders by default; Settings can switch to single-click. Modifier clicks always select.
- Details shows the active pane's selection, image preview, location, size, modified time, permissions, and available actions. Multiple selections show combined file bytes, excluding folder contents.
- Image previews (JPEG, PNG, GIF, WebP, BMP, TIFF, ICO and QOI) decode and resize through a bundled Rust library called with Bun FFI. Video shows its first frame, audio shows embedded cover art or a waveform of its first 30 seconds, and PDF shows its first page. Previews load lazily and fall back to file icons on unsupported content or missing media tools. Image inputs remain limited to 64 MiB and 32 megapixels; video/PDF inputs stay seekable without copying the whole file. At most two isolated renderers run, each with an eight-second deadline, and the existing 128-entry/16 MiB cache remains. Cancellation stops the renderer and its child processes.
- Create, rename and delete dialogs show progress and prevent duplicate submissions. Delete counts appear in the heading; failures retain the remaining items for retry.
- Tabs keep independent folders, history, sorting and selection; F3 opens a second pane. Only the active tab keeps its directory queries and live watches mounted.
- Drag items within one source onto a folder, the other pane, or a sidebar location to move them; hold Ctrl to copy. Cross-source drops default to copy; cross-source moves are rejected. Files dragged in from other apps use local paths. Drops use the same conflict checks and background jobs as paste.
- Local folders use native actions and refresh through directory watches (F5 also reloads). Remote folders offer server previews, upload, download, share links and confirmed permanent deletion; they refresh on focus and periodically. Native actions are gated by each pane's path.
- For local locations, the address bar accepts absolute Linux paths and `~/` paths. Local directory changes made by other programs invalidate the visible view automatically; disconnected watchers periodically revalidate until they reconnect.
- Delete moves local items into the freedesktop Trash. Trash, Removable drives and Servers open as pages in the pane, with back and forward history like folders. Trash lists deleted items and supports restore and confirmed permanent deletion. Shift+Delete is always confirmed. Restore and transfer publication never overwrite an existing destination.
- Undo reverses the last rename or move to Trash. It does not undo permanent deletion or completed transfers.
- Local copy/cut/paste uses the desktop file clipboard, including GNOME copied-files and URI-list formats. A cut clipboard is cleared only after all submitted move jobs are confirmed successful and the clipboard still matches that selection.
- Paste checks the destination, then asks whether to skip a conflict or keep both with a new name. Transfers remain background jobs with their existing progress and cancellation controls. No replace option is offered.
- Open With lists recommended handlers and other installed applications that accept files, with their host icon and a generic fallback when unavailable. Icons use Python 3, PyGObject and GTK 3 to resolve the active icon theme and rasterize SVGs. Search covers both groups, including symlinked desktop entries and D-Bus handlers. Select "Use as default" before Open to set that application's association for the file's detected MIME type on this computer. Leaving it unchecked opens once. Association failures keep the dialog open; a saved default is retained if the subsequent launch fails. Launches pass the file as one argument to GIO. These actions apply only to local Linux files.
- New file views default to folders first, then files, with newest modified time first within both groups and natural filename ties. Browse, search, shared folders, wallpaper browsing, address suggestions and Trash follow this order, including visible hidden entries. Trash still displays its separate deletion date. Existing saved sorts and explicit URL/header choices are preserved. Drive/server/application catalogs and OS-owned file dialogs keep their own ordering. File creation time is not available in the current file metadata contract.
- Removable drives lists actual removable-volume state and exposes mount, unmount and eject through the user's GIO services and permissions. An unavailable service or busy volume is reported by the UI.
- Valid names containing `..` work. The desktop removes the server's 10,000-entry directory cap while rendering the complete listing through the shared UI; self-hosted instances keep the default cap.

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

Open Settings → Connections in the local window to pair a named HTTPS BoxBox server using a single-use code from the remote server's Settings → Account page. Browse its mounts alongside local files in Locations, including independent tabs and split panes. Saved credentials remain in the private Go backend; Connections supports rename, deployment icons, re-pair, retry and removal. Device keys expire after 30 days. The credential file uses mode 0600 inside a mode 0700 directory; browser JSON contains only the public profile and expiration. Cross-source copies stream through that backend with checksum verification and no overwrite; cancellation retains sources and may leave completed files in a partial destination. See [Remote desktop integration](../docs/remote-desktop.md) for setup, security, API details, limitations and dependent release ordering.

## Remaining desktop gaps

Dragging files out to other apps is not available in the current Electrobun SDK. Archive creation/extraction, bulk rename, permission editing, remote download-and-open, full network-protocol mounting, and complete Nautilus/Dolphin parity are not implemented. Default desktop associations and removable-device actions depend on the host desktop services. Only Linux is verified.

BoxBox rejects clipboard publication of paths containing `%`, and rejects native clipboard imports whose literal percent escapes would resolve to a different file. This prevents a selection named `report%20.txt` from selecting a sibling named `report .txt`; full support for literal percent escapes requires changing the shared virtual-path contract.

## Desktop-aware settings and actions

The local desktop omits Account settings; server web sessions retain account and device-pairing controls. Choose wallpaper opens the local file chooser directly, without a type filter because Electrobun 2.0.4's GTK chooser hides valid files once one is set. The app then rejects files that are not supported images and previews the selected image before saving it. Once preferences are saved, startup keeps their wallpaper and deletes images replaced or abandoned in earlier sessions. Cleanup is skipped if preferences are missing or unreadable; images staged during the current session remain available until restart. The web app retains its server-folder and device-upload choices.

Local files offer native opening and Trash actions rather than server Share or Download. Connected server files retain server actions. Read-only folders disable mutation actions in both list and grid menus; copy and inspection remain available.

### Live file-operation updates

The workspace opens one authenticated event stream for local jobs, one for desktop
transfers, and one shared stream for all paired servers. Remote upstream streams
are combined by the private backend so they cannot exhaust the browser connection pool. Each stream starts with a snapshot and then
pushes job changes. Reconnecting reconciles missed changes; closing a workspace or
removing a server closes its streams. Healthy streams do not poll job lists.

Paired-server streams use the desktop gateway, so credentials remain in the host.
Servers built before `/api/v1/jobs/events` are supported through an initial job
snapshot and checks of known active jobs only. Focusing the window retries those
servers. Upgrade the server for live discovery of operations started by other
clients. Directory and capacity reconciliation remains separate from job events.

Cross-source transfers reject percent characters in selected paths and directory-entry names before creating any destination. The shared virtual-path resolver decodes URL escapes, so accepting those literal names could read a different sibling. Rename the affected entry before copying.

## AppImage packaging

The separate [AppImage workflow](../.github/workflows/appimages.yml) builds and checks native x86_64 and ARM64 packages, then attaches both to the GitHub release. It also supports manual backfills by release tag. See [AppImage builds](../docs/desktop.md#appimage-builds) for launch commands, host dependencies and temporary disk requirements. This does not add ARM64 archive installation support.

## Use BoxBox as other applications' file chooser

Linux portal-aware applications can use BoxBox for Open, Save and folder dialogs.
This is separate from the default folder handler. Applications using direct GTK,
Qt or custom dialogs keep their own chooser unless configured to use portals.
For GTK applications, `GTK_USE_PORTAL=1` can request the portal path. Browser and
sandbox settings also affect whether their requests reach xdg-desktop-portal.

The backend implements only `org.freedesktop.impl.portal.FileChooser`. It supports
single and multiple files or folders, SaveFile, SaveFiles, file filters and the
current filter, suggested locations and names, accept labels, and extra choices.
The top bar searches file and folder names in the current folder. Ctrl+F focuses
search; Escape or Clear search restores the listing. Search respects hidden-file
and file-type filters and clears selections that no longer match. Navigating to
another folder clears the query. Other locations lists configured local roots and
discovered drives. Opening an unmounted drive uses the existing desktop mount
action, then browses the confirmed local path.

Paired server locations load independently in the background after the local
picker paints. Other locations shows each server's exposed roots, loading state,
errors and retry. Only locations exposed by that server are browsable; this does
not enable remote disk mounting or expose host paths outside its configured roots.
Opening remote files downloads private local copies before completing the portal
request. Cancel or a failed batch removes its partial copies. Successful copies
remain in `<data_dir>/desktop/chooser-imports/` for the receiving application;
remove them manually when no longer needed. Edits to these copies do not sync.
Remote save destinations and remote folder grants are unavailable because BoxBox
does not provide a writable local filesystem projection. Use a local folder for
those requests. Remote filenames containing percent characters follow the existing
cross-server transfer restriction.

Cancel, portal Request.Close and closing the chooser window cancel the request.
Saving returns a destination URI; BoxBox never writes the application's contents.
Existing destinations require confirmation. Local mounted paths only are returned;
remote BoxBox deployments cannot access the private chooser API.

Portal windows use the existing BoxBox file table and sorting. They are separate
windows; native transient parenting and compositor activation tokens are not yet
implemented, so focus and stacking depend on the window manager. Paths containing
invalid UTF-8 cannot be represented by the current BoxBox file API and are rejected.

### Opt in and undo

No installation or normal launch changes chooser defaults. The AUR package installs
backend metadata but does not select it. For a manually installed desktop bundle,
an administrator must first register the supplied metadata:

```sh
sudo install -Dm644 boxbox.portal /usr/share/xdg-desktop-portal/portals/boxbox.portal
```

Then use the **installed, writable** launcher:

```sh
~/.local/share/boxbox/bin/launcher --enable-chooser
# Undo only BoxBox's managed FileChooser override:
~/.local/share/boxbox/bin/launcher --disable-chooser
```

The enable command registers a per-user D-Bus activation service pointing to that
launcher. It copies the effective portal configuration into the user configuration
only when needed, preserves other interfaces and backend preferences, and changes
only the FileChooser key. Undo restores the previous key. It refuses modified
managed lines and symlinked configuration files rather than overwriting them.
If you later change other portal preferences in a system configuration, update the
user copy too. Re-run enable after moving the installed launcher. Remove its
`$XDG_DATA_HOME/dbus-1/services/org.freedesktop.impl.portal.desktop.boxbox.service`
when uninstalling. Disable before deleting the application.

Log out and back in after selecting or restoring a backend. Restarting the session
portal can interrupt other applications' active sharing and chooser requests, so
the command does not restart it automatically. GTK remains selected when BoxBox
has not been opted in, or after restoring its previous routing.
