#!/usr/bin/env python3
"""Google Drive and OneDrive as folders (services/CloudDrives.qml), with
rclone: ~/Google Drive and ~/OneDrive, real folders every app and the
terminal can use. Files open on demand; ones you use are cached (up to 2 GB
each) so they open quickly again and changes upload in the background.

  cloud-drive.py status            JSON: {"rclone": bool, "fuse": bool,
                                   "drives": {gdrive|onedrive: {"connected",
                                   "mounted", "folder"}}}
  cloud-drive.py connect DRIVE     signs in (rclone opens the browser at
                                   Google's or Microsoft's own sign-in), then
                                   mounts it; JSON {"ok", "error"}
  cloud-drive.py mount DRIVE       mounts a connected drive (a user unit, so it
                                   outlives a shell restart)
  cloud-drive.py unmount DRIVE
  cloud-drive.py disconnect DRIVE  unmounts it and forgets the sign-in (the
                                   files stay in the cloud)

The sign-in is rclone's (its own app registration with Google and Microsoft),
kept in rclone's config (~/.config/rclone/rclone.conf, as remotes named
irice-gdrive and irice-onedrive).
"""
import json
import os
import shutil
import signal
import subprocess
import sys
import time

DRIVES = {
    "gdrive": {"type": "drive", "folder": "Google Drive", "args": ["scope", "drive"]},
    "onedrive": {"type": "onedrive", "folder": "OneDrive", "args": []},
}


def rclone():
    return os.environ.get("IRICE_RCLONE") or shutil.which("rclone")


def remote(name):
    return "irice-" + name


def unit(name):
    return "irice-drive-" + name


def folder(name):
    return os.path.join(os.path.expanduser("~"), DRIVES[name]["folder"])


def run(cmd, timeout=60, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, **kw)


def remotes():
    exe = rclone()
    if not exe:
        return []
    r = run([exe, "listremotes"])
    return [l.strip().rstrip(":") for l in r.stdout.splitlines() if l.strip()]


def mount_points(table=None):
    """Every mount point (from the kernel's table: a mount whose rclone has
    died is still there, though its folder can't be opened)."""
    points = set()
    try:
        with open(table or os.environ.get("IRICE_MOUNTINFO") or "/proc/self/mountinfo") as f:
            for line in f:
                parts = line.split(" ")
                if len(parts) > 4:
                    # (spaces and the like are written as \040)
                    points.add(parts[4].encode().decode("unicode_escape").encode("latin-1").decode())
    except OSError:
        pass
    return points


def mounted(path):
    return os.path.realpath(path) in mount_points()


def alive(path):
    try:
        os.stat(path)
        return True
    except OSError:  # (ENOTCONN: its rclone is gone)
        return False


def fusermount():
    return shutil.which("fusermount3") or "fusermount"


def has_systemd():
    try:
        return run(["systemctl", "--user", "show-environment"], timeout=10).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def unit_state(name):
    try:
        return run(["systemctl", "--user", "is-active", unit(name)], timeout=10).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def status():
    have = remotes()
    return {"rclone": bool(rclone()), "fuse": bool(shutil.which("fusermount3") or shutil.which("fusermount")),
            "drives": {n: {"connected": remote(n) in have, "mounted": mounted(folder(n)), "folder": folder(n)} for n in DRIVES}}


def mount(name):
    exe = rclone()
    path = folder(name)
    if mounted(path):
        if alive(path):
            return {"ok": True, "error": ""}
        run([fusermount(), "-uz", path])  # (left behind by an rclone that died)
    os.makedirs(path, exist_ok=True)
    if os.listdir(path):
        return {"ok": False, "error": f"~/{DRIVES[name]['folder']} already has files in it. Move them out, then try again."}
    cmd = [exe, "mount", remote(name) + ":", path, "--vfs-cache-mode", "full", "--vfs-cache-max-size", "2G",
           "--dir-cache-time", "1m", "--poll-interval", "1m"]
    if has_systemd():
        # A user unit (it outlives a shell restart). Tried again after a
        # failure, but not forever (a sign-in that has expired)
        state = unit_state(name)
        if state not in ("active", "activating", "reloading"):
            run(["systemctl", "--user", "reset-failed", unit(name)])
            r = run(["systemd-run", "--user", "--quiet", "--collect", "--unit=" + unit(name), "-p", "Restart=on-failure",
                     "-p", "RestartSec=10", "-p", "StartLimitIntervalSec=120", "-p", "StartLimitBurst=3"] + cmd)
            if r.returncode != 0:
                return {"ok": False, "error": (r.stderr.strip().splitlines() or ["rclone couldn't mount it."])[-1]}
    else:
        r = run(cmd + ["--daemon"])
        if r.returncode != 0:
            return {"ok": False, "error": (r.stderr.strip().splitlines() or ["rclone couldn't mount it."])[-1]}
    # (it says it worked once the folder is there)
    for _ in range(40):
        if mounted(path):
            return {"ok": True, "error": ""}
        if has_systemd() and unit_state(name) == "failed":
            break
        time.sleep(0.25)
    if mounted(path) or unit_state(name) in ("active", "activating"):
        return {"ok": True, "error": ""}  # (slow to come up, but coming)
    return {"ok": False, "error": "The drive couldn't be opened. Disconnect it, then connect it again."}


def unmount(name):
    path = folder(name)
    if mounted(path):
        r = run([fusermount(), "-u", path])
        if r.returncode != 0:
            if alive(path):
                return {"ok": False, "error": "A file in it is still open. Close it, then try again."}
            run([fusermount(), "-uz", path])  # (its rclone is gone)
    run(["systemctl", "--user", "stop", unit(name)])
    return {"ok": True, "error": ""}


def connect(name):
    exe = rclone()
    if not exe:
        return {"ok": False, "error": "Install rclone first."}
    d = DRIVES[name]
    if remote(name) not in remotes():
        # (rclone opens the browser and waits for the sign-in: up to 5 minutes)
        try:
            r = run([exe, "config", "create", remote(name), d["type"]] + d["args"] + ["config_is_local", "true"], timeout=330)
        except subprocess.TimeoutExpired:
            run([exe, "config", "delete", remote(name)])
            return {"ok": False, "error": "Sign-in took too long. Try again."}
        except Stopped:
            # (Cancel: rclone, waiting for the browser, is ended by run())
            run([exe, "config", "delete", remote(name)])
            raise
        if r.returncode != 0 or remote(name) not in remotes():
            run([exe, "config", "delete", remote(name)])
            return {"ok": False, "error": "Sign-in didn't finish. Try again, and approve access in the browser."}
    return mount(name)


def disconnect(name):
    r = unmount(name)
    if not r["ok"]:
        return r
    exe = rclone()
    if exe:
        run([exe, "config", "delete", remote(name)])
    try:
        os.rmdir(folder(name))  # (only when it's empty: the mount point)
    except OSError:
        pass
    return {"ok": True, "error": ""}


class Stopped(BaseException):
    """SIGTERM (Cancel in Settings)."""


def stopped(*_):
    raise Stopped()


def main(argv):
    signal.signal(signal.SIGTERM, stopped)
    if argv[:1] == ["status"]:
        print(json.dumps(status()))
        return 0
    if len(argv) == 2 and argv[1] in DRIVES and argv[0] in ("connect", "mount", "unmount", "disconnect"):
        r = {"connect": connect, "mount": mount, "unmount": unmount, "disconnect": disconnect}[argv[0]](argv[1])
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    print(json.dumps({"ok": False, "error": "usage: cloud-drive.py status | connect|mount|unmount|disconnect gdrive|onedrive"}))
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Stopped:
        print(json.dumps({"ok": False, "error": ""}))
        sys.exit(143)
