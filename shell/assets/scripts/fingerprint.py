#!/usr/bin/env python3
"""Fingerprints (services/Fingerprint.qml): the reader, what it needs, the
fingers enrolled, and where a finger is accepted.

  fingerprint.py status          JSON (one line):
      {"reader": {"id": "0a5c:5843", "name"} | null,
       "fprintd": bool (installed), "device": bool (fprintd has the reader),
       "packages": [repo packages to install], "aur": [AUR packages],
       "enrolled": [finger names], "pam": {"polkit", "sudo", "login": bool}}
  fingerprint.py enroll FINGER   enrolls one finger: JSON lines
      {"stage": "claimed", "stages": n} then {"result": "enroll-stage-passed"|
      "enroll-retry-scan"|…, "done": bool, "count": stages passed}, ending
      with {"ok", "error"}
  fingerprint.py delete FINGER   removes one ("all": every one): {"ok", "error"}

The reader: any listed in systemd's fingerprint hwdb (the readers libfprint
drives), and readers that need a vendor driver (DRIVERS: Dell's Broadcom
ControlVault 3 needs libfprint-tod and Dell's TOD driver from the AUR).
fprintd does the rest over D-Bus (net.reactivated.Fprint), as the person at
the computer: no root. Where a finger is accepted besides iRice's lock
screen is a marked line in PAM files (irice-root.py fingerprint), read here.
"""
import glob
import json
import os
import re
import shutil
import sys

HWDB = "/usr/lib/udev/hwdb.d/60-autosuspend-fingerprint-reader.hwdb"
# Readers whose driver isn't in libfprint itself: (name, AUR packages)
DRIVERS = {
    "0a5c:5843": ("Broadcom ControlVault 3", ["libfprint-tod", "libfprint-2-tod1-broadcom"]),
}
FINGERS = ["right-index-finger", "left-index-finger", "right-thumb", "left-thumb", "right-middle-finger",
           "left-middle-finger", "right-ring-finger", "left-ring-finger", "right-little-finger", "left-little-finger"]
MARK = "# iRice fingerprint"
# Where a finger may be accepted (irice-root.py fingerprint): the PAM file of each
PAM_FILES = {"polkit": ["polkit-1"], "sudo": ["sudo"], "login": ["greetd", "ly", "sddm", "lightdm", "lxdm"]}
# Makers of fingerprint readers (USB): one of theirs that libfprint doesn't
# know is a reader without a Linux driver yet, said so instead of hidden
VENDORS = {"27c6": "Goodix", "06cb": "Synaptics", "138a": "Validity", "10a5": "FPC", "1c7a": "Egis",
           "2808": "FocalTech", "147e": "UPEK", "08ff": "AuthenTec"}
ROOT = os.environ.get("IRICE_SYSTEM_ROOT", "/")


def path(p):
    return os.path.join(ROOT, p.lstrip("/"))


def usb_ids():
    out = set()
    for d in glob.glob(path("/sys/bus/usb/devices/*")):
        try:
            with open(os.path.join(d, "idVendor")) as v, open(os.path.join(d, "idProduct")) as p:
                out.add(f"{v.read().strip().lower()}:{p.read().strip().lower()}")
        except OSError:
            continue
    return out


def known_readers():
    """The USB ids in systemd's fingerprint-reader hwdb (lowercase "vvvv:pppp")."""
    try:
        with open(path(HWDB)) as f:
            text = f.read()
    except OSError:
        return set()
    return {f"{v.lower()}:{p.lower()}" for v, p in re.findall(r"^usb:v([0-9A-Fa-f]{4})p([0-9A-Fa-f]{4})", text, re.M)}


def find_reader(ids=None, known=None):
    ids = usb_ids() if ids is None else ids
    known = known_readers() if known is None else known
    for i in sorted(ids):
        if i in DRIVERS:
            return {"id": i, "name": DRIVERS[i][0]}
    for i in sorted(ids):
        if i in known:
            return {"id": i, "name": "Fingerprint reader"}
    for i in sorted(ids):
        maker = VENDORS.get(i.split(":")[0])
        if maker:
            return {"id": i, "name": maker + " fingerprint reader", "unsupported": True}
    return None


def installed(pkg):
    return bool(glob.glob(path(f"/var/lib/pacman/local/{pkg}-[0-9]*")))


def needs(reader):
    """(repo packages, AUR packages) still to install for this reader."""
    if reader is None or reader.get("unsupported"):
        return [], []
    aur = [p for p in DRIVERS.get(reader["id"], ("", []))[1] if not installed(p)]
    repo = [] if installed("fprintd") else ["fprintd"]
    if aur and repo:
        # (fprintd needs libfprint, which libfprint-tod provides: the AUR
        # helper installs them together, so the repo's libfprint never comes)
        return [], aur + repo
    return repo, aur


def pam_state():
    out = {}
    for where, names in PAM_FILES.items():
        on = False
        for n in names:
            try:
                with open(path(f"/etc/pam.d/{n}")) as f:
                    on = on or MARK in f.read()
            except OSError:
                pass
        out[where] = on
    return out


# ── fprintd (D-Bus) ──────────────────────────────────────────────────────
FP = "net.reactivated.Fprint"


class Fprint:
    def __init__(self):
        import gi
        gi.require_version("Gio", "2.0")
        from gi.repository import Gio, GLib
        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)

    def call(self, path_, iface, method, sig=None, args=(), timeout=30000):
        params = self.GLib.Variant(sig, args) if sig else None
        r = self.bus.call_sync(FP, path_, iface, method, params, None, self.Gio.DBusCallFlags.NONE, timeout, None)
        return r.unpack() if r is not None else ()

    def device(self):
        (dev,) = self.call("/net/reactivated/Fprint/Manager", FP + ".Manager", "GetDefaultDevice")
        return dev

    def get(self, dev, prop):
        (v,) = self.call(dev, "org.freedesktop.DBus.Properties", "Get", "(ss)", (FP + ".Device", prop))
        return v


def dbus_error(e):
    """fprintd's error name, short ("NoEnrolledPrints", "PermissionDenied"…)."""
    msg = getattr(e, "message", str(e))
    m = re.search(r"net\.reactivated\.Fprint\.Error\.(\w+)", msg)
    return m.group(1) if m else msg


def enrolled():
    try:
        fp = Fprint()
        dev = fp.device()
        (fingers,) = fp.call(dev, FP + ".Device", "ListEnrolledFingers", "(s)", ("",))
        return True, list(fingers)
    except Exception as e:  # noqa: BLE001
        return dbus_error(e) == "NoEnrolledPrints", []


def status():
    reader = find_reader()
    repo, aur = needs(reader)
    fprintd = installed("fprintd") or bool(shutil.which("fprintd-list"))
    device, fingers = enrolled() if fprintd else (False, [])
    return {"reader": reader, "fprintd": fprintd, "device": device, "packages": repo, "aur": aur,
            "enrolled": fingers, "pam": pam_state(), "gdm": os.path.exists(path("/usr/lib/pam.d/gdm-fingerprint")) or os.path.exists(path("/etc/pam.d/gdm-fingerprint"))}


def emit(obj):
    print(json.dumps(obj), flush=True)


def enroll(finger):
    if finger not in FINGERS:
        emit({"ok": False, "error": "unknown finger"})
        return 1
    try:
        fp = Fprint()
        dev = fp.device()
        fp.call(dev, FP + ".Device", "Claim", "(s)", ("",))
    except Exception as e:  # noqa: BLE001
        emit({"ok": False, "error": {"NoSuchDevice": "No fingerprint reader found.",
                                     "PermissionDenied": "Not allowed to use the reader from here.",
                                     "AlreadyInUse": "The reader is busy. Try again in a moment."}.get(dbus_error(e), dbus_error(e))})
        return 1
    loop = fp.GLib.MainLoop()
    state = {"count": 0, "end": None}
    try:
        stages = int(fp.get(dev, "num-enroll-stages"))
    except Exception:  # noqa: BLE001
        stages = 0
    emit({"stage": "claimed", "stages": stages})

    def on_status(conn, sender, path_, iface, signal, params):
        result, done = params.unpack()
        if result == "enroll-stage-passed":
            state["count"] += 1
        emit({"result": result, "done": bool(done), "count": state["count"]})
        if done:
            state["end"] = result
            loop.quit()
    sub = fp.bus.signal_subscribe(FP, FP + ".Device", "EnrollStatus", dev, None, fp.Gio.DBusSignalFlags.NONE, on_status)
    try:
        fp.call(dev, FP + ".Device", "EnrollStart", "(s)", (finger,))
        fp.GLib.timeout_add_seconds(120, lambda: (state.update(end="enroll-timeout"), loop.quit()))
        loop.run()
    except Exception as e:  # noqa: BLE001
        state["end"] = "error:" + dbus_error(e)
    finally:
        fp.bus.signal_unsubscribe(sub)
        for m in ("EnrollStop", "Release"):
            try:
                fp.call(dev, FP + ".Device", m)
            except Exception:  # noqa: BLE001
                pass
    end = state["end"]
    errors = {"enroll-failed": "That didn't work. Try again.",
              "enroll-data-full": "The reader is full: delete a fingerprint first.",
              "enroll-duplicate": "That finger is already added.",
              "enroll-disconnected": "The reader was disconnected.",
              "enroll-timeout": "It took too long. Try again."}
    emit({"ok": end == "enroll-completed", "error": "" if end == "enroll-completed" else errors.get(end, end or "")})
    return 0 if end == "enroll-completed" else 1


def delete(finger):
    try:
        fp = Fprint()
        dev = fp.device()
        fp.call(dev, FP + ".Device", "Claim", "(s)", ("",))
        try:
            if finger == "all":
                fp.call(dev, FP + ".Device", "DeleteEnrolledFingers2")
            else:
                fp.call(dev, FP + ".Device", "DeleteEnrolledFinger", "(s)", (finger,))
        finally:
            fp.call(dev, FP + ".Device", "Release")
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "error": dbus_error(e)}
    return {"ok": True, "error": ""}


def main(argv):
    if argv[:1] == ["status"]:
        print(json.dumps(status()))
        return 0
    if len(argv) == 2 and argv[0] == "enroll":
        return enroll(argv[1])
    if len(argv) == 2 and argv[0] == "delete":
        r = delete(argv[1])
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    print(json.dumps({"ok": False, "error": "usage: fingerprint.py status | enroll FINGER | delete FINGER|all"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
