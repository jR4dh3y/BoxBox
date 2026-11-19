# NAS Users

On a NAS, each person usually owns their own folders under their own UID. By default BoxBox reaches every file as one container user, so files you upload belong to that user, and BoxBox can only reach folders that user can reach.

NAS mode maps each BoxBox account to a NAS identity. BoxBox then does that account's file work as that identity, and the NAS decides what each person can read and write:

- Files and folders you create, upload, copy or extract belong to your UID.
- New files in a setgid folder get that folder's group, as on the NAS itself.
- Your NAS groups apply, so a shared group folder works the same in BoxBox.
- A folder your NAS account cannot open returns *Permission denied* in BoxBox.
- An account without a NAS identity cannot browse or change files.

## Configure Identities

Map each account to its numeric UID, primary GID and any supplementary groups. Use numbers because the container cannot look up your NAS's user names. On TrueNAS SCALE they are under **Credentials → Users** and **Credentials → Groups**.

```yaml
users:
  alice: "$2b$12$..."
  bob: "$2b$12$..."

nas:
  identities:
    alice: { uid: 3000, gid: 3000, groups: [3001] }
    bob:   { uid: 3001, gid: 3001, groups: [3000] }
```

Or set one environment variable per account, as `uid:gid` or `uid:gid:group,group`:

```bash
BOXBOX_NAS_IDENTITY_alice=3000:3000:3001
BOXBOX_NAS_IDENTITY_bob=3001:3001:3000
```

Each identity must name a configured user. BoxBox refuses UID or GID `0`, and group `0`.

Restart BoxBox after changing an identity. Jobs and uploads in progress stop, so an upload started as one identity never finishes as another.

## Run the Container

NAS mode starts as root and keeps only the two capabilities it needs to switch identities. Change the Compose service:

```yaml
services:
  boxbox:
    user: "0:0"
    cap_drop:
      - ALL
    cap_add:
      - SETUID
      - SETGID
    security_opt:
      - no-new-privileges:true
```

Keep the rest of the service as it is. BoxBox starts these processes:

| Process | Runs as | Does |
| --- | --- | --- |
| Supervisor | root, only `SETUID` and `SETGID` | Starts the other processes. Never touches files. |
| API | `10001:10001` with no capabilities | Serves the web app, sign-in and share links. |
| One worker per active account | That account's UID, GID and groups, with no capabilities | Opens, creates, renames and deletes that account's files. |

A worker starts the first time an account touches a file and exits after five idle minutes. Workers get no environment variables, so they never see the JWT secret or password hashes.

The API needs to read `config.yaml` and write `/data` and `/tmp/boxbox`, as it does without NAS mode. Set the API's identity with `nas.service_uid` and `nas.service_gid`, or `BOXBOX_NAS_SERVICE_UID` and `BOXBOX_NAS_SERVICE_GID`, if those paths belong to another user.

## Shared Folders

To let a household write to one folder, give the folder a shared group and the setgid bit, and add each person to the group:

```bash
chgrp family /mnt/tank/shared
chmod 2770 /mnt/tank/shared
```

Files created in it get the `family` group. Whether other members can change those files depends on the file's mode. BoxBox currently creates files as `0644` and folders as `0755`, so other group members can read new files but not change them.

## Moves, Copies and Share Links

- **Copy:** you own the copy.
- **Move within one dataset:** a rename, so the file keeps its owner.
- **Move between datasets:** a copy followed by a delete, so you own the result. If removing the source fails after the copy finished, the job says the destination is complete.
- **Share links:** a visitor acts as the person who created the link. Uploads through the link belong to the creator, and the link can never reach more than the creator can. If the creator loses their NAS identity, the link stops working.

## Trust Model

The supervisor picks every UID from its own copy of the configuration. The API only names the account. This prevents bugs from mixing up identities, such as one person's upload finishing as another.

It does not protect against a compromised API. An attacker who controls the API process can ask for any mapped account's worker. Treat the API as you would without NAS mode, and map only the accounts that need file access.

## Limitations

- NAS mode needs Linux. It cannot be combined with `--dev` or `--desktop`.
- Drive usage on This Server is read as the API's identity. If `10001` cannot reach a mount's root, its usage is missing.
- `auto_discover` lists drives as the API's identity, once at startup.
- Tested on Linux with plain POSIX permissions and a setgid shared folder, not yet on TrueNAS or with NFSv4 ACLs. The kernel enforces ACLs for workers too, but that is untested.

## Troubleshooting

| Message | Cause |
| --- | --- |
| `NAS mode must start as root` | The container runs as a non-root user. Set `user: "0:0"`. |
| `Start API process; NAS mode needs the SETUID and SETGID capabilities` | `cap_add` is missing `SETUID` or `SETGID`. |
| *This account has no NAS identity* | Add the account under `nas.identities`. |
| *File access is temporarily unavailable* | The account's worker could not start. The server log names the reason. |
