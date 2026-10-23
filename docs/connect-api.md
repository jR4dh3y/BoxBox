# Paired client API

BoxBox can authorize a client to use its file API without giving that client an account session. Use HTTPS with a trusted certificate when exposing these endpoints. Existing mount permissions and path confinement continue to apply.

This server API is the foundation for the separately reviewed desktop integration. The desktop management UI, local gateway, native file actions and cross-server transfer jobs are outside this change.

## Authorize a device

All routes below are relative to `/api/v1`. Account operations require an account access token in `Authorization: Bearer <access token>`.

1. Call `POST /connect/pairing` with `{ "password": "<account password>" }`. Password re-entry uses the account's existing login lockout. The response contains `code` and `expiresAt`.
2. Give that code to the intended device. The pairing window lasts five minutes and accepts one successful redemption. A new window replaces the same account's old code; another account cannot replace an active window. Restarting the server closes the window.
3. The device calls `POST /connect/redeem` with `{ "code": "<one-time code>", "clientName": "<device name>" }`. This endpoint needs no account token. The response contains `clientId`, `credential`, `expiresAt` and `username`.
4. The device stores the credential privately and sends it only in an `Authorization: Bearer <credential>` header. Credentials expire after 30 days. Do not put them in URLs, browser storage or logs.

`GET /connect/pairing` reports whether this account's window is open, with its expiry; it never returns the code. `DELETE /connect/pairing` closes that account's window. Pairing codes have eight unambiguous characters and display as `XXXX-XXXX`; redemption also accepts lowercase and the form without the hyphen. A global ten-attempt budget and a rate limiter of one request per second with a burst of five protect redemption. Opening another window does not reset the rate bucket.

## List and revoke

`GET /connect/clients` returns this account's active clients as `{ "clients": [...] }`, with their names, IDs and expiry times. It never returns credentials or credential digests. `DELETE /connect/clients/{id}` revokes an owned client. A client can revoke itself with `DELETE /connect/client` using its paired credential. Successful deletes return HTTP 204.

The server stores only SHA-256 credential digests in `<data_dir>/connect/clients.json`, with a private directory and file permissions. Changes use atomic publication and directory synchronization. Unsafe/corrupt storage fails closed. One server process must own the data directory, as with account session storage.

## Access boundary

Paired credentials can use the supported methods on `/files`, `/stream`, `/search`, `/jobs`, `/shares`, and `GET /system/upload`. Paired identity is bound to the account that enabled pairing. It cannot enable pairing, administer clients, sign in, refresh account tokens, edit settings, access desktop endpoints, or open a WebSocket. Poll job status instead. Stream/media requests also require the header; paired credentials are rejected in query parameters.

The account login and refresh protocol is unchanged. Development mode cannot authorize paired devices or accept their credentials. Pairing responses, including authentication failures, use `Cache-Control: no-store`. Errors retain the existing `{error,code}` API envelope. Invalid credentials return 401, unauthorized credential kinds return 403, unavailable pairing codes return 404, conflicting pairing ownership returns 409 and rate limiting returns 429.

## Create-only upload capability

`GET /system/upload` now includes `createOnly: true` alongside `chunkSizeBytes`. Clients must confirm that capability before relying on no-overwrite behavior with an older deployment.

File uploads use the existing chunk protocol at `/stream/upload/<virtual path>` with `X-Create-Only: true`. The mode is pinned when the session starts; changing it for an existing upload ID fails. The final chunk still requires `X-Checksum: sha256:<hex>`. After checksum verification, publication atomically fails if a destination already exists. Ordinary uploads retain their existing replacement behavior.

An empty file uses one empty chunk and its SHA-256 checksum. Creating a directory uses `POST /stream/upload/<virtual path>` with `X-Upload-Type: directory` and `X-Create-Only: true`; an existing directory is a conflict. Upload/status responses advertise `X-Create-Only: true`. Existing paths are resolved through the file service and revalidated before publication.
