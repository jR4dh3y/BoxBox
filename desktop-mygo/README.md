# BoxBox MyGo webview prototype

This is an isolated no-CEF desktop host for BoxBox. It keeps the existing
Svelte frontend and authenticated Go desktop server, and uses MyGo's system
WebViewGTK window instead of Electrobun/CEF.

Requirements:

- Go 1.27.3+
- GTK 3 and WebKitGTK 4.1
- a built BoxBox server binary

From this directory:

```sh
cd ..
bun run build:single
cd desktop-mygo
go run . -server ../dist/boxbox
```

The prototype intentionally does not replace the Electrobun desktop target.
It is a compatibility and startup experiment; launcher integration, portal
chooser windows, deep-link handoff and thumbnail-worker packaging remain to be
ported after the WebKit path is measured.
