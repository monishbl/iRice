#!/usr/bin/env python3
"""Settings → Software Update's states, as the `about` feed's `upd` (the
shell's bridge_about.rs upd_feed), for tools/updater-layout.sh: one JSON
line tessera-ui reads on stdin.
  updater-layout-feed.py <scenario> → one JSON line.
scenario: uptodate | ready | running | restart | error | busy | options | sheet
(long real values: a long commit message, 40 packages, a long log line)"""
import json
import sys

sc = sys.argv[1] if len(sys.argv) > 1 else "ready"
long_commit = ("Lock after a sleep: fingerprint and face checks stop before the system sleeps and start fresh 1.5 s after it wakes "
               "(one left running across a sleep kept fprintd's reader claimed: every new check was refused and retried every 0.3 s, "
               "and the lock never recovered); \"reader busy\" retries back off 0.3 → 5 s")
pkgs = [{"name": n, "from": f"{i}.{i * 3 % 17}.{i % 9}-{i % 4 + 1}", "to": f"{i}.{i * 3 % 17 + 1}.0-1"} for i, n in enumerate(
    ["linux-cachyos-lts-headers", "lib32-mesa-vulkan-intel-git", "xdg-desktop-portal-hyprland-git", "python-cryptography", "qt6-declarative",
     "noto-fonts-cjk", "gst-plugins-bad-libs", "nvidia-open-dkms-beta", "hyprland-protocols", "wireplumber", "pipewire-pulse", "glibc",
     "gcc-libs", "systemd-libs"], 1)]

t = {"current": "2bfff4d0", "latest": "656db163", "available": True, "downgrade": False, "channelName": "Beta", "ahead": 0, "preparing": 0,
     "hasNotes": True, "changelog": [], "notes": [], "leaving": [], "error": "", "offline": False, "job": None, "fraction": -1.0,
     "ready": False, "staged": "", "restarting": False, "canUpdate": True, "undecided": 0, "result": None, "conflicts": [], "choices": {}}
s = {"count": 14, "aurCount": 1, "aurAll": 1, "includeAur": True, "removeDropped": True, "canCheck": True, "canUpdate": True, "running": False,
     "job": None, "progress": -1.0, "cancellable": False, "interrupted": False, "hyprTooNew": False, "hyprShort": "0.58", "hyprNow": "0.57",
     "hyprMax": "0.57", "hold": -1, "missing": 0, "dropped": 0, "list": [], "aurList": [], "held": [], "heldHypr": [], "missingList": [],
     "droppedList": [], "reasons": {}}
upd = {"allowed": True, "fake": False, "checking": False, "checked": True, "checkedAt": "21:58", "anything": True, "running": False,
       "restartComputer": False, "t": t, "s": s,
       "back": {"count": 23, "long": "18 November, 9:12 pm", "day": "Tuesday 9:12 pm", "packages": ["mesa", "linux", "glibc"], "missing": [],
                "canGoBack": True, "skipped": [], "include": []},
       "snap": {"tool": "snapper", "booted": "", "assistant": True, "canMenu": True, "grubBtrfs": True, "menuOn": True, "error": "", "busy": False},
       "info": {"channel": "beta", "channelName": "Beta", "branch": "beta", "current": "2bfff4d0", "date": "18 Nov 2026"},
       "aurHelper": True, "cache": 3_200_000_000, "cacheBusy": False, "cacheError": "", "sheet": "", "log": ""}

if sc == "uptodate":
    upd.update(anything=False)
    t.update(available=False, latest="", hasNotes=False, canUpdate=False)
    s.update(count=0, aurCount=0, aurAll=0, canUpdate=False)
elif sc == "running":
    upd.update(running=True)
    t.update(job={"state": "running", "phase": "download", "how": "patch", "bytes": 8_000_000}, fraction=0.43, canUpdate=False)
    s.update(canUpdate=False)
elif sc == "restart":
    upd.update(anything=False)
    t.update(available=False, ready=True, staged="656db163", canUpdate=False)
    s.update(count=0, aurCount=0, aurAll=0, canUpdate=False)
elif sc == "error":
    s.update(running=False, job={"state": "error", "text": "The package database is in use", "phase": "done", "package": "",
                                 "error": "Another package manager (pacman, yay, paru or pamac) is running. Wait until it's done, then try again.",
                                 "restart": False, "upgraded": 0, "kind": ""})
elif sc == "busy":
    t.update(job={"state": "busy", "holder": {"what": "restart", "started": 1763585880, "at": "21:58", "where": "other",
                                              "log": "/run/user/1000/tessera-update.Ab12Cd/log"}}, canUpdate=False)
elif sc == "options":
    pass
elif sc == "sheet":
    upd.update(running=True, sheet="system-log",
               log="\n".join([f"({i}/14) upgrading {p['name']}  [###########--------]  57%" for i, p in enumerate(pkgs, 1)] +
                             ["warning: /etc/pacman.d/mirrorlist installed as /etc/pacman.d/mirrorlist.pacnew", long_commit] * 3))
    s.update(running=True, progress=0.57, cancellable=False,
             job={"state": "running", "phase": "install", "text": "Installing packages", "package": "lib32-mesa-vulkan-intel-git (8 of 14)",
                  "error": "", "restart": False, "upgraded": 0, "kind": ""})
about = {"upd": upd, "needsInstalling": False}
print(json.dumps({"to": "settings", "feed": {"about": about}}))
