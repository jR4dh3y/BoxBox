# Share Links API

Share links expose an existing file or folder through a token URL. Management endpoints require a JWT; recipient endpoints are public and authenticate by share token alone.

## Create a Share

```http
POST /api/v1/shares
Content-Type: application/json

{
  "path": "home/projects/shared",
  "permissions": { "view": true, "download": true, "upload": true, "delete": false, "manage": false },
  "maxUploadBytes": 104857600,
  "expiresInSeconds": 3600
}
```

`expiresInSeconds` is optional; omit it for a share that never expires. Folder links always allow viewing and downloading. Set `upload` to allow new files, and set both `upload` and `delete` to allow replacing or removing items below the shared folder. The optional `maxUploadBytes` value is a per-file cap; omitting it uses the server's `max_upload_mb` limit. The server cap always applies. File links always use view/download permissions. Share responses include the effective `canReplace` capability; it is true when delete is allowed or a persisted legacy `write` permission retains replacement access. For existing v1 clients, deprecated `write` mirrors `upload` so upload-only links remain usable; new clients should use `upload`, `delete`, and `canReplace`. Requests cannot set `canReplace`.

Full-access folder links set `manage`, `upload`, and `delete` to `true`. `manage` permits folder creation and empty file edits. File links cannot grant these capabilities.

### Delete a File When Its Link Expires

Set `"deleteOnExpiry": true` to delete the shared file when the link expires. It is off by default. The request needs a file, not a folder or a symlink, on a writable mount, and an `expiresInSeconds` value. Otherwise it returns `400`, or `403` for a read-only mount. `expiresInSeconds` cannot exceed 100 years.

A background job checks expired links every minute, and once when the server starts, so a link that expired while the server was down is handled at startup. Each link gets one attempt. The job deletes the file only if all of these are true at that moment:

- The path still names the same file. The server compares size and modification time, and also device and inode on Unix systems. A file that was edited, renamed, or replaced is kept.
- The mount still exists and is writable.
- No other active link reaches the file. That includes a link to the same file, a folder link that contains it, links through another mount name for the same folder, and, on Unix systems, links through a hard link to the file or a bind mount of a folder that contains it. A file kept for this reason stays, even after the other link ends.

The file is moved aside before the final check, so a save that replaces it at that moment is kept. Every other outcome keeps the file, and the server logs each deletion and each kept file. Revoking a link never deletes its file. Create and list responses report `deleteOnExpiry`; recipient metadata does not.

Response uses `201 Created`:

```json
{
  "id": "b0c1d2e3-...",
  "token": "7nArUufciyjMLLFstZGU_zIerVgTpr2Qr2eL1lUTJFY",
  "url": "/s/7nArUufciyjMLLFstZGU_zIerVgTpr2Qr2eL1lUTJFY",
  "fileName": "shared",
  "permissions": { "view": true, "download": true, "upload": true, "delete": false, "manage": false, "canReplace": false, "write": true },
  "maxUploadBytes": 104857600,
  "isFolder": true,
  "createdAt": "2026-09-02T09:00:00Z",
  "expiresAt": "2026-09-02T10:00:00Z",
  "deleteOnExpiry": false
}
```

## List Active Shares

```http
GET /api/v1/shares
```

Returns only the authenticated caller's active (non-revoked, non-expired) shares. Each entry includes the `token`, `url`, `fileName`, mount-relative `path`, `permissions`, `maxUploadBytes`, `isFolder`, expiry, and `deleteOnExpiry`.

## Update a Folder Share

```http
PATCH /api/v1/shares/{id}
Content-Type: application/json

{
  "permissions": { "view": true, "download": true, "upload": true, "delete": true, "manage": true },
  "maxUploadBytes": 52428800
}
```

Updates permissions and the per-file upload cap on an active folder share owned by the caller. `delete` requires `upload`, and `manage` requires both. A value of `0` for `maxUploadBytes` restores the server's configured maximum.

## Revoke a Share

```http
DELETE /api/v1/shares/{id}
```

Unknown ids and links owned by another user return `404`. Revoked links stop working immediately.

## Recipient Endpoints

Recipient endpoints are public (no JWT) and rate-limited per client IP. The share token in the path is the only credential.

```http
GET /api/v1/share/{token}
```

Returns recipient-facing metadata only; mount names and internal paths are never included:

```json
{
  "fileName": "shared",
  "size": 1024,
  "mimeType": "application/octet-stream",
  "permissions": { "view": true, "download": true, "upload": true, "delete": false, "manage": false, "canReplace": false, "write": true },
  "maxUploadBytes": 104857600,
  "isFolder": true,
  "expiresAt": "2026-09-02T10:00:00Z"
}
```

```http
GET /api/v1/share/{token}/download
GET /api/v1/share/{token}/preview
GET /api/v1/share/{token}/items?path=reports
GET /api/v1/share/{token}/archive?path=reports
GET /api/v1/share/{token}/wallpaper
POST /api/v1/share/{token}/folders?path=reports/new-folder
POST /api/v1/share/{token}/upload?path=reports/new.txt
DELETE /api/v1/share/{token}/items?path=reports/old.txt
```

Folder item paths are relative to the share root. Archive downloads include only paths inside that root. Delete requires the `delete` permission and cannot remove the shared root. Download streams a file as an attachment; preview streams it inline for browser media. Both support HTTP range requests, set a sandboxed `Content-Security-Policy`, and force attachment disposition for active document formats (HTML, SVG, XML).

Unknown, expired, and revoked tokens all return the same `404`, so recipients cannot distinguish between them. Requests the share's permissions do not allow return `403`.

Folder creation requires `manage` and a writable mount. It creates missing parents below the shared root and returns `201` with `{"success": true}`. Traversal outside the shared root is denied.

The wallpaper endpoint serves the owner's image with the sandboxed content policy and returns `404` when no image is stored. Share metadata optionally includes `wallpaper` with `mode`, `frostedGlass`, and a numeric `version` for cache invalidation.

## Upload Through a Folder Share

```http
POST /api/v1/share/{token}/upload?path=reports/new.txt
Content-Type: application/octet-stream

[file body]
```

Requires the `upload` permission and a writable mount point. Upload-only links create new files but cannot replace an existing item; links with `delete` can also replace or delete items below the shared folder. Uploads are atomically published and capped by both the per-link `maxUploadBytes` and server `max_upload_mb` limits. If a revocation completes while the body is still uploading, the pending upload is discarded:

```json
{
  "fileName": "notes.txt",
  "size": 2048
}
```

`403` means the share does not allow this operation; `413` means the body exceeded a size limit. Existing persisted `write` links retain their historical replacement behavior but do not gain delete permission.

A zero-byte body can empty an existing file when the link has `manage` permission. Other zero-byte uploads are rejected. The server rechecks permissions and revocation before publishing the edit.

