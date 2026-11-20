"""Drives a NAS-mode BoxBox through its HTTP API and checks ownership on disk."""
import hashlib
import json
import os
import signal
import sys
import time
import urllib.error
import urllib.request

ROOT, SUPERVISOR = sys.argv[1], int(sys.argv[2])
TANK = os.path.join(ROOT, "tank")
BASE = "http://127.0.0.1:18080/api/v1"
failures = 0


def check(ok, message):
    global failures
    print(("PASS " if ok else "FAIL ") + message)
    if not ok:
        failures += 1


def request(method, path, token=None, body=None, headers=None, raw=False):
    data = body if isinstance(body, (bytes, type(None))) else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method)
    if token:
        req.add_header("Authorization", "Bearer " + token)
    if data is not None and not raw:
        req.add_header("Content-Type", "application/json")
    for key, value in (headers or {}).items():
        req.add_header(key, value)
    try:
        # Bounded so a stuck API fails the check and still reaches shutdown.
        with urllib.request.urlopen(req, timeout=30) as response:
            text = response.read()
            return response.status, (json.loads(text) if text and not raw else text)
    except urllib.error.HTTPError as error:
        text = error.read()
        try:
            return error.code, json.loads(text)
        except ValueError:
            return error.code, text
    except (urllib.error.URLError, TimeoutError) as error:
        return 0, {"error": f"{method} {path}: {error}"}


def login(user):
    status, body = request("POST", "/auth/login", body={"username": user, "password": user + "-pass"})
    assert status == 200, (user, status, body)
    return body["accessToken"]


def upload(token, path, content, upload_id, chunk_size=None, chunks=None):
    """Sends the given chunk indexes of content (all by default) and returns the last status."""
    chunk_size = chunk_size or max(len(content), 1)
    total = max(1, -(-len(content) // chunk_size))
    result = None
    for index in range(total) if chunks is None else chunks:
        headers = {
            "Content-Type": "application/octet-stream",
            "X-Upload-ID": upload_id, "X-Chunk-Index": str(index), "X-Total-Chunks": str(total),
            "X-Chunk-Size": str(chunk_size), "X-Total-Size": str(len(content)), "X-Create-Only": "true",
        }
        if index == total - 1:
            headers["X-Checksum"] = "sha256:" + hashlib.sha256(content).hexdigest()
        chunk = content[index * chunk_size:(index + 1) * chunk_size]
        result = request("POST", "/stream/upload/" + path, token, chunk, raw=True, headers=headers)
    return result


def owner(path):
    info = os.stat(path)
    return info.st_uid, info.st_gid


def wait_job(token, job_id, timeout=30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        status, job = request("GET", "/jobs/" + job_id, token)
        if status == 200 and job["state"] in ("completed", "failed", "cancelled"):
            return job
        time.sleep(0.05)
    return {"state": "timeout"}


def processes():
    """pid -> (uid, cmdline, CapEff, NoNewPrivs) for the BoxBox processes."""
    found = {}
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            cmdline = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")
            status = open(f"/proc/{pid}/status").read()
        except OSError:
            continue
        if not cmdline[0].endswith(b"/server"):
            continue
        fields = dict(line.split(":\t", 1) for line in status.splitlines() if ":\t" in line)
        uid = int(fields["Uid"].split()[0])
        found[int(pid)] = (uid, b" ".join(cmdline).decode(), fields["CapEff"].strip(), fields.get("NoNewPrivs", "?").strip())
    return found


# The API runs as 10001 yet signs people in, though only root can read the config.
config_stat = os.stat(os.path.join(ROOT, "config.yaml"))
check((config_stat.st_uid, config_stat.st_mode & 0o777) == (0, 0o600), f"config.yaml is root-owned 0600 ({config_stat.st_uid}, {oct(config_stat.st_mode & 0o777)})")
alice, bob, carol = login("alice"), login("bob"), login("carol")

# An account without a NAS identity gets no file access at all.
status, body = request("GET", "/files/", carol)
check(status == 403 and "no NAS identity" in json.dumps(body), f"unmapped account is refused file access ({status})")

# Uploads into a setgid shared folder belong to the uploader and the folder's
# group. Both sessions use the same upload ID and are open at the same time.
ALICE_TEXT, BOB_TEXT = b"alice wrote this, in two chunks", b"bob wrote this, also in two chunks"
CHUNK = 16
status, _ = upload(alice, "tank/shared/from-alice.txt", ALICE_TEXT, "u1", CHUNK, [0])
check(status in (200, 201), f"alice starts an upload in the shared folder ({status})")
status, _ = upload(bob, "tank/shared/from-bob.txt", BOB_TEXT, "u1", CHUNK, [0])
check(status in (200, 201), f"bob starts an upload with the same upload ID ({status})")
status, _ = upload(alice, "tank/shared/from-alice.txt", ALICE_TEXT, "u1", CHUNK, [1])
check(status in (200, 201), f"alice finishes her upload ({status})")
status, _ = upload(bob, "tank/shared/from-bob.txt", BOB_TEXT, "u1", CHUNK, [1, 2])
check(status in (200, 201), f"bob finishes his upload ({status})")
for name, text, expected in (("from-alice.txt", ALICE_TEXT, (3000, 4000)), ("from-bob.txt", BOB_TEXT, (3001, 4000))):
    path = f"{TANK}/shared/{name}"
    got = (open(path, "rb").read(), owner(path)) if os.path.exists(path) else (None, None)
    check(got == (text, expected), f"{name} has its own content and is owned by {expected[0]}:{expected[1]}, got {got[1]}")

# The kernel keeps private folders private, and BoxBox reports it as a 403.
status, _ = request("GET", "/files/tank/alice", bob)
check(status == 403, f"bob cannot list alice's private folder ({status})")
status, _ = upload(bob, "tank/alice/intruder.txt", b"x", "u2")
check(status == 403 and not os.path.exists(f"{TANK}/alice/intruder.txt"), f"bob cannot upload into alice's private folder ({status})")
status, _ = request("GET", "/files/tank/alice", alice)
check(status == 200, f"alice lists her own private folder ({status})")

# Folders and job copies belong to the account that made them.
status, _ = request("POST", "/files/tank/alice", alice, {"name": "docs", "type": "directory"})
check(status in (200, 201) and owner(f"{TANK}/alice/docs") == (3000, 3000), f"alice's new folder is owned by 3000:3000 ({status})")
status, job = request("POST", "/jobs", alice, {"type": "copy", "sourcePath": "tank/shared/from-bob.txt", "destPath": "tank/alice/docs/copy.txt"})
job = wait_job(alice, job["id"]) if status in (200, 201, 202) else {"state": status}
check(job["state"] == "completed" and owner(f"{TANK}/alice/docs/copy.txt") == (3000, 3000), f"alice's copy of bob's file belongs to alice ({job['state']})")
status, job = request("POST", "/jobs", bob, {"type": "copy", "sourcePath": "tank/alice/docs/copy.txt", "destPath": "tank/bob/stolen.txt"})
if status in (200, 201, 202):
    job = wait_job(bob, job["id"])
    check(job["state"] == "failed" and not os.path.exists(f"{TANK}/bob/stolen.txt"), f"bob's copy out of alice's folder fails ({job['state']}: {job.get('error')})")
else:
    check(status == 403 and not os.path.exists(f"{TANK}/bob/stolen.txt"), f"bob's copy out of alice's folder is refused ({status})")

# A job belongs to its account's stack.
status, _ = request("GET", "/jobs/" + job.get("id", "x"), alice)
check(status == 404, f"alice cannot see bob's job ({status})")

# A public upload link writes as the link's creator.
status, share = request("POST", "/shares", alice, {"path": "tank/alice/docs", "permissions": {"upload": True}})
check(status in (200, 201), f"alice creates an upload link ({status})")
status, _ = request("POST", f"/share/{share['token']}/upload?path=visitor.txt", body=b"from a visitor", raw=True, headers={"Content-Type": "application/octet-stream"})
check(status in (200, 201) and owner(f"{TANK}/alice/docs/visitor.txt") == (3000, 3000), f"a visitor's upload through alice's link belongs to alice ({status})")
status, _ = request("POST", "/shares", bob, {"path": "tank/alice/docs", "permissions": {"upload": True}})
check(status in (403, 404), f"bob cannot share alice's private folder ({status})")

# Each worker runs as its account, without capabilities or a way to regain them.
seen = processes()
workers = {uid: (caps, nnp) for uid, cmd, caps, nnp in seen.values() if "--nas-worker" in cmd}
api = [(uid, caps) for uid, cmd, caps, nnp in seen.values() if "--nas-api" in cmd]
check(set(workers) == {3000, 3001}, f"one worker each for uid 3000 and 3001, got {sorted(workers)}")
check(all(caps == "0000000000000000" and nnp == "1" for caps, nnp in workers.values()), f"workers have no capabilities and no_new_privs set: {workers}")
check(api == [(10001, "0000000000000000")], f"API runs as 10001 with no capabilities: {api}")
supervisor_caps = seen.get(SUPERVISOR, (None, None, None, None))[2]
check(supervisor_caps == "00000000000000c0", f"supervisor keeps only SETUID and SETGID (CapEff {supervisor_caps})")

# Kill alice's worker in the middle of a large copy. Files the API already
# holds stay valid, and alice's next call starts a fresh worker.
with open(f"{TANK}/shared/big.bin", "wb") as big:
    big.truncate(512 << 20)
os.chown(f"{TANK}/shared/big.bin", 3000, 4000)


def kill_worker_mid_copy(attempts=5):
    """Kills alice's worker while a copy is running. A copy that finishes before
    progress is seen proves nothing, so it is retried with a fresh destination."""
    for attempt in range(attempts):
        dest = f"tank/alice/big-{attempt}.bin"
        status, job = request("POST", "/jobs", alice, {"type": "copy", "sourcePath": "tank/shared/big.bin", "destPath": dest})
        if status not in (200, 201, 202):
            return None, dest, {"state": status}
        deadline = time.time() + 30
        while time.time() < deadline:
            _, progress = request("GET", "/jobs/" + job["id"], alice)
            if progress.get("state") in ("completed", "failed", "cancelled"):
                # Finished too soon to prove anything; free its space for the next try.
                if os.path.exists(f"{ROOT}/{dest}"):
                    os.remove(f"{ROOT}/{dest}")
                break
            if progress.get("state") == "running" and 0 < progress.get("progress", 0) < 100:
                worker = next(pid for pid, (uid, cmd, _, _) in processes().items() if uid == 3000 and "--nas-worker" in cmd)
                os.kill(worker, signal.SIGKILL)
                return job, dest, progress
            time.sleep(0.01)
    return None, None, {"state": f"no copy was caught running in {attempts} attempts"}


job, dest, progress = kill_worker_mid_copy()
check(job is not None, f"worker killed mid-copy ({progress.get('state')}, {progress.get('progress')}%)")
job = wait_job(alice, job["id"], timeout=120) if job else {"state": "not started"}
target = f"{ROOT}/{dest}" if dest else ""
size = os.path.getsize(target) if dest and os.path.exists(target) else -1
if job["state"] == "completed":
    check(size == 512 << 20, f"copy finished after its worker was killed, with every byte ({size})")
else:
    check("file worker stopped" in job.get("error", "") and size in (-1,), f"copy reports the stopped worker and leaves no partial file ({job['state']}: {job.get('error')}, size {size})")
status, _ = request("GET", "/files/tank/alice", alice)
check(status == 200, f"alice's next request starts a fresh worker ({status})")
check(sum(1 for uid, cmd, _, _ in processes().values() if uid == 3000 and "--nas-worker" in cmd) == 1, "exactly one worker runs for alice after the restart")

# Upload sessions belong to their account.
status, _ = request("GET", "/stream/upload/status/tank/shared/x?uploadId=u1", carol)
check(status == 403, f"unmapped account cannot read upload status ({status})")

print(f"{failures} failure(s)")
sys.exit(1 if failures else 0)
