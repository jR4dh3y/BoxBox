# Linux desktop app

BoxBox Files is a native Linux file manager with the BoxBox look. It manages the files on your computer directly and browses paired BoxBox servers. It needs no Docker, browser engine or local server.

It replaced the Electrobun desktop app shipped in v0.3.2. Settings from that app are not carried over.

## Supported systems

Desktop builds target **Linux with glibc**, built on Ubuntu 24.04. The archive and the AUR package are x86_64; AppImages are x86_64 and ARM64. It runs as a native Wayland client (tested on Niri, Hyprland and sway) or on X11. Windows, macOS, 32-bit ARM and musl are not provided. The server remains available for Linux amd64 and arm64.

It draws with Vulkan. On Ubuntu 24.04 install:

```bash
sudo apt install libvulkan1 mesa-vulkan-drivers libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libfontconfig1 libfreetype6 libglib2.0-bin
```

Optional, for the features that use them:

- `udisks2`: disks and removable drives in the sidebar, with mount, unlock and eject.
- `gvfs` and `gvfs-backends`: phones (MTP) and network locations (`smb://`, `sftp://`, `ftp://`).
- `xclip`: the file clipboard on X11. Wayland needs nothing extra.
- `ffmpeg` and `poppler-utils`: video and PDF thumbnails.
- A Secret Service keyring, such as GNOME Keyring or KeePassXC: saved server credentials.

On a laptop with two GPUs it draws on the GPU the compositor uses. Set `ZED_DEVICE_ID` to a PCI device ID to pick one yourself.

## Download and install

Arch Linux users can install from the AUR:

```bash
paru -S boxbox-bin
```

Otherwise, open [Downloads](/download/) or the [GitHub release](https://github.com/jR4dh3y/BoxBox/releases) and download `boxbox-desktop-v<version>-linux-amd64.tar.gz` with `SHA256SUMS`. Verify, extract and install:

```bash
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf boxbox-desktop-v<version>-linux-amd64.tar.gz
cd boxbox-desktop-v<version>-linux-amd64
./install.sh
```

Run the installer **without sudo**. It puts `boxbox-files` in `~/.local/bin` and adds a desktop entry and icon under `~/.local/share`. It leaves your default file manager unchanged. To make BoxBox Files the default folder handler:

```bash
./install.sh --set-default
```

Pass `--set-picker` to use BoxBox Files for other apps' Open and Save dialogs through the xdg-desktop-portal FileChooser. `boxbox-files --disable-picker`, or **Settings → General**, undoes it.

To update, install a newer archive the same way. To remove it, run `./install.sh --uninstall`. Settings in `~/.config/boxbox-gpui` and saved state in `~/.local/state/boxbox-gpui` are kept; delete them yourself to start fresh.

### AppImage

Each release also has `boxbox-desktop-v<version>-x86_64.AppImage` and `boxbox-desktop-v<version>-aarch64.AppImage`, each with a `.sha256` file:

```sh
sha256sum --check boxbox-desktop-v<version>-x86_64.AppImage.sha256
chmod +x boxbox-desktop-v<version>-x86_64.AppImage
./boxbox-desktop-v<version>-x86_64.AppImage
```

If FUSE is unavailable, add `--appimage-extract-and-run`. AppImages do not install a desktop entry or change the default file manager. The host still needs the Vulkan and windowing libraries above.

## Local files and connected servers

- Browse drives, home folders, disks, phones and network shares. Type `smb://server/share`, `sftp://user@host/path` or `ftp://host` in the location bar (Ctrl+L) to mount and open one.
- Use tabs, split panes, list and grid views, search, favorites (shared with other file managers through the GTK bookmarks file) and the freedesktop Trash.
- Files open in their default app; **Open with** lists the installed apps that take the file. Drag files out of the window into other apps.

To connect a server:

1. Run a BoxBox server with the pairing API (v0.3.2 or newer). Reach any server beyond your own machine over HTTPS with a trusted certificate; the pairing code and credential travel with each request.
2. In the server's web UI, open **Settings → Account → Pair a desktop**, confirm your password and generate a single-use code.
3. In BoxBox Files, open **Settings → Connections** (Ctrl+Shift+O), enter the server's address and the code.

The credential is stored in your keyring through the Secret Service. Saved servers connect in the background at launch, and their drives join the sidebar and Overview. Removing a connection revokes it on the server and forgets it here; it does not touch files on the server.

## Build from source

Install Rust, Bun and the development packages for the libraries above (on Ubuntu: `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev`), then run:

```bash
bun run build:desktop
bun run install:desktop
```

See the [desktop README](https://github.com/jR4dh3y/BoxBox/blob/master/desktop/README.md) for development, profiling and tests, and [the paired-client API](connect-api.md) for the authorization contract.
