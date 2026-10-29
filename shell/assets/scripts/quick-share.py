#!/usr/bin/env python3
"""Hand selected files to Packet's published Quick Share D-Bus action.

Packet owns discovery, pairing/confirmation and transfer progress. No receiver
is started until the user opens Packet or explicitly shares files with it.
Native and Flatpak installations are supported; no files are copied to a shell
command or clipboard. See Packet's packet_nautilus.py for the D-Bus contract.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

APP_ID = "io.github.nozwock.Packet"
OBJECT_PATH = "/io/github/nozwock/Packet/Share"


def installed_command():
    if shutil.which("packet"):
        return [shutil.which("packet")]
    if shutil.which("flatpak"):
        check = subprocess.run(["flatpak", "info", APP_ID], capture_output=True, timeout=10)
        if check.returncode == 0:
            return ["flatpak", "run", APP_ID]
    return None


def activate(paths):
    """Packet's send-files action, called over D-Bus with the paths as data
    (a GVariant built by GLib, not text pasted into a gdbus command)."""
    from gi.repository import Gio, GLib
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    bus.call_sync(APP_ID, OBJECT_PATH, "org.gtk.Actions", "Activate",
                  GLib.Variant("(sava{sv})", ("send-files", [GLib.Variant("as", paths)], {})),
                  None, Gio.DBusCallFlags.NONE, 10000, None)


def dbus_available():
    try:
        import gi
        gi.require_version("Gio", "2.0")
        from gi.repository import Gio  # noqa: F401
        return True
    except (ImportError, ValueError):
        return False


def handoff(paths, command):
    paths = [str(Path(p).expanduser().resolve()) for p in paths]
    if not paths or any(not Path(p).is_file() for p in paths):
        raise ValueError("Choose existing files to send. Folders must be zipped first.")
    for p in paths:
        try:
            p.encode("utf-8")
        except UnicodeEncodeError:
            raise ValueError("Rename %s: Quick Share only takes file names in UTF-8." % os.path.basename(p.encode("utf-8", "surrogateescape").decode("utf-8", "replace")))
    if not dbus_available():
        raise RuntimeError("Quick Share needs python-gobject. Install it and try again.")
    subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    # An already running Packet handles this immediately. A first launch needs
    # a moment to export its action group. Retry only pre-delivery errors.
    for attempt in range(25):
        try:
            activate(paths)
            return
        except Exception as error:  # GLib.Error (Gio isn't imported up here)
            text = str(error)
            if not any(name in text for name in ["ServiceUnknown", "NameHasNoOwner", "UnknownObject", "UnknownMethod"]) or attempt == 24:
                raise RuntimeError("Packet couldn't open the files. Open Packet and try again. " + text[-350:])
        time.sleep(0.2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--status", action="store_true")
    parser.add_argument("--open", action="store_true")
    parser.add_argument("paths", nargs="*")
    args = parser.parse_args()
    try:
        command = installed_command()
        if args.status:
            print(json.dumps({"available": command is not None and dbus_available()}))
            return 0
        if command is None:
            raise RuntimeError("Install Packet to use Android Quick Share.")
        if args.open:
            subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        else:
            handoff(args.paths, command)
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
