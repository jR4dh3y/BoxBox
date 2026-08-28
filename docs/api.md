# API Reference

BoxBox exposes a REST API, streaming endpoints, and a WebSocket endpoint under `/api/v1`.

## Base URLs

```text
http://localhost:8080/api/v1   # if Docker maps 8080 -> container port 8080
http://localhost/api/v1        # inside a reverse proxy host
```

Health checks are available at both `/health` and `/api/v1/health`.

## Authentication

All API routes except auth and health require a JWT access token.

### Login

```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "your-password"
}
```

Response:

```json
{
  "accessToken": "eyJhbGciOiJIUzI1NiIs...",
  "refreshToken": "eyJhbGciOiJIUzI1NiIs...",
  "expiresAt": "2026-05-13T10:15:00Z"
}
```

### Use an Access Token

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
```

### Refresh

```http
POST /api/v1/auth/refresh
Content-Type: application/json

{
  "refreshToken": "eyJhbGciOiJIUzI1NiIs..."
}
```

### Logout

```http
POST /api/v1/auth/logout
Content-Type: application/json

{
  "refreshToken": "eyJhbGciOiJIUzI1NiIs..."
}
```

## Files

### List Mount Points

```http
GET /api/v1/files
```

```json
{
  "roots": [
    { "name": "home", "readOnly": false },
    { "name": "backups", "readOnly": true }
  ]
}
```

### Drive Stats

```http
GET /api/v1/files/stats
```

```json
{
  "drives": [
    {
      "name": "home",
      "path": "/home/user",
      "device": "/dev/sda2",
      "fsType": "ext4",
      "mountPoint": "/",
      "totalBytes": 250000000000,
      "freeBytes": 100000000000,
      "usedBytes": 150000000000,
      "usedPct": 60,
      "readOnly": false
    }
  ]
}
```

### List a Directory

```http
GET /api/v1/files/{path}?page=1&pageSize=50&sortBy=name&sortDir=asc&filter=log
```

`{path}` starts with the mount name, for example `home/projects`.

Query options:

| Name | Default | Notes |
| --- | --- | --- |
| `page` | `1` | Positive integer. |
| `pageSize` | `50` | Maximum `1000`. |
| `sortBy` | `name` | `name`, `size`, `modTime`, or `type`. |
| `sortDir` | `asc` | `asc` or `desc`. |
| `filter` | empty | Name contains filter. |

Response:

```json
{
  "path": "home/projects",
  "items": [
    {
      "name": "BoxBox",
      "path": "home/projects/BoxBox",
      "size": 4096,
      "isDir": true,
      "modTime": "2026-05-13T09:00:00Z",
      "permissions": "drwxr-xr-x"
    }
  ],
  "totalCount": 1,
  "page": 1,
  "pageSize": 50
}
```

### Get File Info

```http
GET /api/v1/files/{path}
```

If the path is a file, the response is one `FileInfo` object. If it is a directory, the response is a `FileList`.

### Create Directory

```http
POST /api/v1/files/{basePath}
Content-Type: application/json

{
  "name": "new-folder"
}
```

### Rename or Move

```http
PUT /api/v1/files/{oldPath}
Content-Type: application/json

{
  "newPath": "home/projects/renamed.txt"
}
```

### Delete

```http
DELETE /api/v1/files/{path}
```

Directories require confirmation:

```http
DELETE /api/v1/files/{path}?confirm=true
```

## Streaming

### Download

```http
GET /api/v1/stream/download/{path}
```

The download endpoint supports HTTP range requests.

### Preview

```http
GET /api/v1/stream/preview/{path}
```

Preview streams inline content for images, PDFs, audio, video, and text/code previews.

### Chunked Upload

```http
POST /api/v1/stream/upload/{destinationPath}
Content-Type: application/octet-stream
X-Upload-ID: upload_123
X-Chunk-Index: 0
X-Total-Chunks: 12
X-Chunk-Size: 10485760
X-Total-Size: 120000000
X-Checksum: sha256:abc123...

[binary chunk body]
```

`X-Checksum` is optional and normally sent on the final chunk.

Response:

```json
{
  "uploadId": "upload_123",
  "chunkIndex": 0,
  "receivedChunks": 1,
  "totalChunks": 12,
  "complete": false
}
```

Final response:

```json
{
  "uploadId": "upload_123",
  "chunkIndex": 11,
  "receivedChunks": 12,
  "totalChunks": 12,
  "complete": true,
  "path": "home/uploads/archive.zip"
}
```

### Upload Status

```http
GET /api/v1/stream/upload/status/?uploadId=upload_123
```

## Search

```http
GET /api/v1/search?path=home&q=invoice
```

```json
{
  "path": "home",
  "query": "invoice",
  "results": [],
  "count": 0
}
```

Both `path` and `q` are required.

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

## Errors

Errors use:

```json
{
  "error": "Path is required",
  "code": "VALIDATION_ERROR",
  "details": ""
}
```

Common codes include `VALIDATION_ERROR`, `UNAUTHORIZED`, `TOKEN_INVALID`, `ACCESS_DENIED`, `READ_ONLY`, `INVALID_PATH`, `NOT_FOUND`, `CONFLICT`, `JOB_NOT_FOUND`, `CHECKSUM_MISMATCH`, and `INTERNAL_ERROR`.
