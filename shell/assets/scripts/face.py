#!/usr/bin/env python3
"""Face unlock for the lock screen (services/Face.qml), with Howdy (the
howdy-next package from the AUR) and an infrared camera: a photo or a screen
doesn't show up in infrared, which a plain webcam would be fooled by, so only
IR cameras are offered.

  face.py status     JSON: {"camera": {"path", "name"} | null (an IR camera,
                     by its stable /dev/v4l path), "howdy": bool (installed),
                     "pam": bool (pam_howdy.so), "device": the camera Howdy
                     is set to ("" when not set up), "models": downloaded}

Adding a face and setting Howdy up need root: irice-root face …
"""
import glob
import json
import os
import re
import shutil
import sys

ROOT = os.environ.get("IRICE_SYSTEM_ROOT", "/")
CONFIG = "/etc/howdy/config.ini"
PAM_MODULES = ("/usr/lib/security/pam_howdy.so", "/lib/security/pam_howdy.so")
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
    return out


def ir_camera(fmts=formats):
    """The infrared camera (laptops' Windows Hello cameras), by its stable
    path: a capture node whose name says IR, or one that only gives
    greyscale (many don't say IR in their name)."""
    found = []
    for node in sorted(glob.glob(path("/sys/class/video4linux/video*"))):
        name = read(os.path.join(node, "name"), True).strip()
        iface = read(os.path.join(node, "device/interface"), True).strip()
        idx = read(os.path.join(node, "index"), True).strip()
        label = iface or name
        dev = os.path.basename(node)
        if idx not in ("", "0"):
            continue
        named = re.search(r"\bIR\b|infrared", label, re.I)
        f = [] if named else fmts(path("/dev/" + dev))
        if not named and not (f and all(x == GREY for x in f)):
            continue
        stable = ""
        for link in sorted(glob.glob(path("/dev/v4l/by-path/*"))) + sorted(glob.glob(path("/dev/v4l/by-id/*"))):
            if os.path.basename(os.readlink(link)) == dev:
                stable = "/dev/v4l/" + link.split("/dev/v4l/", 1)[1]
                break
        found.append({"path": stable or "/dev/" + dev, "name": label if named else label + " (infrared)"})
    return found[0] if found else None


def configured_device():
    m = re.search(r"^\s*device_path\s*=\s*(\S+)", read(CONFIG), re.M)
    return m.group(1) if m and m.group(1).lower() not in ("none", "") else ""


def status():
    return {"camera": ir_camera(),
            "howdy": bool(shutil.which("howdy")) if ROOT == "/" else os.path.exists(path("/usr/bin/howdy")),
            "pam": any(os.path.exists(path(m)) for m in PAM_MODULES),
            "device": configured_device(),
            "models": bool(glob.glob(path(ONNX_DIR + "/*.onnx")))}


if __name__ == "__main__":
    if sys.argv[1:] == ["status"]:
        print(json.dumps(status()))
        sys.exit(0)
    print(json.dumps({"error": "usage: face.py status"}))
    sys.exit(2)
