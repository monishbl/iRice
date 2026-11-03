#!/usr/bin/env python3
"""What this computer has, and what the iRice installer could set up for it.

  hardware.py plan             the plan, one item a line (tab-separated:
                               id, ticked 1/0, ask 1/0, locked 1/0, group,
                               title, why, packages, AUR packages)
  hardware.py plan --json      the same as JSON, with the detected facts
  hardware.py apply ID...      what install.sh does for the chosen items,
                               one a line: "pkg NAME" (pacman), "aur NAME"
                               (yay or paru), "svc NAME" (systemctl enable),
                               "scale OUTPUT SCALE" (Hyprland's monitor scale)
  --facts FILE                 (before the command) facts from a JSON file
                               instead of this computer's (tests)

Only reads: lspci, /sys, /proc and pacman's database; nothing here installs
or changes anything (install.sh does, after showing the plan). The plan
holds only what's missing and relevant: firmware and drivers for the
hardware found, the fingerprint reader's and the face camera's (each asked),
what the installer builds for this machine (shown, always done), and
tuning. An NVIDIA driver is only ever installed when asked for by name.

Detection is pure functions over text (parse_lspci, parse_cpu, edid_size,
suggest_scale, plan): tests feed them fixtures. IRICE_SYSTEM_ROOT moves /sys,
/proc and /var/lib/pacman somewhere else (tests).
"""
import glob
import importlib.util
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
SCRIPTS = os.path.join(os.path.dirname(os.path.dirname(HERE)), "shell", "assets", "scripts")
ROOT = os.environ.get("IRICE_SYSTEM_ROOT", "/")

GROUPS = ["Firmware & drivers", "Biometrics", "Build for this machine", "Tuning"]
# PCI vendors
INTEL, AMD, NVIDIA, BROADCOM = "8086", "1002", "10de", "14e4"
VENDOR_NAMES = {INTEL: "Intel", AMD: "AMD", NVIDIA: "NVIDIA", BROADCOM: "Broadcom"}
# Other power managers: power-profiles-daemon would fight them
POWER_MANAGERS = ("tlp", "auto-cpufreq", "tuned-ppd", "tuned")


def path(p):
    return os.path.join(ROOT, p.lstrip("/"))


def load(name):
    """One of the shell's scripts (fingerprint.py, face.py) as a module: the
    installer asks them, rather than knowing readers and cameras twice."""
    spec = importlib.util.spec_from_file_location("irice_" + name, os.path.join(SCRIPTS, name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# ── Parsing ─────────────────────────────────────────────────────────────
DEV = re.compile(r"^(\S+) (.+?) \[([0-9a-f]{4})\]: (.*) \[([0-9a-f]{4}):([0-9a-f]{4})\](?: \(rev [0-9a-f]+\))?$")


def parse_lspci(text):
    """`lspci -nnk`: [{slot, cls (e.g. "0300"), kind, vendor, device, name,
    driver, modules}]"""
    out = []
    for line in text.splitlines():
        m = DEV.match(line)
        if m:
            out.append({"slot": m[1], "kind": m[2], "cls": m[3], "name": m[4], "vendor": m[5], "device": m[6],
                        "driver": "", "modules": []})
            continue
        if not out or not line.startswith(("\t", " ")):
            continue
        s = line.strip()
        if s.startswith("Kernel driver in use:"):
            out[-1]["driver"] = s.split(":", 1)[1].strip()
        elif s.startswith("Kernel modules:"):
            out[-1]["modules"] = [x.strip() for x in s.split(":", 1)[1].split(",") if x.strip()]
    return out


def parse_lsusb(text):
    """`lsusb`: the USB ids ("vvvv:pppp"), as fingerprint.find_reader takes them."""
    return {m[1].lower() for m in re.finditer(r"\bID ([0-9a-fA-F]{4}:[0-9a-fA-F]{4})\b", text)}


def parse_cpu(text):
    m = re.search(r"^vendor_id\s*:\s*(\S+)", text, re.M)
    return {"GenuineIntel": "intel", "AuthenticAMD": "amd"}.get(m[1] if m else "", "")


def edid_size(blob):
    """The screen's size from its EDID, in mm (bytes 21 and 22 are cm); None
    for projectors and EDIDs that leave it out."""
    if len(blob) < 23 or blob[:8] != b"\x00\xff\xff\xff\xff\xff\xff\x00":
        return None
    w, h = blob[21], blob[22]
    return (w * 10, h * 10) if w and h else None


SCALES = (1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0)


def suggest_scale(px_w, mm_w, internal):
    """A scale for a screen this sharp: a laptop's own screen is nearer the
    eyes (about 120 dpi feels like 1x) than a monitor on a desk (about 130)."""
    if not px_w or not mm_w:
        return 1.0
    dpi = px_w / (mm_w / 25.4)
    want = dpi / (120 if internal else 130)
    return min(SCALES, key=lambda s: abs(s - want)) if want > 1.12 else 1.0


def kernel_headers(installed):
    """The headers of each installed kernel (what DKMS drivers build against)."""
    kernels = [p for p in installed if re.fullmatch(r"linux(-[a-z0-9]+)*", p)
               and not p.endswith(("-headers", "-docs", "-api-headers", "-firmware", "-tools"))
               and not p.startswith("linux-firmware")]
    return sorted(k + "-headers" for k in kernels)


# ── The plan ────────────────────────────────────────────────────────────
def item(id, group, title, why, packages=(), aur=(), services=(), ticked=True, ask=False, locked=False):
    return {"id": id, "group": group, "title": title, "why": why, "packages": list(packages), "aur": list(aur),
            "services": list(services), "ticked": ticked, "ask": ask, "locked": locked}


def plan(f):
    """The items for these facts (collect()'s): only what's missing."""
    have = set(f.get("installed", ()))
    miss = lambda pkgs: [p for p in pkgs if p not in have]
    pci = f.get("pci", [])
    out = []
    G = GROUPS

    # Firmware
    if not any(p == "linux-firmware" or p.startswith("linux-firmware-") for p in have):
        out.append(item("firmware", G[0], "Linux firmware", "Wi-Fi, Bluetooth and graphics chips load their firmware from it",
                        ["linux-firmware"]))
    audio = [d for d in pci if d["cls"] in ("0401", "0403", "0480") and d["vendor"] == INTEL]
    sof = [d for d in audio if d["driver"].startswith("sof") or any(m.startswith("snd_sof") for m in d["modules"])]
    if sof and miss(["sof-firmware", "alsa-ucm-conf"]):
        out.append(item("sof", G[0], "Sound firmware (SOF)",
                        "This laptop's sound (" + sof[0]["name"] + ") needs Sound Open Firmware, or the speakers and mic stay silent",
                        miss(["sof-firmware", "alsa-ucm-conf"])))
    headers = kernel_headers(have)
    wifi = [d for d in pci if d["cls"] == "0280"]
    for d in wifi:
        if d["vendor"] == BROADCOM and not d["driver"] and not any(m in ("brcmfmac", "brcmsmac", "b43") for m in d["modules"]):
            # (no driver of its own in the kernel, bcma being only its bus)
            out.append(item("broadcom-wl", G[0], "Broadcom Wi-Fi driver",
                            d["name"] + " has no driver loaded; Broadcom's own (wl) drives it",
                            miss(["broadcom-wl-dkms"] + headers)))
            break

    # Graphics: video decoding and Vulkan for each GPU
    gpus = [d for d in pci if d["cls"] in ("0300", "0302", "0380")]
    vendors = {d["vendor"] for d in gpus}
    if INTEL in vendors and miss(["intel-media-driver", "vulkan-intel"]):
        out.append(item("gpu-intel", G[0], "Intel graphics: video decoding and Vulkan",
                        "Smooth video playback and recording on the GPU (VA-API), Vulkan for apps and games",
                        miss(["intel-media-driver", "vulkan-intel"])))
    if AMD in vendors and miss(["vulkan-radeon"]):
        out.append(item("gpu-amd", G[0], "AMD graphics: Vulkan", "Vulkan for apps and games (video decoding is in mesa)",
                        miss(["vulkan-radeon"])))
    nv = [d for d in gpus if d["vendor"] == NVIDIA]
    if nv:
        if any(d["driver"] == "nvidia" for d in nv):
            if miss(["libva-nvidia-driver"]):
                out.append(item("gpu-nvidia-video", G[0], "NVIDIA: video decoding",
                                "Video playback on the GPU through NVIDIA's driver (VA-API)", miss(["libva-nvidia-driver"])))
        else:
            # Never by default: a driver change is the one thing that can
            # leave a desktop without a picture
            out.append(item("gpu-nvidia", G[0], "NVIDIA driver (open kernel modules)",
                            nv[0]["name"] + " runs on nouveau now; NVIDIA's driver is faster and needed for many games. Changes how the screen is driven: only if you want it",
                            miss(["nvidia-open-dkms", "nvidia-utils", "libva-nvidia-driver"] + headers), ticked=False, ask=True))

    # Biometrics (each asked)
    fp = f.get("fingerprint")
    if fp and not fp.get("unsupported") and (fp.get("repo") or fp.get("aur")):
        out.append(item("fingerprint", G[1], "Fingerprint: " + fp["name"],
                        "Unlock and approve with a finger (set it up in Settings → Privacy & Security)"
                        + ("; its driver comes from the AUR" if fp.get("aur") else ""),
                        fp.get("repo", []), fp.get("aur", []), ask=True))
    cam = f.get("ir_camera")
    if cam and not have & {"howdy-next", "howdy", "howdy-git", "howdy-beta-git"}:
        out.append(item("face", G[1], "Face unlock: " + cam["name"],
                        "Unlock by looking at the infrared camera (Howdy, built from the AUR: a few minutes)",
                        [], ["howdy-next"], ticked=False, ask=True))

    # What the installer builds here (always, in its Building step)
    out.append(item("build-plugin", G[2], "Liquid Glass plugin for Hyprland " + (f.get("hyprland") or "(installed next)"),
                    "Compiled against this Hyprland, so the glass and window controls match it exactly", locked=True))
    out.append(item("build-native", G[2], "Native helpers for this Qt",
                    "Faster glass sync and requests than the fall-backs; built for the Qt installed here", locked=True))
    out.append(item("build-shaders", G[2], "Shaders", "Compiled once for this machine's Qt", locked=True))

    # Tuning
    laptop = f.get("laptop", False)
    pm = f.get("power_manager", "")
    # (power-profiles-daemon is one of iRice's packages, turned on with the
    # services: shown here, done there; never next to another power manager)
    if laptop and not pm and "power-profiles-daemon" not in have:
        out.append(item("power", G[3], "Energy modes", "Balanced, Power Saver and Performance from the menu bar; longer battery life",
                        ["power-profiles-daemon"], services=["power-profiles-daemon"], locked=True))
    if laptop and f.get("cpu") == "intel" and "thermald" not in have and not pm:
        out.append(item("thermald", G[3], "Intel thermal daemon",
                        "Keeps an Intel laptop cooler and quieter under load, before it has to slow down",
                        ["thermald"], services=["thermald"], ticked=False))
    # (only before Settings → Displays has its own: that's yours to change)
    for s in [] if f.get("monitors_set") else f.get("screens", []):
        if s.get("scale", 1.0) > 1.0:
            i = item("scale-" + s["name"], G[3], "Display scale {:g} for {}".format(s["scale"], s["name"]),
                     "{}×{} on a {:.1f}\" screen: text and controls at a comfortable size (Settings → Displays changes it)".format(
                         s["width"], s["height"], s["inches"]))
            i["scale"] = s["scale"]
            out.append(i)
    return out


def apply_lines(items, ids):
    """What install.sh does for the chosen items (only those)."""
    out = []
    for i in items:
        if i["id"] not in ids or i["locked"]:
            continue
        out += ["pkg " + p for p in i["packages"]]
        out += ["aur " + p for p in i["aur"]]
        out += ["svc " + s for s in i["services"]]
        if "scale" in i:
            out.append("scale {} {:g}".format(i["id"][len("scale-"):], i["scale"]))
    return out


# ── This computer ───────────────────────────────────────────────────────
def read(p, binary=False):
    """A file, by its path as found (already under IRICE_SYSTEM_ROOT)."""
    try:
        with open(p, "rb" if binary else "r") as f:
            return f.read()
    except OSError:
        return b"" if binary else ""


def installed_packages():
    return sorted({re.sub(r"-[^-]+-[^-]+$", "", os.path.basename(d)) for d in glob.glob(path("/var/lib/pacman/local/*-*-*"))})


def sysfs_pci():
    """The PCI devices, from /sys: the same fields as parse_lspci (no names)."""
    out = []
    for d in sorted(glob.glob(path("/sys/bus/pci/devices/*"))):
        cls = read(os.path.join(d, "class")).strip()
        vendor = read(os.path.join(d, "vendor")).strip().lower().removeprefix("0x")
        device = read(os.path.join(d, "device")).strip().lower().removeprefix("0x")
        if not cls or not vendor:
            continue
        drv = os.path.join(d, "driver")
        out.append({"slot": os.path.basename(d), "kind": "", "cls": cls.lower().removeprefix("0x")[:4],
                    "vendor": vendor, "device": device,
                    "name": "{} device {}:{}".format(VENDOR_NAMES.get(vendor, "PCI"), vendor, device),
                    "driver": os.path.basename(os.readlink(drv)) if os.path.islink(drv) else "", "modules": []})
    return out


def screens():
    out = []
    for d in sorted(glob.glob(path("/sys/class/drm/card*-*"))):
        if read(os.path.join(d, "status")).strip() != "connected":
            continue
        name = os.path.basename(d).split("-", 1)[1]
        size = edid_size(read(os.path.join(d, "edid"), binary=True))
        mode = read(os.path.join(d, "modes")).split("\n", 1)[0]
        m = re.match(r"(\d+)x(\d+)", mode)
        if not size or not m:
            continue
        w, h = int(m[1]), int(m[2])
        internal = name.startswith(("eDP", "LVDS", "DSI"))
        out.append({"name": name, "width": w, "height": h, "mm": size, "internal": internal,
                    "inches": (size[0] ** 2 + size[1] ** 2) ** 0.5 / 25.4, "scale": suggest_scale(w, size[0], internal)})
    return out


def collect(run=subprocess.run):
    have = installed_packages()
    pci = sysfs_pci()
    # lspci's names, and the modules that could drive each device (pciutils
    # isn't one of iRice's packages: without it, sysfs alone)
    if ROOT == "/":
        try:
            named = {d["slot"]: d for d in parse_lspci(run(["lspci", "-nnk"], capture_output=True, text=True, timeout=10).stdout)}
        except (OSError, subprocess.SubprocessError):
            named = {}
        for d in pci:
            n = named.get(d["slot"].split(":", 1)[1]) or named.get(d["slot"])
            if n:
                d.update(name=n["name"], kind=n["kind"], modules=n["modules"])
    supplies = glob.glob(path("/sys/class/power_supply/*/type"))
    # A laptop: a battery of its own (not a mouse's: those are "scope Device"), or a lid
    laptop = any(read(t).strip() == "Battery" and read(os.path.join(os.path.dirname(t), "scope")).strip() != "Device"
                 for t in supplies) or bool(glob.glob(path("/proc/acpi/button/lid/*")))
    fp = None
    try:
        fpm = load("fingerprint")
        reader = fpm.find_reader()
        if reader:
            repo, aur = fpm.needs(reader)
            fp = dict(reader, repo=repo, aur=aur)
    except Exception:  # (an older shell's script: no reader shown, nothing breaks)
        fp = None
    try:
        cam = load("face").ir_camera()
    except Exception:
        cam = None
    # What else runs the network when NetworkManager doesn't (the installer
    # asks before switching), and the login manager in use
    def active(unit):
        try:
            return run(["systemctl", "is-active", "--quiet", unit], timeout=5).returncode == 0
        except (OSError, subprocess.SubprocessError):
            return False
    network = ""
    if ROOT == "/" and not active("NetworkManager"):
        network = next((u for u in ("systemd-networkd", "dhcpcd", "iwd") if active(u)), "")
    try:
        dm = os.path.basename(os.readlink(path("/etc/systemd/system/display-manager.service"))).removesuffix(".service")
    except OSError:
        dm = ""
    hv = ""
    m = re.search(r"/hyprland-([0-9.]+)", " ".join(glob.glob(path("/var/lib/pacman/local/hyprland-[0-9]*"))))
    if m:
        hv = m[1].rstrip(".")
    return {
        "cpu": parse_cpu(read(path("/proc/cpuinfo"))),
        "pci": pci,
        "laptop": laptop,
        "installed": have,
        "power_manager": next((p for p in POWER_MANAGERS if p in have), ""),
        "fingerprint": fp,
        "ir_camera": cam,
        "screens": screens(),
        "monitors_set": os.path.exists(os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"),
                                                    "irice", "monitors.lua")),
        "hyprland": hv,
        "pulseaudio": "pulseaudio" in have,
        "network": network,
        "display_manager": "" if dm == "greetd" else dm,
    }


def main(argv):
    facts = None
    if argv[:1] == ["--facts"] and len(argv) > 1:
        with open(argv[1]) as f:
            facts = json.load(f)
        argv = argv[2:]
    if not argv or argv[0] not in ("plan", "apply"):
        print(__doc__, file=sys.stderr)
        return 2
    facts = collect() if facts is None else facts
    items = plan(facts)
    if argv[0] == "apply":
        for line in apply_lines(items, set(argv[1:])):
            print(line)
        return 0
    if argv[1:] == ["--json"]:
        print(json.dumps({"facts": {k: v for k, v in facts.items() if k != "installed"}, "groups": GROUPS, "items": items}, indent=1))
        return 0
    for i in items:
        print("\t".join([i["id"], "1" if i["ticked"] else "0", "1" if i["ask"] else "0", "1" if i["locked"] else "0",
                         i["group"], i["title"], i["why"], " ".join(i["packages"]), " ".join(i["aur"])]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
