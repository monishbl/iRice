#!/usr/bin/env python3
"""Measures one bar in a sandbox arena, the same way for both (docs/rust-ui-spike-slint.md).
Run inside the sandbox:  IRICE_ARENA=4 ./dev.sh run python3 measure.py slint|qml [out.json]

  start-up   exec -> the bar's layer is mapped on IRICE-TEST (Hyprland's layer
             list polled every 2 ms), and the bar's own first-frame log line
  memory     VmRSS, PSS and private memory of the process after 60 s
  idle CPU   utime+stime over the next 60 s, % of one core
  animation  10 menu open/close cycles (pointer click on the logo, Esc), then
             10 workspace switches (1 <-> 5): the client's frame intervals,
             the time it spent drawing (Slint) and the CPU it used
The bar is ended by closing its stdin (Slint) or SIGTERM (Quickshell)."""
import json
import os
import re
import signal
import socket
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
ROOT = os.path.abspath(os.path.join(CRATE, "../.."))
VPTR = os.environ.get("VPTR", "/home/user/Projects/Personal/iRice/.sandbox/bin/vptr")
RT = os.environ["XDG_RUNTIME_DIR"]
SOCK = os.path.join(RT, "hypr", os.environ["HYPRLAND_INSTANCE_SIGNATURE"], ".socket.sock")
TICK = os.sysconf("SC_CLK_TCK")

which = sys.argv[1]
out_path = sys.argv[2] if len(sys.argv) > 2 else os.path.join(RT, f"measure-{which}.json")
if which == "slint":
    cmd = [os.path.join(CRATE, "target/release/tessera-ui")]
    ns = "tessera:bar"
else:
    cmd = ["qs", "-p", os.path.join(CRATE, "qml-baseline/shell.qml")]
    ns = "irice:qmlbar"
log_path = os.path.join(RT, f"measure-{which}.log")


def hypr(req):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(SOCK)
    s.sendall(req.encode())
    out = b""
    while True:
        c = s.recv(65536)
        if not c:
            break
        out += c
    s.close()
    return out.decode(errors="replace")


def mapped():
    try:
        layers = json.loads(hypr("j/layers"))
    except Exception:
        return False
    mon = layers.get("IRICE-TEST", {})
    return any(l.get("namespace") == ns for lv in mon.get("levels", {}).values() for l in lv)


def cpu_ticks(pid):
    with open(f"/proc/{pid}/stat") as f:
        fields = f.read().rsplit(")", 1)[1].split()
    return int(fields[11]) + int(fields[12])  # utime, stime


def mem(pid):
    m = {}
    with open(f"/proc/{pid}/status") as f:
        for l in f:
            k, v = l.split(":", 1)
            if k in ("VmRSS", "VmHWM", "Threads", "RssAnon", "RssFile", "RssShmem"):
                m[k] = int(v.split()[0]) if k != "Threads" else int(v)
    with open(f"/proc/{pid}/smaps_rollup") as f:
        for l in f:
            k, v = l.split(":", 1)
            if k in ("Pss", "Private_Clean", "Private_Dirty"):
                m[k] = int(v.split()[0])
    m["Private"] = m.pop("Private_Clean", 0) + m.pop("Private_Dirty", 0)
    return m  # kB


def children(pid):
    r = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True)
    return [int(x) for x in r.stdout.split()]


def run(*a):
    subprocess.run(a, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=20)


res = {"which": which, "cmd": cmd}
log = open(log_path, "w")
t0 = time.time()
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=log, stderr=log, start_new_session=True)
while not mapped():
    if p.poll() is not None:
        sys.exit(f"{which} exited early, see {log_path}")
    if time.time() - t0 > 20:
        sys.exit("never mapped")
    time.sleep(0.002)
res["startup_mapped_ms"] = round((time.time() - t0) * 1000, 1)
time.sleep(1.5)
log.flush()
m = re.search(r"first-frame wall=([0-9.]+)", open(log_path).read())
res["startup_first_frame_ms"] = round((float(m.group(1)) - t0) * 1000, 1) if m else None

time.sleep(max(0, 60 - (time.time() - t0)))
res["mem_60s_kB"] = mem(p.pid)
res["children"] = children(p.pid)

c0, w0 = cpu_ticks(p.pid), time.time()
time.sleep(60)
c1, w1 = cpu_ticks(p.pid), time.time()
res["idle_cpu_pct"] = round((c1 - c0) / TICK / (w1 - w0) * 100, 3)
res["idle_cpu_ticks"] = c1 - c0


def compositor():
    # the irice-glass plugin's frame pacing on IRICE-TEST (when it's loaded), then reset
    r = subprocess.run(["hyprctl", "irice-glass", "frametimes"], capture_output=True, text=True, timeout=20).stdout
    run("hyprctl", "irice-glass", "frametimes", "reset")
    return next((l for l in r.splitlines() if l.startswith("IRICE-TEST")), r.strip())


def stats():
    if which == "slint":
        n = len(open(log_path).read())
        p.send_signal(signal.SIGUSR1)
        time.sleep(0.5)
        new = open(log_path).read()[n:]
        return [json.loads(l.split("stats ", 1)[1]) for l in new.splitlines() if "stats {" in l]
    r = subprocess.run(["qs", "-p", cmd[2], "ipc", "call", "bar", "stats"], capture_output=True, text=True, timeout=20)
    try:
        return [json.loads(r.stdout.strip())]
    except Exception:
        return [r.stdout + r.stderr]


stats()  # reset
compositor()
# Menu: open with a click on the logo, close with Esc
c0, w0 = cpu_ticks(p.pid), time.time()
for i in range(10):
    run(VPTR, "-o", "IRICE-TEST", "move", "70", "15", "glide", "25", "15", "120", "click")
    time.sleep(0.9)
    run("wtype", "-k", "Escape")
    time.sleep(0.7)
c1, w1 = cpu_ticks(p.pid), time.time()
res["menu"] = {"cpu_ms_per_cycle": round((c1 - c0) / TICK * 1000 / 10, 1), "stats": stats(), "compositor": compositor()}
# Spaces: the capsule slides between 1 and 5
c0, w0 = cpu_ticks(p.pid), time.time()
for i in range(10):
    run("hyprctl", "dispatch", f"hl.dsp.focus({{ workspace = {5 if i % 2 == 0 else 1} }})")
    time.sleep(0.8)
c1, w1 = cpu_ticks(p.pid), time.time()
res["spaces"] = {"cpu_ms_per_switch": round((c1 - c0) / TICK * 1000 / 10, 1), "stats": stats(), "compositor": compositor()}
run("hyprctl", "dispatch", "hl.dsp.focus({ workspace = 1 })")

# Ending: stdin closes (Slint), SIGTERM (Quickshell)
t = time.time()
if which == "slint":
    p.stdin.close()
else:
    p.send_signal(signal.SIGTERM)
try:
    p.wait(10)
    res["exit"] = {"code": p.returncode, "ms": round((time.time() - t) * 1000, 1)}
except subprocess.TimeoutExpired:
    p.kill()
    res["exit"] = "killed after 10 s"
res["log_tail"] = open(log_path).read().splitlines()[-3:]
json.dump(res, open(out_path, "w"), indent=1)
print(json.dumps(res, indent=1))
