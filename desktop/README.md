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

- Tabs keep independent folders, history, sorting and selection; F3 opens a second pane. Only the active tab keeps its directory queries and live watches mounted.
- The address bar accepts absolute Linux paths and `~/` paths. Local directory changes made by other programs invalidate the visible view automatically; disconnected watchers periodically revalidate until they reconnect.
- Delete moves local items into the freedesktop Trash. Trash lists deleted items and supports restore and confirmed permanent deletion. Shift+Delete is always confirmed. Restore and transfer publication never overwrite an existing destination.
- Undo reverses the last rename or move to Trash. It does not undo permanent deletion or completed transfers.
- Copy/cut/paste uses the desktop file clipboard, including GNOME copied-files and URI-list formats. A cut clipboard is cleared only after all submitted move jobs are confirmed successful and the clipboard still matches that selection.
- Paste checks the destination, then asks whether to skip a conflict or keep both with a new name. Transfers remain background jobs with their existing progress and cancellation controls. No replace option is offered.
- Open With lists installed applications associated with the file's MIME type and launches the chosen application through GIO. Terminal opens the current folder using an installed terminal.
- Drives lists actual removable-volume state and exposes mount, unmount and eject through the user's GIO services and permissions. An unavailable service or busy volume is reported by the UI.
- Valid names containing `..` work. The desktop removes the server's 10,000-entry directory cap while retaining paginated rendering; self-hosted instances keep the default cap.

## Install as a Linux file manager

After building the stable bundle:

```sh
bun run install:desktop
# Explicitly opt in to changing your default folder/file-URL handler:
bun run install:desktop --set-default
```

The installer installs the runtime bundle into the user's XDG data directory and installs a desktop entry and icon. It replaces only an install marked as managed by BoxBox. It never changes default handlers unless `--set-default` is supplied. Opening a folder or file URL from another application selects its local directory; `--select /path/to/file` reveals a file without opening it.

## Saved connections

Use Connections in the desktop toolbar or Ctrl+Shift+O to manage named remote BoxBox server profiles. Profiles persist in app-scoped user data, and each remote window is sandboxed without the local API bridge. The manager can reopen local files; closing one window does not close the others. URLs cannot contain embedded usernames or passwords. Avoid putting secrets in URL query strings.

## Remaining desktop gaps

Outbound native file drag is not available in the current Electrobun SDK. Archive creation/extraction, bulk rename, permission editing, remote download-and-open, full network-protocol mounting, and complete Nautilus/Dolphin parity are not implemented. Default desktop associations and removable-device actions depend on the host desktop services. Only Linux is verified.

BoxBox rejects clipboard publication of paths containing `%`, and rejects native clipboard imports whose literal percent escapes would resolve to a different file. This prevents a selection named `report%20.txt` from selecting a sibling named `report .txt`; full support for literal percent escapes requires changing the shared virtual-path contract.
