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


def ir_camera():
    """The infrared camera: a capture node whose name says IR (laptops'
    Windows Hello cameras), by its stable path."""
    found = []
    for node in sorted(glob.glob(path("/sys/class/video4linux/video*"))):
        name = read(os.path.join(node, "name"), True).strip()
        iface = read(os.path.join(node, "device/interface"), True).strip()
        idx = read(os.path.join(node, "index"), True).strip()
        label = iface or name
        if not re.search(r"\bIR\b|infrared", label, re.I) or idx not in ("", "0"):
            continue
        dev = os.path.basename(node)
        stable = ""
        for link in sorted(glob.glob(path("/dev/v4l/by-path/*"))) + sorted(glob.glob(path("/dev/v4l/by-id/*"))):
            if os.path.basename(os.readlink(link)) == dev:
                stable = "/dev/v4l/" + link.split("/dev/v4l/", 1)[1]
                break
        found.append({"path": stable or "/dev/" + dev, "name": label})
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
