# Files API

List mount points and directories, read file details, and create, rename, move or delete entries. All routes need an access token; see [API reference](api.md).

## List Mount Points

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

## Drive Stats

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

## List a Directory

```http
GET /api/v1/files/list/{path}
```

`{path}` starts with the mount name, for example `home/projects`.

The response holds every entry of the directory, hidden entries included, in no particular order. Clients reconstruct each entry path as `path + "/" + name`, then sort and filter it. Refresh replaces the listing rather than appending pages. A server rejects a directory with more than 10,000 entries with `413`; the desktop app has no limit.

BoxBox's file views default to folders first, then files, ordered by newest `modTime` within each group with natural filename ties. Saved preferences and explicit sorts override the default field and direction. The metadata exposes modification time, not creation time. Public shared-folder listings apply the default order on the server.

Response:

```json
{
  "path": "home/projects",
  "items": [
    {
      "name": "BoxBox",
      "size": 4096,
      "isDir": true,
      "modTime": "2026-05-13T09:00:00Z",
      "permissions": "drwxr-xr-x"
    }
  ]
}
```

## Get File Info

```http
GET /api/v1/files/{path}
```

If the path is a file, the response is one `FileInfo` object. If it is a directory, the response is a `FileList`.

## Create Directory

```http
POST /api/v1/files/{basePath}
Content-Type: application/json

{
  "name": "new-folder"
}
```

## Rename or Move

```http
PUT /api/v1/files/{oldPath}
Content-Type: application/json

{
  "newPath": "home/projects/renamed.txt"
}
```

## Delete

```http
DELETE /api/v1/files/{path}
```

Directories require confirmation:

```http
DELETE /api/v1/files/{path}?confirm=true
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

