# BoxBox on MyGo

The BoxBox desktop app on the system WebKitGTK instead of Electrobun/CEF. It
runs the same authenticated BoxBox server on loopback and shows the same
Svelte frontend; the host is Go.

Requirements: Go 1.27.3+, Rust 1.88+, Bun, GTK 3 and WebKitGTK 4.1.

```sh
desktop-mygo/build.sh                     # dist/desktop-mygo
dist/desktop-mygo/boxbox-mygo [folder | file] [--select file] [--remote https://…]
```

`build.sh` builds the server, the app, and the media thumbnail worker with
its Rust library (`desktop/native`). `SKIP_SERVER=1` reuses `dist/boxbox`.
Settings live in `~/.config/boxbox` unless `--data-dir` says otherwise.

## Host features

- **Launch targets:** a folder, or a file's folder with the file selected;
  `--remote` or `BOXBOX_DESKTOP_URL` opens a saved HTTPS deployment.
- **One instance:** a later launch hands its command line to the running app,
  which opens the target in a new tab of the main window.
- **System file chooser:** the server serves the FileChooser portal; the app
  opens a window per request and cancels a request whose window is closed.
  `--portal` starts without a window, for D-Bus activation;
  `--enable-chooser` / `--disable-chooser` route the desktop's chooser here.
- **Media thumbnails:** `boxbox-thumbnail` renders video, audio and PDF
  thumbnails through the Rust library.
- **Hardware acceleration:** Appearance → Hardware acceleration maps to
  WebKitGTK's DMA-BUF renderer and applies after a restart. Frosted glass
  needs it. Turn it off if pages show blank, as they can when the NVIDIA
  driver renders the desktop.
- **Drag and drop:** inside BoxBox, from other apps into a pane or onto a
  folder, and out to other apps, including GTK4 apps through the FileTransfer
  portal. WebKitGTK hides dropped paths from the page and sends a page's
  uri-list empty; `gtkdrag.go` fills both gaps from the GTK drag signals.

The host opens the server's loopback socket itself and passes it as
descriptor 3, so the window loads while the server starts.

`BOXBOX_DESKTOP_SMOKE=1` adds Electrobun's paint probe: the page requests
`/api/v1/_desktop-painted/<path>` once a folder's rows are on screen.

Not ported: Electrobun's release packaging (AppImage, AUR, installer).
