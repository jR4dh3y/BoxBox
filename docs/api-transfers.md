# Downloads and Uploads

Stream files out, archive folders, preview content and upload large files in chunks. All routes need an access token; see [API reference](api.md).

## Download

```http
GET /api/v1/stream/download/{path}
```

The download endpoint supports HTTP range requests.

## Folder Archive

```http
GET /api/v1/stream/archive/{folderPath}
```

Requires authentication and streams the folder as an `application/zip` attachment.

## Preview

```http
GET /api/v1/stream/preview/{path}
```

Preview streams inline content for images, PDFs, audio, video, and text/code previews.

## Chunked Upload

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

## Upload Status

```http
GET /api/v1/stream/upload/status/?uploadId=upload_123
```

