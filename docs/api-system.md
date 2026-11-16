# Jobs, Settings and WebSocket

Background jobs, server settings and the live event stream. All routes need an access token; see [API reference](/docs/api/).

## Jobs

Background jobs handle copy, move, and delete operations.

```http
GET /api/v1/jobs
GET /api/v1/jobs/{id}
DELETE /api/v1/jobs/{id}
```

Create a job:

```http
POST /api/v1/jobs
Content-Type: application/json

{
  "type": "copy",
  "sourcePath": "home/movie.mkv",
  "destPath": "backups/movie.mkv"
}
```

Valid job types are `copy`, `move`, and `delete`. Copy and move require `destPath`.

Create response uses `202 Accepted`:

```json
{
  "id": "job_abc123",
  "type": "copy",
  "state": "pending",
  "progress": 0,
  "sourcePath": "home/movie.mkv",
  "destPath": "backups/movie.mkv",
  "createdAt": "2026-05-13T09:00:00Z"
}
```

## System and Settings

System drives:

```http
GET /api/v1/system/drives
```

Upload settings:

```http
GET /api/v1/system/upload
```

```json
{
  "chunkSizeBytes": 10485760
}
```

Custom drive names:

```http
GET /api/v1/settings/drive-names
PUT /api/v1/settings/drive-names
DELETE /api/v1/settings/drive-names/{mountPoint}
```

Set a drive name:

```json
{
  "mountPoint": "home",
  "customName": "Main Home"
}
```

### Share-page wallpaper

Wallpaper settings endpoints require a JWT and operate on the authenticated user's wallpaper:

```http
GET /api/v1/settings/wallpaper
PUT /api/v1/settings/wallpaper?mode=cover&frostedGlass=false&source=client-reference
PATCH /api/v1/settings/wallpaper
DELETE /api/v1/settings/wallpaper
```

`PUT` accepts a raw JPEG, PNG, GIF, WebP, AVIF, or SVG image up to 20 MiB. `source` is an optional client reference of at most 2,048 bytes. Display modes are `cover`, `contain`, `stretch`, `center`, and `tile`.

`GET`, `PUT`, and `PATCH` return `mode`, `frostedGlass`, `source`, `sha256`, `contentType`, and `updatedAt`. `GET` and `PATCH` return `404` when no wallpaper exists. `PATCH` accepts `{"mode": "contain", "frostedGlass": true}`. `DELETE` succeeds even when the wallpaper is already absent.

The browser syncs its wallpaper after login and explicit settings saves. An empty browser does not remove a stored wallpaper on login. Logout and session expiry cancel pending synchronization.

## WebSocket

Connect with a token in the query string or `Authorization` header:

```text
ws://localhost:8080/api/v1/ws?token=eyJhbGciOiJIUzI1NiIs...
```

Client messages:

```json
{ "type": "subscribe", "jobId": "job_abc123" }
```

```json
{ "type": "unsubscribe", "jobId": "job_abc123" }
```

```json
{ "type": "ping" }
```

Server messages:

```json
{
  "type": "job_update",
  "payload": {
    "jobId": "job_abc123",
    "state": "running",
    "progress": 45
  }
}
```

Terminal jobs use `job_complete`. Errors use `error`; ping replies use `pong`.

WebSocket authentication is checked at handshake. Logout does not close a connection that was already established.


## Private desktop file chooser

The authenticated loopback-only desktop server exposes `GET /api/v1/desktop/chooser`
for active portal requests, `GET /api/v1/desktop/chooser/{id}` for picker options,
`GET /api/v1/desktop/chooser/{id}/directory?path=home` for mount-confined listings
with content-detected unknown MIME types when filters need them, and
`POST /api/v1/desktop/chooser/{id}` for selection or cancellation. These routes are absent
from normal deployments and are not forwarded through remote gateways. Native
launchers long-poll the active request set using a `known` list of request IDs.
Picker selections contain virtual mount paths; the server resolves every path
through the mount confinement boundary before returning local file URIs to the
session portal. Requests originate from the D-Bus portal frontend, never from
remote clients or arbitrary launcher commands. Full protocol details are the
installed `org.freedesktop.impl.portal.FileChooser` and Request interfaces.
