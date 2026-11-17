#!/usr/bin/env python3
"""The gate's settings for its arena (tools/gate): the same every run, with
what would make two runs differ switched off.
  config.py <config.json> <wallpaper> [--check]
Writes them into config.json (made if missing); --check only says (exit 1)
whether a key the gate set has a different value now (the shell rewrote it).
"""
import json
import os
import sys

path, wall = sys.argv[1], sys.argv[2]
check = "--check" in sys.argv[3:]

GATE = {
    # the Rust shell alone, the first-start screens done
    "rust.standalone": True,
    "rust.surfaces": [],
    "welcome.done": True,
    # one look: Glass, dark, the liquid material, a fixed accent
    "appearance.look": "glass",
    "appearance.mode": "dark",
    "appearance.glass": "liquid",
    "appearance.accent": "wallpaper",
    "appearance.reduceMotion": False,
    # one still wallpaper of the repo's own
    "wallpaper.path": wall,
    "wallpaper.dynamic": "off",
    "wallpaper.live": "",
    # nothing from the network or the clock of the day: no weather, stocks,
    # morning greeting, update checks; the desktop shows the clock only
    "menubar.weather": False,
    "desktop.weather": False,
    "desktop.icons": False,
    "desktop.smartRotate": False,
    "desktop.widgets": ["clock:m:0:0"],
    "today.mode": "off",
    "update.autoCheck": False,
    # the island: only what the script makes (timers); no system events
    # (battery, Bluetooth, USB, resources: the build machine's own), no
    # temperature alert, no random content
    "island.events": ["timers"],
    "island.hotTemp": 200,
    "island.hotThrottle": False,
    "island.pinned": False,
    # no idle lock half way through a run, no login intro
    "lock.standby": False,
    "lock.loginIntro": False,
}


def get(c, k):
    for p in k.split("."):
        if not isinstance(c, dict) or p not in c:
            return None
        c = c[p]
    return c


def put(c, k, v):
    *ps, last = k.split(".")
    for p in ps:
        if not isinstance(c.get(p), dict):
            c[p] = {}
        c = c[p]
    c[last] = v


c = {}
if os.path.exists(path):
    with open(path) as f:
        c = json.load(f)
if check:
    off = {k: get(c, k) for k, v in GATE.items() if get(c, k) != v}
    if off:
        print("changed:", json.dumps(off), file=sys.stderr)
    sys.exit(1 if off else 0)
for k, v in GATE.items():
    put(c, k, v)
os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w") as f:
    json.dump(c, f, indent=4)
