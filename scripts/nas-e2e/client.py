"""Drives a NAS-mode BoxBox through its HTTP API and checks ownership on disk."""
import hashlib
import json
import os
import signal
import subprocess
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
        with urllib.request.urlopen(req) as response:
            text = response.read()
            return response.status, (json.loads(text) if text and not raw else text)
    except urllib.error.HTTPError as error:
        text = error.read()
        try:
            return error.code, json.loads(text)
        except ValueError:
            return error.code, text


def login(user):
    status, body = request("POST", "/auth/login", body={"username": user, "password": user + "-pass"})
    assert status == 200, (user, status, body)
    return body["accessToken"]


def upload(token, path, content, upload_id):
    digest = hashlib.sha256(content).hexdigest()
    return request("POST", "/stream/upload/" + path, token, content, raw=True, headers={
        "Content-Type": "application/octet-stream",
        "X-Upload-ID": upload_id, "X-Chunk-Index": "0", "X-Total-Chunks": "1",
        "X-Chunk-Size": str(max(len(content), 1)), "X-Total-Size": str(len(content)),
        "X-Checksum": "sha256:" + digest, "X-Create-Only": "true",
    })


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


alice, bob, carol = login("alice"), login("bob"), login("carol")

# An account without a NAS identity gets no file access at all.
status, body = request("GET", "/files/", carol)
check(status == 403 and "no NAS identity" in json.dumps(body), f"unmapped account is refused file access ({status})")

# Uploads into a setgid shared folder belong to the uploader and the folder's group.
status, _ = upload(alice, "tank/shared/from-alice.txt", b"alice wrote this", "u1")
check(status in (200, 201), f"alice uploads into the shared folder ({status})")
check(owner(f"{TANK}/shared/from-alice.txt") == (3000, 4000), f"alice's upload is owned by 3000:4000, got {owner(f'{TANK}/shared/from-alice.txt')}")
status, _ = upload(bob, "tank/shared/from-bob.txt", b"bob wrote this", "u1")
check(status in (200, 201), f"bob uploads into the shared folder with the same upload ID ({status})")
check(owner(f"{TANK}/shared/from-bob.txt") == (3001, 4000), f"bob's upload is owned by 3001:4000, got {owner(f'{TANK}/shared/from-bob.txt')}")

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
status, job = request("POST", "/jobs", alice, {"type": "copy", "sourcePath": "tank/shared/big.bin", "destPath": "tank/alice/big.bin"})
alice_worker = next(pid for pid, (uid, cmd, _, _) in processes().items() if uid == 3000 and "--nas-worker" in cmd)
os.kill(alice_worker, signal.SIGKILL)
job = wait_job(alice, job["id"], timeout=120)
size = os.path.getsize(f"{TANK}/alice/big.bin") if os.path.exists(f"{TANK}/alice/big.bin") else -1
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
