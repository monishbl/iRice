#!/usr/bin/env python3
# speed-probe.py PRELUDE_FILE COMMAND [SECONDS]: run inside a sandbox arena
# (tools/speed-suite.sh). Starts the command (bash, with the capture prelude:
# ipc, P, key) and, for SECONDS, watches the compositor:
#  - the test screen's frames: the glass plugin's per-monitor frame counter
#    (`tessera-glass stats`, frames:IRICE-TEST), read over Hyprland's socket
#    every ~3 ms, so each compositor frame gets a time (±3 ms);
#  - Hyprland's events: the first `openlayer`/`closelayer` (a layer mapped:
#    its first frame; unmapped: gone).
# Prints one tab-separated line:
#   first_ms last_ms frames late max_gap map_ms map_ns unmap_ms unmap_ns
# first/last: the first and the last frame after the command (the motion's
# start and end: frames closer than 250 ms together); late: gaps over 20 ms
# between them; max_gap: the longest.
import os, re, socket, subprocess, sys, time

prelude = open(sys.argv[1]).read()
cmd = sys.argv[2]
secs = float(sys.argv[3]) if len(sys.argv) > 3 else 1.5
sig = os.environ["HYPRLAND_INSTANCE_SIGNATURE"]
rt = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
base = f"{rt}/hypr/{sig}"


def ctl(q):
    c = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    c.connect(f"{base}/.socket.sock")
    c.sendall(q.encode())
    out = b""
    while True:
        d = c.recv(65536)
        if not d:
            break
        out += d
    c.close()
    return out.decode(errors="replace")


FR = re.compile(r"frames:IRICE-TEST (\d+)")


def frames():
    m = FR.search(ctl("tessera-glass stats"))
    return int(m.group(1)) if m else -1


ev = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
ev.connect(f"{base}/.socket2.sock")
ev.setblocking(False)
try:
    while ev.recv(65536):
        pass
except BlockingIOError:
    pass
n0 = frames()
t0 = time.monotonic()
p = subprocess.Popen(["bash", "-c", prelude + "\n" + cmd], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
times = []
mapped = unmapped = None
buf = b""
skip = ("tessera-wallpaper", "tessera-desktop")
last_n = n0
while time.monotonic() - t0 < secs:
    n = frames()
    now = 1000 * (time.monotonic() - t0)
    if n > last_n:
        times += [now] * (n - last_n)   # (two frames between reads: same time)
        last_n = n
    try:
        buf += ev.recv(65536)
    except BlockingIOError:
        pass
    while b"\n" in buf:
        line, buf = buf.split(b"\n", 1)
        e, _, data = line.decode(errors="replace").partition(">>")
        if data.startswith(skip):
            continue
        if e == "openlayer" and mapped is None:
            mapped = (round(now), data)
        elif e == "closelayer" and unmapped is None:
            unmapped = (round(now), data)
    time.sleep(0.002)
p.wait()
first = last = None
late = 0
gap = 0.0
cnt = 0
for i, t in enumerate(times):
    if first is None:
        first = last = t
        cnt = 1
        continue
    g = t - last
    if g > 250:
        break   # (a later, separate change)
    gap = max(gap, g)
    late += g > 20
    last = t
    cnt += 1
f = lambda x: "" if x is None else str(round(x))
m = lambda x: "\t" if x is None else f"{x[0]}\t{x[1]}"
print(f"{f(first)}\t{f(last)}\t{cnt}\t{late}\t{round(gap)}\t{m(mapped)}\t{m(unmapped)}")
