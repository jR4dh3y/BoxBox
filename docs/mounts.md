# Mount Points

Mount points are the folders BoxBox shows. They are set in the `mount_points` section of `config.yaml`; see [Configuration](/docs/configuration/) for the rest of the file. Restart BoxBox after changing them.

Every mounted path must have a unique `name` and an absolute container `path`.

```yaml
mount_points:
  - name: "media"
    path: "/srv/media"
    read_only: false
```

The `name` is the first segment in API paths. For example, a mount named `media` is browsed through `/api/v1/files/media`.

## Drives and Places

The sidebar lists mounts in two groups. **Drives** are storage roots and also appear as cards on This Server. **Places** are folder shortcuts such as Downloads or Pictures.

BoxBox picks the group automatically: auto-discovered mounts are drives, a mount inside another mount (such as `/home/user/Downloads` inside `/home/user`) is a place, and everything else is a drive. Set `kind` to choose explicitly:

```yaml
mount_points:
  - name: "downloads"
    path: "/srv/downloads"
    kind: "place"
```

## Read-Only Mounts

Use read-only mounts for backups and other paths that should not be modified:

```yaml
mount_points:
  - name: "backups"
    path: "/mnt/backups"
    read_only: true
```

Writes, uploads, renames, moves, and deletes under that mount return `403`, including access through another mount that exposes the same files. For overlapping locations, the most specific real mount controls write access; read-only aliases of the same location take precedence. An explicitly writable child of a read-only parent remains writable through its own mount. Deleting or moving a parent directory that contains a read-only mount is denied. These restrictions apply to shared-folder recipients as well as signed-in users.

BoxBox must be able to resolve every read-only mount root. If permissions or another resolution failure hide a root's identity, writes are denied across mounts until the root is accessible or the configuration is corrected; an unresolved root could otherwise be reached through a writable alias. Reads remain available. A malformed root beneath a regular file (`ENOTDIR`) cannot hide a live directory and does not block unrelated writes.

## Auto-Discovery

Set `auto_discover: true` on a parent directory when you want BoxBox to expose mounted subdirectories as individual mount points.

```yaml
mount_points:
  - name: "drives"
    path: "/media/devmon"
    read_only: false
    auto_discover: true
```

On Linux, BoxBox checks real system mounts and filters virtual filesystems before adding discovered entries.

## Docker Path Mapping

Paths in `config.yaml` are container paths. Bind host directories to those paths:

```yaml
services:
  boxbox:
    volumes:
      - /srv/media:/srv/media
      - /mnt/backups:/mnt/backups:ro
```

Then configure:

```yaml
mount_points:
  - name: "media"
    path: "/srv/media"
    read_only: false
  - name: "backups"
    path: "/mnt/backups"
    read_only: true
```

