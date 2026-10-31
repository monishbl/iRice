#!/usr/bin/env python3
"""Live check of the Wi-Fi helper's D-Bus shield: host a Wi-Fi Direct group
next to NetworkManager's connection and watch that the group lasts while the
internet stays up; then stop and check the rule is gone. Needs the password
(pkexec). No phone needed."""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

HELPER = str(REPO) + "/shell/assets/scripts/quickshare/qs-wifi-helper.py"
SHIELD = "/etc/dbus-1/system.d/zzz-irice-quickshare.conf"
t0 = time.monotonic()


def say(*a):
    print(f"{time.monotonic() - t0:6.1f}", *a, flush=True)


def sh(*cmd):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=20).stdout


def state():
    groups = [l.split()[1] for l in sh("iw", "dev").splitlines() if l.strip().startswith("Interface p2p-wlan0-")]
    gtype = sh("iw", "dev", groups[0], "info") if groups else ""
    chan = next((l.strip() for l in gtype.splitlines() if "channel" in l), "")
    wifi = next((l for l in sh("nmcli", "-t", "-f", "NAME,DEVICE", "connection", "show", "--active").splitlines()
                 if l.endswith(":wlan0")), "none")
    net = subprocess.run(["ping", "-c1", "-W2", "1.1.1.1"], capture_output=True).returncode == 0
    return f"group={groups or 'none'} {chan} | wlan0 on {wifi} | internet={'up' if net else 'DOWN'} | rule={'on' if os.path.exists(SHIELD) else 'off'}"


p = subprocess.Popen(["pkexec", sys.executable, "-I", HELPER], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
say("waiting for the password…")
hello = p.stdout.readline()
say("helper:", hello.strip())
if not hello:
    sys.exit(1)
say("before:", state())
p.stdin.write(json.dumps({"cmd": "host", "mode": "p2p", "freq": 0}) + "\n")
p.stdin.flush()
reply = json.loads(p.stdout.readline())
say("host →", {k: ("…" if k == "passphrase" else v) for k, v in reply.items()})
for _ in range(10):
    time.sleep(2)
    say(state())
p.stdin.write(json.dumps({"cmd": "stop"}) + "\n")
p.stdin.flush()
say("stop →", p.stdout.readline().strip())
p.stdin.close()
p.wait(20)
time.sleep(2)
say("after:", state())
