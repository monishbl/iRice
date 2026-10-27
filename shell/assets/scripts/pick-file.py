#!/usr/bin/env python3
"""Asks the user for a file or folder and prints its path (exit 1 if cancelled).

  pick-file.py [--folder] [--title T] [--start DIR] [--filter "Images|*.png *.jpg"]

Uses zenity or kdialog when installed: they open on the display this runs on,
which matters in the tty session. Otherwise the desktop portal's file chooser.
"""
import argparse, os, shutil, subprocess, sys

ap = argparse.ArgumentParser()
ap.add_argument("--folder", action="store_true")
ap.add_argument("--title", default="")
ap.add_argument("--start", default=os.path.expanduser("~"))
ap.add_argument("--filter", default="")  # "Name|*.a *.b"
a = ap.parse_args()
title = a.title or ("Choose a folder" if a.folder else "Choose a file")
start = a.start if os.path.isdir(a.start) else os.path.expanduser("~")
name, pats = (a.filter.split("|", 1) + [""])[:2] if a.filter else ("", "")


def done(path):
    path = path.strip()
    if not path:
        sys.exit(1)
    print(path)
    sys.exit(0)


if shutil.which("zenity"):
    cmd = ["zenity", "--file-selection", "--title", title, "--filename", start.rstrip("/") + "/"]
    if a.folder:
        cmd.append("--directory")
    elif pats:
        cmd += ["--file-filter", f"{name} | {pats}"]
    r = subprocess.run(cmd, capture_output=True, text=True)
    done(r.stdout if r.returncode == 0 else "")
if shutil.which("kdialog"):
    cmd = ["kdialog", "--title", title]
    cmd += ["--getexistingdirectory", start] if a.folder else ["--getopenfilename", start] + ([f"{pats}|{name}"] if pats else [])
    r = subprocess.run(cmd, capture_output=True, text=True)
    done(r.stdout if r.returncode == 0 else "")

# The desktop portal
try:
    from gi.repository import Gio, GLib
except ImportError:
    print("No file chooser: install zenity", file=sys.stderr)
    sys.exit(2)
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
loop = GLib.MainLoop()
result = {}
token = "irice%d" % os.getpid()
sender = bus.get_unique_name()[1:].replace(".", "_")
handle = f"/org/freedesktop/portal/desktop/request/{sender}/{token}"


def on_response(_c, _s, _p, _i, _sig, params):
    code, res = params.unpack()
    if code == 0 and res.get("uris"):
        result["path"] = GLib.filename_from_uri(res["uris"][0])[0]
    loop.quit()


bus.signal_subscribe("org.freedesktop.portal.Desktop", "org.freedesktop.portal.Request", "Response", handle, None, 0, on_response)
opts = {"handle_token": GLib.Variant("s", token), "directory": GLib.Variant("b", a.folder),
        "current_folder": GLib.Variant("ay", start.encode() + b"\0")}
if pats and not a.folder:
    opts["filters"] = GLib.Variant("a(sa(us))", [(name, [(0, p) for p in pats.split()])])
bus.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.portal.FileChooser",
              "OpenFile", GLib.Variant("(ssa{sv})", ("", title, opts)), None, 0, -1, None)
loop.run()
done(result.get("path", ""))
