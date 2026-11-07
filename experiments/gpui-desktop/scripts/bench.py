#!/usr/bin/env python3
"""Compares the GPUI client with the Electrobun app on one X display.

Both apps run the same Go server binary, so the numbers isolate the client. For each run
the script starts the app, nudges the pointer every frame (GPUI on Xvfb repaints on input),
and records when the window first maps and when its pixels stop changing (content settled).
It then measures memory (PSS, client and server separately), idle CPU over 10 s, and the
CPU and wall time of 150 wheel-scroll steps through the open folder.

Usage: bench.py gpui|electrobun RUNS PATH  (PATH is an absolute local folder)
Needs python-xlib and Pillow, DISPLAY set, and for GPUI: GPUI_BIN, SERVER_BIN;
for Electrobun: ELECTROBUN_APPRUN (the extracted AppImage's AppRun).
"""
import hashlib, json, os, signal, statistics, subprocess, sys, time
from Xlib import X, display
from Xlib.ext import xtest

SETTLE = 1.5  # seconds without pixel changes that count as settled
TIMEOUT = 60

def tree(pid):
    """pid and all descendants."""
    children = {}
    for entry in os.listdir('/proc'):
        if entry.isdigit():
            try:
                with open(f'/proc/{entry}/stat') as f:
                    ppid = int(f.read().rsplit(')', 1)[1].split()[1])
                children.setdefault(ppid, []).append(int(entry))
            except (OSError, IndexError, ValueError):
                pass
    out, stack = [], [pid]
    while stack:
        p = stack.pop(); out.append(p); stack.extend(children.get(p, []))
    return out

def cmdline(pid):
    try:
        return open(f'/proc/{pid}/cmdline', 'rb').read().replace(b'\0', b' ').decode(errors='replace')
    except OSError:
        return ''

def pss_kib(pid):
    try:
        for line in open(f'/proc/{pid}/smaps_rollup'):
            if line.startswith('Pss:'):
                return int(line.split()[1])
    except OSError:
        pass
    return 0

def cpu_ticks(pid):
    try:
        fields = open(f'/proc/{pid}/stat').read().rsplit(')', 1)[1].split()
        return int(fields[11]) + int(fields[12])
    except (OSError, IndexError):
        return 0

def split(pids):
    server = [p for p in pids if 'boxbox-server' in cmdline(p) or ' --desktop --config' in cmdline(p)]
    return [p for p in pids if p not in server], server

def find_window(d):
    for child in d.screen().root.query_tree().children:
        try:
            if child.get_wm_name() == 'BoxBox' and child.get_attributes().map_state == X.IsViewable:
                return child
        except Exception:
            pass
    return None

def frame_hash(win):
    g = win.get_geometry()
    return hashlib.sha1(win.get_image(0, 0, g.width, g.height, X.ZPixmap, 0xffffffff).data).hexdigest()

def run(kind, path):
    d = display.Display()
    if kind == 'gpui':
        env = dict(os.environ, BOXBOX_SERVER_BIN=os.environ['SERVER_BIN']); env.pop('BOXBOX_URL', None)
        command = [os.environ['GPUI_BIN'], path]
    else:
        env = dict(os.environ)
        command = [os.environ['ELECTROBUN_APPRUN'], path]
    start = time.monotonic()
    proc = subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    mapped = last_change = None; last = None; nudge = 0
    while time.monotonic() - start < TIMEOUT:
        win = find_window(d)
        if win is not None:
            if mapped is None:
                mapped = time.monotonic() - start
            nudge ^= 1
            g = win.get_geometry()
            xtest.fake_input(d, X.MotionNotify, x=g.x + g.width - 20 - nudge, y=g.y + g.height - 20); d.sync()
            try:
                h = frame_hash(win)
            except Exception:
                h = None
            if h != last:
                last, last_change = h, time.monotonic() - start
            elif last_change is not None and time.monotonic() - start - last_change >= SETTLE:
                break
        time.sleep(0.03)
    settled = last_change
    time.sleep(2)
    shot = os.environ.get('BENCH_SHOTS')
    win = find_window(d)
    if shot and win is not None:
        from PIL import Image
        g = win.get_geometry(); raw = win.get_image(0, 0, g.width, g.height, X.ZPixmap, 0xffffffff)
        Image.frombytes('RGBX', (g.width, g.height), raw.data, 'raw', 'BGRX').convert('RGB').save(f'{shot}/bench-{kind}.png')
    pids = tree(proc.pid)
    client, server = split(pids)
    client_pss = sum(pss_kib(p) for p in client) / 1024
    server_pss = sum(pss_kib(p) for p in server) / 1024
    hz = os.sysconf('SC_CLK_TCK')
    before = sum(cpu_ticks(p) for p in client); time.sleep(10)
    idle_cpu = (sum(cpu_ticks(p) for p in client) - before) / hz
    # Scroll the listing: 150 wheel-down steps over the middle of the window.
    win = find_window(d); g = win.get_geometry()
    xtest.fake_input(d, X.MotionNotify, x=g.x + g.width // 2, y=g.y + g.height // 2); d.sync()
    client_now, _ = split(tree(proc.pid))
    before_frame = frame_hash(win)
    before = sum(cpu_ticks(p) for p in client_now); t0 = time.monotonic()
    for _ in range(150):
        xtest.fake_input(d, X.ButtonPress, 5); xtest.fake_input(d, X.ButtonRelease, 5); d.sync(); time.sleep(0.01)
    time.sleep(1.0)
    scroll_cpu = (sum(cpu_ticks(p) for p in client_now) - before) / hz
    scroll_wall = time.monotonic() - t0 - 1.0
    xtest.fake_input(d, X.MotionNotify, x=g.x + g.width // 2 + 1, y=g.y + g.height // 2); d.sync(); time.sleep(0.3)
    scrolled = frame_hash(win) != before_frame
    os.killpg(proc.pid, signal.SIGTERM)
    try:
        proc.wait(10)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
    time.sleep(2)
    return dict(mapped_s=mapped, settled_s=settled, client_pss_mib=client_pss, server_pss_mib=server_pss,
                client_processes=len(client), idle_cpu_s_per_10s=idle_cpu, scroll_cpu_s=scroll_cpu, scroll_wall_s=scroll_wall, scrolled=scrolled)

if __name__ == '__main__':
    kind, runs, path = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    results = [run(kind, path) for _ in range(runs)]
    median = {key: statistics.median(r[key] for r in results if r[key] is not None) for key in results[0]}
    print(json.dumps({'app': kind, 'path': path, 'runs': results, 'median': median}, indent=1))
