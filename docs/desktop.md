# Linux desktop app

BoxBox Desktop manages files on your Linux computer and on paired BoxBox servers. Local browsing works without Docker or a separately deployed server. The app bundles Chromium and a private local backend.

## Supported systems

The desktop download targets **Linux x86_64 with glibc**, built on Ubuntu 24.04. It is not a Windows, macOS, Linux ARM or Alpine/musl desktop package. The server remains available for Linux amd64 and arm64.

Install these runtime libraries on Ubuntu 24.04:

```bash
sudo apt install libgtk-3-0t64 libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2 xdg-utils
```

For clipboard integration on Wayland, install `wl-clipboard`; on X11, install `xclip`. Interoperable cut/copy also uses Python 3, PyGObject and GTK 4 (`python3-gi gir1.2-gtk-4.0`). Without them, file copy can fall back to URI lists, while cut reports the missing capability. FFmpeg and Poppler (`ffmpeg poppler-utils`) enable video/audio and PDF thumbnails; missing helpers leave file icons.

Other glibc distributions need equivalent packages. Only the published Linux x86_64 target is offered; distro compatibility depends on those runtime libraries.

## Download and install

Open [Downloads](/download/) and choose the **desktop** package. If the latest release has no desktop asset yet, the page links to the source-build instructions instead. The standalone server tarballs are different downloads.

For v0.3.2, download `boxbox-desktop-v0.3.2-linux-amd64.tar.gz` and `SHA256SUMS` from the same [GitHub release](https://github.com/jR4dh3y/BoxBox/releases). Verify before extracting:

```bash
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf boxbox-desktop-v0.3.2-linux-amd64.tar.gz
cd boxbox-desktop-v0.3.2-linux-amd64
./install.sh
```

Check that verification reports `OK` for the desktop archive. Run the installer **without sudo**. It uses the included runtime: Go and Bun do not need to be installed. Installation adds BoxBox to your application menu and registers folder/file URL handling, but leaves your default file manager unchanged. To opt in explicitly:

```bash
./install.sh --set-default
```

The app installs under `${XDG_DATA_HOME:-$HOME/.local/share}/boxbox`; preferences and server profiles live separately under the app's `io.boxbox.desktop/stable` data directory. Use a writable installation: changing hardware acceleration updates the runtime's startup configuration.

## Local files and connected servers

- Browse local drives and standard home folders. Files open in their OS default app; **Open with** lists installed file-capable applications.
- Use tabs, split panes, native clipboard operations and local Trash. Restore moves an item back; permanently deleting it is irreversible.
- Copy between local folders and paired servers, or between paired servers, with live transfer status. Cross-source moves are rejected; copy first and delete the source only after checking the result.
- Download and Share apply to server files, not files on this computer. Read-only locations keep mutation actions disabled.

To connect a server:

1. Run a BoxBox server with the pairing API: v0.3.2 or newer, or a recent nightly containing it. Older public releases do not provide pairing.
2. Open the server in your browser over HTTPS with a trusted certificate. In **Settings → Account → Pair a desktop**, confirm your password and generate a single-use code.
3. In the desktop app, open **Settings → Connections → Add server**, enter its HTTPS address and code, and pair it.

Remote credentials stay in the local backend. The server can revoke the paired device from Account settings. An expired or revoked connection needs pairing again; an offline server can be retried. Removing a desktop connection removes its saved local profile, not files on that server.

## Appearance and rendering

Appearance includes wallpaper fit, frosted glass, custom or sampled accents, and optional component tint from your wallpaper. Tint does not change the blue/orange/red drive-usage status colours. The desktop-only **Hardware acceleration** setting takes effect after fully closing and reopening the app.

## Update, remove or build from source

Close all BoxBox windows before updating. Extract the new release and run its `./install.sh` again. The installer replaces only an app directory marked as its own; preferences, paired servers and wallpapers remain in the separate app data directory. This release uses manual updates, not an automatic update feed.

To remove the app, delete the managed `boxbox` installation directory, its `applications/io.boxbox.desktop` desktop entry, and `icons/hicolor/scalable/apps/io.boxbox.desktop.svg` under your XDG data home. Leave `io.boxbox.desktop/stable` in place to retain settings and paired-server data. If you made BoxBox the default file manager, choose a replacement in your desktop settings before removing it.

To build instead of downloading, install the prerequisites in the [desktop README](https://github.com/jR4dh3y/BoxBox/blob/master/desktop/README.md), then run:

```bash
bun install --cwd frontend --frozen-lockfile
bun install --cwd desktop --frozen-lockfile
bun run build:desktop
bun run install:desktop
```

Source builds need Go, Bun and Rust in addition to the native build libraries. Use [Development](/docs/development/) for development commands and [the paired-client API](/docs/connect-api/) for the authorization contract.
