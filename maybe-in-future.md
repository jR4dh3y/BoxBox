# Maybe in Future

Design notes for work we have deliberately not committed to yet. This file records
the conclusions from evaluating [tailscale/tailcat](https://github.com/tailscale/tailcat),
an S3 backend, and a plugin system for BoxBox, so the reasoning survives the
conversation that produced it. Nothing here is a promise.

## Media sharing: transport architecture

The share feature being built now is deliberately transport-agnostic at its core:
a share record (single file, permission bits, expiry, revocation) with an access
token. What serves the bytes is a per-share transport.

- **`http` transport (now):** BoxBox serves the share over its own HTTP surface.
  Works whenever the instance has any ingress — reverse proxy, Cloudflare tunnel,
  Tailscale/Funnel, or an exposed port. Zero new dependencies.
- **`tailcat` transport (future, optional):** the recipient connects to BoxBox over
  a WireGuard-encrypted peer-to-peer tunnel that BoxBox dials *out* to establish.
  Works with no ingress at all.

### Why tailcat is interesting

| Host deployment | HTTP share | tailcat share |
| --- | --- | --- |
| Tunnel / reverse proxy / exposed port | works | works; better for very large files |
| Tailscale network | works | works; redundant for tailnet members |
| LAN-only, no port, no tunnel, no VPN | LAN recipients only | works — this is its reason to exist |

Tailcat is Tailscale's data plane (magicsock, WireGuard, gVisor netstack) without
the control plane: one side runs a listener and gets a compact connection token
(ConnBlob, `tc…`), the other side connects with it. Bootstrap goes through a DERP
relay (outbound HTTPS), then NAT traversal upgrades to direct peer-to-peer UDP,
with DERP as relay-of-last-resort. The entire thing is userspace Go:

- No root, no TUN, no net-admin — compatible with the container's `cap_drop: ALL`,
  non-root UID, and `no-new-privileges` hardening.
- Outbound-only from the BoxBox side — works behind CGNAT and firewalls with no
  forwarded ports.
- No account, identity, or coordination server; possession of the token is access.
- Recipients use the `tailcat` CLI, or a browser: the WASM client (Tailscale hosts
  a public copy at tailscale.github.io/tailcat) connects browser-to-BoxBox
  directly over DERP, so a BoxBox instance with no ingress can still share to a
  recipient's browser.

### What it would cost

- **Toolchain and dependency weight:** requires Go 1.27 and pins a pre-release
  `tailscale.com` (plus wireguard-go and an unavoidable gVisor netstack). Largest
  dependency BoxBox would ever take; inflates the binary and adds real memory
  under the 512 MB compose limit (one netstack instance per active share).
- **No API stability promises:** the project is v0.1.x and explicitly warns that
  the Go API, CLI, and wire format may change. Any integration must pin or vendor
  the exact version and isolate it behind a BoxBox-owned interface.
- **Browsers cannot speak SFTP:** tailcat's built-in file service is SFTP; the
  browser WASM client is a raw byte stream. A BoxBox integration needs a small
  custom framing protocol in the `OnTCP` handler that serves exactly the one
  shared file — no directory listing, no HTTP semantics (no Range/video seeking).
- **Relay terms:** Tailscale's public DERP relays are free, rate-limited, and
  revocable at will. A shipped feature should make a self-hosted DERP map
  (WebSocket-capable `derper`) configurable, and embed the region in the ConnBlob
  so browser recipients need no extra fetch.
- **Security review:** this would be BoxBox's first outbound network dependency
  and an intentional exception to the "keep it behind your VPN" posture in
  docs/security.md. It must land opt-in, default-off, with its own audit-style
  review of the recipient-facing path, per-share ephemeral keys (revocation =
  listener close), and app-level expiry/byte caps layered on top.

### Implementation sketch (if we do it)

- Gate the import behind a build tag (e.g. `//go:build boxbox_tailcat`) in a
  dedicated package, so default builds never link the tailscale.com/gvisor tree.
- Per share: `tailcat.Server{Key: ephemeral, ServedTCPPorts: pinned to one port,
  OnTCP: framing handler}` → `Start()` → `ConnBlob()` → token embedded in the
  share URL. Revoke = `Close()`, which kills the ephemeral key and the token.
- Config: `sharing.tailcat.enabled` (default false), `derp_map_url`,
  max concurrent tailcat shares.

## Plugin / add-on system (staged)

Grow extension seams in order of cost and demand — do not build a speculative
plugin platform before anyone has written a plugin.

1. **Internal seams (with the share work):** replace the hand-wired handler
   construction in `cmd/server/main.go` with a small registry — a Go `Plugin`
   interface (`Name`, `Init`, `RegisterRoutes`, `RegisterSettings`, `Start`,
   `Stop`) and a `plugins:` config section. Sharing, drive names, and later
   tailcat become first-party plugins. Also fixes, once and centrally, the viper
   `BindEnv` gap that would otherwise break `BOXBOX_SHARING_*`-style overrides.
2. **External integration surface (cheap, immediately useful):** a documented,
   versioned REST API plus API tokens (separate from browser JWTs) and outbound
   webhooks on events (job finished, upload completed, share accessed, login
   failed). For a homelab file manager this is where community integrations
   actually start — n8n, Node-RED, Home Assistant, backup scripts — with no
   sandboxing risk.
3. **WASM plugins, only on demonstrated demand:** host via wazero (pure Go, no
   cgo, keeps `CGO_ENABLED=0` static builds). Manifest plus capability model:
   scoped route prefix (`/api/v1/plugins/<id>/…`), sandboxed storage under
   `/data/plugins/<id>`, event subscriptions, CPU/memory caps. Language-agnostic
   for plugin authors. Explicitly not Go's native `plugin` (.so) package —
   Linux-only, toolchain-pinned ABI, cross-compile-hostile, and arbitrary native
   code in a container that can modify host files.
4. **Frontend plugins last:** a plugin contributing its own route/page via its
   registered handler is achievable; runtime-loaded Svelte components fight the
   strict CSP posture and stay out of scope until there is a real need.

Licensing ground rules: the core stays MIT; first-party bundled plugins must be
MIT-compatible (tailcat is BSD-3-Clause, which qualifies); GPL-3 projects (see
below) can only ever be sidecar services, never linked code.

## Projects evaluated and set aside

- **fbs-core** (S3-compatible blob storage, GPL-3.0, self-hosted): well built —
  SigV4/bearer auth, signed public reads, sendfile streaming, an s3-tests
  compatibility suite — but the wrong tool for sharing. BoxBox's files live on
  bind mounts; fbs-core stores objects in its own layout, so using it for shares
  means either migrating storage to S3 (a rewrite of the service layer) or
  copy-then-share with sync-back for writable shares (two sources of truth), plus
  a second stateful service with its own database, bootstrap, and auth. GPL-3
  also rules out linking it into the MIT codebase. Possible future fit: an
  optional "publish to object storage" integration or S3-backed mount type, as a
  sidecar only.
- **tailcat:** see above — plausible future optional transport, kept out of the
  core for now.
