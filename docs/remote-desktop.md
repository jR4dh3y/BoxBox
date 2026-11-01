# Remote servers in BoxBox Desktop

The local desktop window can browse its own files and paired BoxBox servers in independent tabs and split panes. Add a server from **Servers**, then browse its mounts from **Locations**. A saved server's human name appears beside its mounts. Renaming a saved server preserves its identity; changing its URL requires adding a new server.

## Pair a desktop

1. Open the self-hosted server in a browser and sign in to the account that will authorize access.
2. In Settings → Connect, re-enter that account's password to show a pairing code.
3. In the local desktop window, open Servers → Add server and enter a name, the server's HTTPS URL, and the displayed code. A trusted HTTPS base path, such as `https://files.example.com/boxbox`, is supported.

Codes contain eight unambiguous uppercase characters, displayed as `XXXX-XXXX`. They expire after five minutes, are single-use, and disappear when the server restarts. Redemption has a global ten-attempt limit per pairing window and a global rate of one request per second with a burst of five. The account can close its pairing window and list or revoke connected clients in Settings. Pairing is unavailable in development authentication bypass mode.

The desktop exchanges the code for an opaque, revocable client credential valid for 30 days. The server account password never enters the desktop pairing request. This is a one-time pairing code followed by bearer authentication, not authenticator TOTP or mutual TLS. A disconnected or expired client can be paired again from Servers. Remove deletes the local saved entry even when the server is offline and attempts to revoke its upstream credential.

## Security and storage

The desktop's private loopback API requires its per-launch HttpOnly, SameSite=Strict cookie and validates Host and Origin. The renderer receives public saved-server metadata only: ID, name, URL and credential expiration. It never receives saved bearer credentials.

The Go gateway verifies upstream HTTPS certificates, rejects redirects, and uses only the saved bearer credential upstream. It does not forward the local cookie, local access token, Origin or forwarding headers. Allowed routes and methods cover files, streaming, search, jobs, share management and GET upload configuration; authentication, settings, desktop APIs, WebSockets and static pages are excluded. Upstream authentication failures become `502 REMOTE_AUTH_REQUIRED` rather than a local sign-out. Preview responses retain sandbox protection and downloads preserve Range support.

Desktop credentials are stored in `<DataDir>/desktop/servers.json` with mode `0600` inside a `0700` directory. Treat this file as a secret when making backups. Snapshots are saved atomically with durability checks; invalid or uncertain storage disables remote access while local native routes remain available. The self-hosted server stores only credential digests in `<DataDir>/connect/clients.json`, also under private directory/file permissions. Pairing codes are process-local and are not persisted.

Remote path references use `@remote/<saved-server UUID>/<upstream virtual path>`. The UUID is routing identity, not a remote filesystem mount name. The gateway removes the reference prefix and forwards the upstream virtual path unchanged. Local filesystem and native OS actions never receive these references. Tabs, caches and job references retain server identity, so equal mount names on different servers remain independent.

## Actions and refresh

Local locations retain OS open/Open With, terminal, native clipboard, local directory watches and freedesktop Trash. Remote locations use server previews, downloads, uploads, share links and confirmed permanent server deletion. Native actions are selected by the active path, including in split panes. Remote mount roots return directly to Locations when navigating up.

Remote directories refresh on window focus and periodically while visible; remote jobs and cross-source transfer jobs are polled because paired credentials cannot open the server WebSocket. Saved-server changes invalidate the remote catalog. Offline and pairing-required errors remain visible in Servers with retry/re-pair actions. Refresh confirms current server data rather than treating an old mount catalog as authoritative.

## Copy between sources

Dragging between local and remote locations, or between two saved servers, defaults to copy. Cross-source moves are rejected; a source is never deleted as part of these copies. Copies between paths on one source use that source's ordinary job API.

Cross-source bytes flow through the desktop Go backend in bounded 1 MiB chunks, never through renderer memory. Recursive copies are bounded by depth and entry limits. Before changing a remote destination, the desktop requires its upload configuration to advertise `createOnly: true`; there is no overwrite-prone fallback for older servers. Each file is SHA-256 verified before atomic create-only publication. Directories are claimed without replacement. An existing destination causes failure rather than replacement.

Cancellation aborts network/stream work and retains the source. An incomplete file is not published. Completed files and newly created directories can remain after cancellation or a later failure in a recursive copy; inspect and remove that partial destination before retrying. Local temporary upload chunks are discarded when the copy exits. Incomplete upstream upload sessions follow the server's normal expired-session cleanup. Jobs are in memory and do not resume after the desktop backend restarts.

## API reference

All paths below are relative to `/api/v1`.

- Account pairing: `GET`, `POST {password}` and `DELETE /connect/pairing`.
- Public code exchange: `POST /connect/redeem {code,clientName}` returns `{clientId,credential,expiresAt,username}`.
- Account clients: `GET /connect/clients` and `DELETE /connect/clients/{id}`. Paired self-revocation: `DELETE /connect/client`.
- Private desktop servers: `GET /desktop/servers`, `POST /desktop/servers {name,url,code,clientName}`, `PATCH /desktop/servers/{id} {name}`, `DELETE /desktop/servers/{id}` and `POST /desktop/servers/{id}/pair {code,clientName}`.
- Private forwarding: `/desktop/servers/{id}/api/v1/<allowed upstream route>`.
- Cross-source jobs: `POST /desktop/transfers {type:"copy",sourcePath,destPath}`, `GET /desktop/transfers`, and `GET`/`DELETE /desktop/transfers/{id}`. Creation returns HTTP 202 and an ordinary job with ID `transfer-<UUID>`; DELETE requests cancellation.
- Upload capability: `GET /system/upload` returns `{chunkSizeBytes,createOnly:true}`. File uploads use `/stream/upload/<virtual path>` with `X-Create-Only: true`, chunk metadata and a final `X-Checksum: sha256:<hex>`. An empty file uses one empty chunk. A directory uses `X-Upload-Type: directory` with `X-Create-Only: true`.

## Release order

This remote integration is a dependent release layer on top of the desktop improvements and grid/selection fixes. Review those layers separately, then apply the remote-only patch against the published parent-layer snapshot. Build and ship the combined desktop, embedded frontend and Go server together; an older self-hosted server can support browsing only if it implements pairing, and cross-source upload additionally requires the advertised create-only protocol. Do not apply the remote-only layer directly to the earlier base without its desktop/grid dependencies.

`BOXBOX_DESKTOP_URL=https://files.example.com` remains a separate sandboxed remote-only window mode. It opens the server login page without the local backend or local filesystem bridge; it is not the paired, mixed-location desktop workflow described above.
