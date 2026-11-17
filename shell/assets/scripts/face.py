#!/usr/bin/env python3
"""Face unlock for the lock screen (services/Face.qml), with Howdy (the
howdy-next package from the AUR). An infrared camera is best: a photo or a
screen doesn't show up in infrared. A regular (colour) camera works too, but
only with a blink as well (face_live.py, the tessera-face-rgb PAM file): it
would be fooled by a photo otherwise.

  face.py status     JSON: {"camera": {"path", "name"} | null (an IR camera,
                     by its stable /dev/v4l path: what the QML offers),
                     "cameras": [{"path", "name", "kind": "ir" | "rgb"}]
                     (every camera, infrared first), "howdy": bool
                     (installed), "pam": bool (pam_howdy.so), "pamExec":
                     bool (pam_exec.so, for the blink), "device": the camera
                     Howdy is set to ("" when not set up), "models":
                     downloaded, "private": Howdy's settings are root's to
                     read, "live": face_live.py's status}

howdy-next keeps /etc/howdy root's (0750: users' face models are in it), so
this, running as you, usually can't see the camera it's set to: "private",
and the shell goes by what it set up itself (Config lock.faceDevice).

Adding a face and setting Howdy up need root: tessera-root face …
"""
import glob
import json
import os
import re
import shutil
import sys

# (TESSERA_FACE_ROOT: a stand-in /sys, /dev, /etc and /usr for a sandbox's
# Settings, so it never opens a real camera)
ROOT = os.environ.get("TESSERA_FACE_ROOT") or os.environ.get("TESSERA_SYSTEM_ROOT", "/")
CONFIG = "/etc/howdy/config.ini"
PAM_MODULES = ("/usr/lib/security/pam_howdy.so", "/lib/security/pam_howdy.so")
PAM_EXEC = ("/usr/lib/security/pam_exec.so", "/lib/security/pam_exec.so")
ONNX_DIR = "/usr/share/howdy/models"


def path(p):
    return os.path.join(ROOT, p.lstrip("/"))


def read(p, resolved=False):
    try:
        with open(p if resolved else path(p), errors="replace") as f:
            return f.read()
    except OSError:
        return ""


VIDIOC_ENUM_FMT = 0xC0405602  # _IOWR('V', 2, struct v4l2_fmtdesc), 64 bytes
GREY = 0x59455247  # 'GREY': 8-bit greyscale, what infrared cameras give


def fourcc(code):
    return int.from_bytes(code.encode("ascii").ljust(4)[:4], "little")


# Greyscale formats: infrared cameras give only these (GREY is Y8)
GREYS = {fourcc(c) for c in ("GREY", "Y8  ", "Y10 ", "Y12 ", "Y16 ", "Y10B", "Y8I ", "Y12I")}


def v4l2ctl_formats(text):
    """The formats in `v4l2-ctl --list-formats` output ("[0]: 'YUYV' (…)")"""
    return [fourcc(m.group(1)) for m in re.finditer(r"\[\d+\]:\s*'([^']{1,4})'", text)]


def camera_kind(label, fmts):
    """"ir" for an infrared camera (its name says so, or it only gives
    greyscale), else "rgb" (a regular colour camera)"""
    if re.search(r"\bIR\b|infrared", label, re.I):
        return "ir"
    if fmts and all(f in GREYS for f in fmts):
        return "ir"
    return "rgb"


def formats(dev):
    """The pixel formats a camera captures (VIDIOC_ENUM_FMT: nothing is
    switched on). Empty when it can't be opened."""
    import fcntl
    import struct
    out = []
    try:
        fd = os.open(dev, os.O_RDONLY | os.O_NONBLOCK)
    except OSError:
        return out
    try:
        for i in range(32):
            buf = bytearray(struct.pack("II", i, 1) + bytes(56))  # index, V4L2_BUF_TYPE_VIDEO_CAPTURE
            try:
                fcntl.ioctl(fd, VIDIOC_ENUM_FMT, buf)
            except OSError:
                break
            out.append(struct.unpack_from("I", buf, 44)[0])  # pixelformat, after flags and description[32]
    finally:
        os.close(fd)
    if not out and ROOT == "/" and shutil.which("v4l2-ctl"):
        # (some drivers answer v4l2-ctl's way only: it tries the other buffer types)
        import subprocess
        try:
            r = subprocess.run(["v4l2-ctl", "-d", dev, "--list-formats"], capture_output=True, text=True, timeout=3)
            out = v4l2ctl_formats(r.stdout)
        except (OSError, subprocess.SubprocessError):
            pass
    return out


def cameras(fmts=formats):
    """Every camera, infrared first, by its stable path. Infrared: a
    capture node whose name says IR, or one that only gives greyscale (many
    don't say IR in their name; laptops' Windows Hello cameras). Virtual
    cameras (v4l2loopback: the phone camera, OBS) aren't offered: any app
    could show them a face."""
    found = []
    for node in sorted(glob.glob(path("/sys/class/video4linux/video*"))):
        name = read(os.path.join(node, "name"), True).strip()
        iface = read(os.path.join(node, "device/interface"), True).strip()
        idx = read(os.path.join(node, "index"), True).strip()
        label = iface or name
        dev = os.path.basename(node)
        if idx not in ("", "0"):
            continue
        if "/devices/virtual/" in os.path.realpath(node) or not os.path.exists(os.path.join(node, "device")):
            continue
        named = re.search(r"\bIR\b|infrared", label, re.I)
        f = [] if named else fmts(path("/dev/" + dev))
        kind = camera_kind(label, f)
        stable = ""
        for link in sorted(glob.glob(path("/dev/v4l/by-path/*"))) + sorted(glob.glob(path("/dev/v4l/by-id/*"))):
            if os.path.basename(os.readlink(link)) == dev:
                stable = "/dev/v4l/" + link.split("/dev/v4l/", 1)[1]
                break
        shown = label if named or kind == "rgb" else label + " (infrared)"
        found.append({"path": stable or "/dev/" + dev, "name": shown, "kind": kind})
    return sorted(found, key=lambda c: c["kind"] != "ir")


def ir_camera(fmts=formats):
    """The infrared camera, or None"""
    return next(({"path": c["path"], "name": c["name"]} for c in cameras(fmts) if c["kind"] == "ir"), None)


def config_private():
    """Howdy's config is there but not yours to read (its folder is 0750)"""
    try:
        with open(path(CONFIG)):
            return False
    except PermissionError:
        return True
    except OSError:
        return False


def configured_device():
    m = re.search(r"^\s*device_path\s*=\s*(\S+)", read(CONFIG), re.M)
    return m.group(1) if m and m.group(1).lower() not in ("none", "") else ""


def live_status():
    """face_live.py's status (the blink check's OpenCV, model and face)"""
    try:
        import importlib.util
        here = os.path.dirname(os.path.abspath(__file__))
        spec = importlib.util.spec_from_file_location("face_live", os.path.join(here, "face_live.py"))
        fl = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(fl)
        fl.HOWDY_MODELS = path(ONNX_DIR)
        return fl.status()
    except Exception:  # noqa: BLE001 (a status never fails)
        return {}


def status(fmts=formats):
    cams = cameras(fmts)
    ir = next(({"path": c["path"], "name": c["name"]} for c in cams if c["kind"] == "ir"), None)
    return {"camera": ir, "cameras": cams, "pamExec": any(os.path.exists(path(m)) for m in PAM_EXEC),
            "live": live_status(),
            "howdy": bool(shutil.which("howdy")) if ROOT == "/" else os.path.exists(path("/usr/bin/howdy")),
            "pam": any(os.path.exists(path(m)) for m in PAM_MODULES),
            "device": configured_device(),
            "private": config_private(),
            "models": bool(glob.glob(path(ONNX_DIR + "/*.onnx")))}


if __name__ == "__main__":
    if sys.argv[1:] == ["status"]:
        print(json.dumps(status()))
        sys.exit(0)
    print(json.dumps({"error": "usage: face.py status"}))
    sys.exit(2)
