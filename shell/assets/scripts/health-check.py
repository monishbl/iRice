#!/usr/bin/env python3
"""System health for iRice (Settings → About → System health).

Each check asks the service itself, not whether its package is installed:
PipeWire answers pw-cli (and its PulseAudio part pactl), NetworkManager reports itself running, Avahi gives
its version, the glass plugin answers a ping. They run side by side, each
given 2 s and all of them 3 s, so a hung daemon shows up as "didn't answer"
instead of holding the page.

  health-check.py [--shell-pid N] [--polkit yes|no|off] [--google]
        every check, as JSON: [{id, name, group, status, detail, fix, canFix}]
        status: ok | warn (works, but something's off) | fail | off (turned
        off on purpose, or not used)
  health-check.py text [same options]
        the same as plain text, scrubbed like a bug report (for Copy report)
  health-check.py fix <id>
        the fix for a check, if it has one, from a fixed list (nothing
        free-form ever runs): restarting a user service runs directly;
        starting a system service runs as root through pkexec, only after
        the page showed the exact command and the person pressed the button
        for it: through iRice's root helper (/usr/lib/irice/irice-root fix
        <id>, no password) once an update installed it, and until then the
        command itself, after the password in iRice's own dialog.

Nothing runs until the page asks: there's no background polling.
"""
import argparse
import concurrent.futures
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
SHELL = os.path.dirname(os.path.dirname(HERE))
REPO = os.path.dirname(SHELL)
EACH = 2.0   # seconds a check may take
TOTAL = 3.0  # seconds for all of them together
PLUGIN_VERSION = 4  # what `hyprctl irice-glass ping` answers when current

GROUPS = ("Sound", "Network & sharing", "Power", "Desktop", "iRice itself")

# The fixes this script will run, and nothing else: user services directly,
# root ones (ROOT_FIXES, below) through pkexec
FIXES = {
    "pipewire": ["systemctl", "--user", "restart", "pipewire", "pipewire-pulse", "wireplumber"],
    "wireplumber": ["systemctl", "--user", "restart", "wireplumber"],
    "portal": ["systemctl", "--user", "restart", "xdg-desktop-portal", "xdg-desktop-portal-hyprland"],
}
# Sharing's own checks (firewall, Bluetooth pairing prompts), and the root
# fixes: the fixed list in the root helper (irice-root.py)
_spec = importlib.util.spec_from_file_location("sharing_check", os.path.join(HERE, "sharing-check.py"))
sharing = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sharing)
ROOT_FIXES = sharing.irice_root.FIXES
SHOWN = {"firewall": sharing.FIREWALL_SHOWN, "btpairing": sharing.PAIRING_SHOWN,  # (what the page shows for these)
         "networkmanager": sharing.irice_root.NETWORK_SHOWN}
SYSTEM = os.environ.get("IRICE_SYSTEM_ROOT", "/")  # where /usr is looked for (tests)

_local = threading.local() # per check: whether something it asked hung


def run(cmd, timeout=EACH):
    """(exit code, stdout) of a command; (None, why) if it couldn't run."""
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return p.returncode, (p.stdout or "") + ("" if p.stdout else p.stderr or "")
    except FileNotFoundError:
        return None, "not installed"
    except subprocess.TimeoutExpired:
        _local.hung = True
        return None, "didn't answer in time"
    except OSError as e:
        return None, str(e)


def result(id, name, group, status, detail="", fix=""):
    broken = status in ("fail", "warn")
    root = broken and id in ROOT_FIXES
    return {"id": id, "name": name, "group": group, "status": status, "detail": detail, "fix": fix,
            "canFix": broken and (id in FIXES or root), "fixRoot": root,
            # a root fix asks for the password (no root helper installed yet)
            "fixPassword": root and not sharing.helper_installed(),
            # what the button will run, shown before a root fix is confirmed
            "fixCommand": (SHOWN.get(id) or " ".join(ROOT_FIXES[id])) if root else ""}


def bus_value(out):
    """The value from `busctl get-property` output: `s "x"` → x, `b true` → True."""
    out = out.strip()
    m = re.match(r'^[a-z]+\s+(.*)$', out)
    v = m.group(1) if m else out
    if v in ("true", "false"):
        return v == "true"
    return v.strip('"')


# ── Sound ───────────────────────────────────────────────────────────────

def check_pipewire(_):
    # PipeWire itself first: without pipewire-pulse (apps that speak
    # PulseAudio, pactl) it still plays sound, so that's a warning, not a failure
    code, out = run(["pw-cli", "info", "0"])
    if code == 0 and "PipeWire:Interface:Core" in out:
        version = re.search(r'^\W*version:\s*"([^"]+)"', out, re.M)
        name = f"PipeWire {version.group(1)}" if version else "PipeWire"
        pcode, pout = run(["pactl", "info"])
        if pcode == 0 and "Server" in pout:
            return result("pipewire", "PipeWire", "Sound", "ok", name)
        return result("pipewire", "PipeWire", "Sound", "warn",
                      f"{name} runs, but its PulseAudio part (pipewire-pulse) doesn't answer: most apps and the volume controls use it",
                      "systemctl --user restart pipewire pipewire-pulse wireplumber (sudo pacman -S pipewire-pulse if it's missing)")
    code, out = run(["pactl", "info"])
    if code == 0 and "Server" in out:
        name = re.search(r"Server Name: (.*)", out)
        return result("pipewire", "PipeWire", "Sound", "ok", name.group(1).strip() if name else "Running")
    return result("pipewire", "PipeWire", "Sound", "fail", f"Sound server not answering ({out.strip()[:80]})",
                  "systemctl --user restart pipewire pipewire-pulse wireplumber")


def check_wireplumber(_):
    code, out = run(["systemctl", "--user", "is-active", "wireplumber"])
    if code == 0 and out.strip() == "active":
        return result("wireplumber", "WirePlumber", "Sound", "ok", "Running")
    if code is None and out == "not installed":
        return result("wireplumber", "WirePlumber", "Sound", "warn", "Couldn't ask systemd", "systemctl --user status wireplumber")
    return result("wireplumber", "WirePlumber", "Sound", "fail", "Not running: devices and volume won't follow",
                  "systemctl --user restart wireplumber")


# ── Network & sharing ───────────────────────────────────────────────────

def active(unit):
    code, out = run(["systemctl", "is-active", unit])
    return code == 0 and out.strip() == "active"


def check_networkmanager(_):
    code, out = run(["nmcli", "-t", "-f", "RUNNING,STATE", "general"])
    if code == 0 and out.startswith("running"):
        state = out.strip().split(":", 1)[-1]
        return result("networkmanager", "NetworkManager", "Network & sharing", "ok" if state.startswith("connected") else "warn",
                      "Connected" if state.startswith("connected") else f"Running, {state}")
    # Something else runs the network on purpose: switching is a choice (the
    # installer offers it), not a fix. (NetworkManager turned on but stopped
    # is still broken, even with iwd under it.)
    # (nmcli hanging means NetworkManager is there, just stuck)
    if not getattr(_local, "hung", False) and run(["systemctl", "is-enabled", "--quiet", "NetworkManager"])[0] != 0:
        others = [u for u in ("iwd", "systemd-networkd") if active(u)]
        if others:
            return result("networkmanager", "NetworkManager", "Network & sharing", "off",
                          f"Network managed by {'/'.join(others)}; iRice's Wi-Fi menus need NetworkManager")
    return result("networkmanager", "NetworkManager", "Network & sharing", "fail", "Not running: Wi-Fi and the network menu won't work",
                  "sudo systemctl enable --now NetworkManager")


def adapters(tree):
    """`busctl tree --list org.bluez` → the adapters' object paths (/org/bluez/hciN)."""
    return sorted(p for p in tree.split() if re.fullmatch(r"/org/bluez/hci\d+", p))


def check_bluetooth(_):
    name = "Bluetooth"
    code, out = run(["busctl", "--system", "tree", "--list", "org.bluez"])
    if code == 0:
        found = adapters(out)
        if not found:
            return result("bluetooth", name, "Network & sharing", "off", "No Bluetooth adapter found")
        for path in found:
            c, v = run(["busctl", "--system", "get-property", "org.bluez", path, "org.bluez.Adapter1", "Powered"])
            if c == 0 and bus_value(v) is True:
                return result("bluetooth", name, "Network & sharing", "ok", "On")
        return result("bluetooth", name, "Network & sharing", "off", "Off. Quick Share needs it to find phones without Wi-Fi",
                      "Turn it on in Control Center or Settings → Bluetooth")
    if not any(os.path.exists(os.path.join(SYSTEM, d, "bluetooth/bluetoothd")) for d in ("usr/lib", "usr/libexec")) and \
            not shutil.which("bluetoothd"):
        r = result("bluetooth", name, "Network & sharing", "fail", "BlueZ (the Bluetooth service) isn't installed",
                   "sudo pacman -S bluez bluez-utils, then sudo systemctl enable --now bluetooth")
        r.update(canFix=False, fixRoot=False, fixPassword=False, fixCommand="")  # (installing isn't one of the fixes)
        return r
    return result("bluetooth", name, "Network & sharing", "fail", "The Bluetooth service isn't running",
                  "sudo systemctl enable --now bluetooth")


def check_avahi(_):
    code, out = run(["busctl", "--system", "call", "org.freedesktop.Avahi", "/", "org.freedesktop.Avahi.Server", "GetVersionString"])
    if code == 0:
        return result("avahi", "Avahi (finding nearby devices)", "Network & sharing", "ok", bus_value(out))
    return result("avahi", "Avahi (finding nearby devices)", "Network & sharing", "fail",
                  "Not running: LocalSend and Quick Share can't find devices on your network",
                  "sudo systemctl enable --now avahi-daemon")


def check_firewall(_):
    f = sharing.firewall()
    name = "Firewall lets sharing through"
    if f["kind"] is None:
        return result("firewall", name, "Network & sharing", "ok", "No firewall turned on")
    if not f["blocks"] and f.get("hotspot") is not False:
        return result("firewall", name, "Network & sharing", "ok", f"{f['kind']}: sharing's ports are open" +
                      (" until the next restart (the Fix button opens them again)" if f.get("runtime") else ""))
    if not f["blocks"]:
        detail = (f"{f['kind']} keeps phones from joining Quick Share's hotspot (DHCP, port 67): "
                  "transfers without a shared Wi-Fi stay slow")
    else:
        detail = (f"{f['kind']} is on" + (" and blocks" if f["allowed"] is False else " and may block") +
                  " LocalSend and Quick Share (ports 53317-53320, mDNS, DHCP)")
    r = result("firewall", name, "Network & sharing", "warn", detail,
               "Allow them: Settings → Sharing, or the Fix button here")
    if r["fixRoot"] and f["kind"] in sharing.FIREWALL_SHOWN_BY:
        r["fixCommand"] = sharing.FIREWALL_SHOWN_BY[f["kind"]]
    return r


def check_btpairing(_):
    p = sharing.pairing()
    name = "Bluetooth sharing without pairing"
    if not p["risk"]:
        return result("btpairing", name, "Network & sharing", "ok",
                      "Phones connect for Quick Share without pairing")
    return result("btpairing", name, "Network & sharing", "warn",
                  "BlueZ's experimental Ranging and LE Audio control profiles make phones ask to pair when they send with Quick Share",
                  "Turn those profiles off (headphone batteries keep working): the Fix button, which restarts Bluetooth")


def check_portal(_):
    code, out = run(["busctl", "--user", "get-property", "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                     "org.freedesktop.portal.FileChooser", "version"])
    if code != 0:
        return result("portal", "Desktop portal", "Desktop", "fail",
                      "Not answering: file pickers, screen sharing and screenshots from apps won't open",
                      "systemctl --user restart xdg-desktop-portal xdg-desktop-portal-hyprland")
    code2, names = run(["busctl", "--user", "list", "--activatable"])
    if code2 == 0 and "org.freedesktop.impl.portal.desktop.hyprland" not in names:
        return result("portal", "Desktop portal", "Desktop", "warn", "The Hyprland portal isn't installed: screen sharing won't work",
                      "sudo pacman -S xdg-desktop-portal-hyprland")
    return result("portal", "Desktop portal", "Desktop", "ok", f"File chooser version {bus_value(out)}")


def check_goa(opts):
    if not opts.google:
        return result("goa", "GNOME Online Accounts", "Network & sharing", "off", "Google isn't connected")
    code, out = run(["busctl", "--user", "list", "--activatable"])
    if code == 0 and "org.gnome.OnlineAccounts" in out:
        return result("goa", "GNOME Online Accounts", "Network & sharing", "ok", "Available")
    return result("goa", "GNOME Online Accounts", "Network & sharing", "fail", "Not installed: Google sign-in needs it",
                  "sudo pacman -S gnome-online-accounts")


def check_geoclue(_):
    code, out = run(["busctl", "--system", "list", "--activatable"])
    if code == 0 and "org.freedesktop.GeoClue2" in out:
        return result("geoclue", "Location (GeoClue)", "Network & sharing", "ok", "Available")
    return result("geoclue", "Location (GeoClue)", "Network & sharing", "warn",
                  "Not installed: weather uses your network's rough location instead", "sudo pacman -S geoclue")


# ── Power ───────────────────────────────────────────────────────────────

def check_upower(_):
    code, out = run(["busctl", "--system", "get-property", "org.freedesktop.UPower", "/org/freedesktop/UPower",
                     "org.freedesktop.UPower", "DaemonVersion"])
    if code == 0:
        return result("upower", "UPower (battery)", "Power", "ok", f"Version {bus_value(out)}")
    return result("upower", "UPower (battery)", "Power", "fail", "Not answering: battery level and charging won't show",
                  "sudo systemctl enable --now upower")


# Power managers that power-profiles-daemon would fight with (iRice leaves it
# out next to them: install.sh, tools/irice-packages.sh)
POWER_MANAGERS = (("tlp", "TLP"), ("auto-cpufreq", "auto-cpufreq"), ("tuned", "tuned"))


def power_manager():
    for unit, name in POWER_MANAGERS:
        if any(os.path.exists(os.path.join(SYSTEM, d, unit)) for d in ("usr/bin", "usr/local/bin")) or active(unit):
            return name
    return ""


def check_profiles(_):
    for name, path, iface in (("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles", "org.freedesktop.UPower.PowerProfiles"),
                              ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles", "net.hadess.PowerProfiles")):
        code, out = run(["busctl", "--system", "get-property", name, path, iface, "ActiveProfile"])
        if code == 0:
            return result("profiles", "Energy modes", "Power", "ok", bus_value(out).replace("-", " ").capitalize())
    other = power_manager()
    if other:
        return result("profiles", "Energy modes", "Power", "off", f"Managed by {other}")
    return result("profiles", "Energy modes", "Power", "fail", "power-profiles-daemon isn't running: Low Power and Performance won't work",
                  "sudo systemctl enable --now power-profiles-daemon")


def check_hypridle(_):
    code, out = run(["pgrep", "-u", str(os.getuid()), "-x", "hypridle"])
    if code == 0 and out.strip():
        return result("hypridle", "Idle and lock", "Power", "ok", "Running")
    return result("hypridle", "Idle and lock", "Power", "fail",
                  "Not running: the screen won't dim, lock or sleep when you're away",
                  "Log out and in again; if it stays, run hypridle from a terminal to see why")


# ── Desktop ─────────────────────────────────────────────────────────────

def check_notifications(opts):
    code, out = run(["busctl", "--user", "call", "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                     "GetConnectionUnixProcessID", "s", "org.freedesktop.Notifications"])
    if code != 0:
        return result("notifications", "Notifications", "Desktop", "fail", "Nothing is receiving notifications",
                      "Restart iRice: Super+Ctrl+Escape")
    m = re.search(r"(\d+)\s*$", out.strip())
    pid = int(m.group(1)) if m else 0
    owner = ""
    try:
        owner = open(f"/proc/{pid}/comm").read().strip()
    except OSError:
        pass
    # (the shell itself, or, asked from somewhere else, a Quickshell)
    if (opts.shell_pid and pid == opts.shell_pid) or owner in ("qs", "quickshell"):
        return result("notifications", "Notifications", "Desktop", "ok", "iRice shows them")
    return result("notifications", "Notifications", "Desktop", "warn",
                  f"Another program ({owner or 'unknown'}) is showing notifications instead of iRice",
                  f"Stop it from starting with your session (pid {pid}), then restart iRice")


def check_polkit(opts):
    if opts.polkit == "off":
        return result("polkit", "Password prompts", "Desktop", "off", "Not used in this session")
    if opts.polkit == "yes":
        return result("polkit", "Password prompts", "Desktop", "ok", "iRice asks for your password")
    return result("polkit", "Password prompts", "Desktop", "fail",
                  "iRice's password prompt isn't registered: apps asking for admin rights will fail",
                  "Close any other polkit agent (polkit-gnome, polkit-kde), then restart iRice: Super+Ctrl+Escape")


def check_clipboard(_):
    if not shutil.which("cliphist"):
        return result("clipboard", "Clipboard history", "Desktop", "fail", "cliphist isn't installed", "sudo pacman -S cliphist")
    code, out = run(["pgrep", "-u", str(os.getuid()), "-f", "wl-paste --watch cliphist"])
    if code == 0 and out.strip():
        return result("clipboard", "Clipboard history", "Desktop", "ok", "Recording")
    return result("clipboard", "Clipboard history", "Desktop", "fail", "Nothing is recording what you copy",
                  "Log out and in again (the session starts it)")


def check_recorder(_):
    if shutil.which("gpu-screen-recorder"):
        return result("recorder", "Screen recording", "Desktop", "ok", "gpu-screen-recorder")
    if shutil.which("wf-recorder"):
        return result("recorder", "Screen recording", "Desktop", "warn", "Using wf-recorder: no pause, one sound source",
                      "sudo pacman -S gpu-screen-recorder")
    return result("recorder", "Screen recording", "Desktop", "fail", "No screen recorder installed", "sudo pacman -S gpu-screen-recorder")


def check_disk(_):
    path = os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"))
    try:
        free = shutil.disk_usage(path if os.path.exists(path) else os.path.expanduser("~")).free
    except OSError as e:
        return result("disk", "Disk space", "Desktop", "warn", str(e))
    gb = free / 1024 ** 3
    if gb < 0.5:
        return result("disk", "Disk space", "Desktop", "fail", f"{gb:.1f} GB free: settings and history may fail to save",
                      "Free some space (empty the Trash, or sudo pacman -Sc for old packages)")
    if gb < 2:
        return result("disk", "Disk space", "Desktop", "warn", f"{gb:.1f} GB free", "Free some space soon")
    return result("disk", "Disk space", "Desktop", "ok", f"{gb:.0f} GB free")


# ── iRice itself ────────────────────────────────────────────────────────

def source():
    """The iRice folder: this one, or (an installed copy of the shell) the one
    the installer recorded."""
    if os.path.isfile(os.path.join(REPO, "compat.json")):
        return REPO
    for base in (os.environ.get("XDG_DATA_HOME", ""), os.path.expanduser("~/.local/share")):
        try:
            src = open(os.path.join(base, "irice", "source")).read().strip() if base else ""
        except OSError:
            continue
        if src and os.path.isfile(os.path.join(src, "compat.json")):
            return src
    return REPO


def compat():
    try:
        return json.load(open(os.path.join(source(), "compat.json")))
    except (OSError, ValueError):
        return {}


def version_tuple(v):
    return tuple(int(x) for x in re.findall(r"\d+", v)[:3])


def check_hyprland(_):
    code, out = run(["hyprctl", "version", "-j"])
    try:
        tag = json.loads(out).get("tag", "") if code == 0 else ""
    except ValueError:
        tag = ""
    if not tag:
        return result("hyprland", "Hyprland", "iRice itself", "fail", "Couldn't ask Hyprland", "Is this a Hyprland session?")
    rng = compat().get("hyprland", {})
    v = version_tuple(tag)
    lo = version_tuple(rng.get("min", "0"))
    hi = rng.get("max", "")
    # "0.56.x": any 0.56 release; otherwise an exact upper bound
    if hi.endswith(".x"):
        top = version_tuple(hi[:-2])
        ok = v >= lo and v[:len(top)] <= top
    else:
        ok = v >= lo and (not hi or v <= version_tuple(hi))
    if ok:
        return result("hyprland", "Hyprland", "iRice itself", "ok", tag.lstrip("v"))
    if v >= lo:
        return result("hyprland", "Hyprland", "iRice itself", "warn",
                      f"{tag.lstrip('v')} is newer than iRice is tested with ({hi}): the glass or some settings may not work",
                      "Run Software Update when an iRice that supports it is out")
    return result("hyprland", "Hyprland", "iRice itself", "warn",
                  f"{tag.lstrip('v')} is older than iRice needs ({rng.get('min', '?')})",
                  "Run Software Update (it updates the system)")


def check_plugin(_):
    code, out = run(["hyprctl", "irice-glass", "ping"])
    m = re.search(r"irice-glass (\d+)", out or "")
    if code == 0 and m:
        n = int(m.group(1))
        if n >= PLUGIN_VERSION:
            return result("plugin", "Liquid Glass plugin", "iRice itself", "ok", f"Loaded (version {n})")
        return result("plugin", "Liquid Glass plugin", "iRice itself", "warn",
                      f"An older build is loaded (version {n}): pointer and focus fixes need the new one",
                      "Run Software Update, or rebuild it: irice rebuild plugin; then log out and in")
    return result("plugin", "Liquid Glass plugin", "iRice itself", "fail", "Not loaded: glass falls back to frosted",
                  "Settings → Appearance → Glass → Build plugin, or: irice rebuild plugin")


def check_native(_):
    build = os.path.join(SHELL, "native", "build")
    have = os.path.isdir(os.path.join(build, "qml", "IRice", "Native")) and os.path.exists(os.path.join(build, "glass-ipc"))
    if have:
        return result("native", "Native helpers", "iRice itself", "ok", "Built")
    return result("native", "Native helpers", "iRice itself", "warn", "Not built: glass updates use a slower path",
                  "irice rebuild native")


def check_quickshell(opts):
    exe = ""
    try:
        exe = os.readlink(f"/proc/{opts.shell_pid}/exe") if opts.shell_pid else ""
    except OSError:
        pass
    if os.path.basename(exe) not in ("qs", "quickshell"):
        bundled = os.path.expanduser("~/.local/share/irice/quickshell/bin/qs")
        exe = bundled if os.access(bundled, os.X_OK) else (shutil.which("qs") or "qs")
    code, out = run([exe, "--version"])
    m = re.search(r"Quickshell (\d+\.\d+\.\d+)", out or "")
    qs = compat().get("quickshell", {})
    want, least = qs.get("bundled", ""), qs.get("min", "") or qs.get("bundled", "")
    if not m:
        return result("quickshell", "Quickshell", "iRice itself", "warn", "Couldn't read its version")
    # (Software Update doesn't build Quickshell: the fix is the build script, by hand)
    build = os.path.join(source(), "tools", "build-quickshell.sh") + (f" v{want}" if want else "")
    restart = ", then restart iRice: Super+Ctrl+Escape"
    have = m.group(1)
    if "/irice/quickshell/" in exe:  # iRice's own build
        if want and have != want:
            return result("quickshell", "Quickshell", "iRice itself", "warn", f"iRice's own build is {have}; this iRice expects {want}",
                          "Rebuild it: " + build + restart)
        return result("quickshell", "Quickshell", "iRice itself", "ok", f"{have} (iRice's own build)")
    if least and version_tuple(have) < version_tuple(least):
        return result("quickshell", "Quickshell", "iRice itself", "warn", f"{have}, older than iRice needs ({least})",
                      "Build iRice its own: " + build + restart)
    return result("quickshell", "Quickshell", "iRice itself", "ok", have)


def check_watchers(_):
    code, out = run([sys.executable, "-I", os.path.join(HERE, "process-identity.py"), "reap", "--dry-run"])
    lines = [l for l in (out or "").splitlines() if l.strip()]
    if code is None:
        return result("watchers", "Background helpers", "iRice itself", "warn", out)
    if not lines:
        return result("watchers", "Background helpers", "iRice itself", "ok", "None left behind")
    return result("watchers", "Background helpers", "iRice itself", "warn",
                  f"{len(lines)} left behind by an earlier shell (they use battery)",
                  "Restart iRice (Super+Ctrl+Escape): it ends them as it starts")


CHECKS = [check_pipewire, check_wireplumber, check_networkmanager, check_bluetooth, check_avahi, check_firewall,
          check_btpairing, check_goa, check_geoclue,
          check_upower, check_profiles, check_hypridle, check_portal, check_notifications, check_polkit, check_clipboard,
          check_recorder, check_disk, check_hyprland, check_plugin, check_native, check_quickshell, check_watchers]


def check_all(opts):
    out = {}
    pool = concurrent.futures.ThreadPoolExecutor(max_workers=len(CHECKS))
    def guarded(c):
        # A service that didn't answer is hung rather than missing: say so
        # instead of the check's "not running"
        _local.hung = False
        r = c(opts)
        if _local.hung and r["status"] in ("fail", "warn"):
            r["status"] = "fail"
            r["detail"] = f"Didn't answer within {EACH:.0f} s: it may be hung"
        return r
    futures = {pool.submit(guarded, c): c for c in CHECKS}
    try:
        for f in concurrent.futures.as_completed(futures, timeout=TOTAL):
            try:
                r = f.result()
            except Exception as e:  # a check's own bug mustn't hide the others
                name = futures[f].__name__.removeprefix("check_")
                r = result(name, name, "iRice itself", "warn", f"Check failed: {e}")
            out[futures[f]] = r
    except concurrent.futures.TimeoutError:
        pass
    pool.shutdown(wait=False, cancel_futures=True)
    rows = []
    for c in CHECKS:
        if c in out:
            rows.append(out[c])
        else:
            name = c.__name__.removeprefix("check_")
            rows.append(result(name, name, "iRice itself", "warn", "Didn't answer in time"))
    return rows


def as_text(rows):
    spec = importlib.util.spec_from_file_location("bug_report", os.path.join(HERE, "bug-report.py"))
    bug = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bug)
    bad = [r for r in rows if r["status"] in ("fail", "warn")]
    lines = ["iRice system health: " + ("all good" if not bad else f"{len(bad)} need attention"), ""]
    for g in GROUPS:
        lines.append(g)
        for r in rows:
            if r["group"] == g:
                lines.append(f"  [{r['status']}] {r['name']}: {r['detail']}" + (f" (fix: {r['fix']})" if r["fix"] and r["status"] in ("fail", "warn") else ""))
        lines.append("")
    return bug.scrub("\n".join(lines), bug.personal_names())


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", nargs="?", default="check", choices=["check", "text", "fix"])
    ap.add_argument("id", nargs="?")
    ap.add_argument("--shell-pid", type=int, default=os.getppid())
    ap.add_argument("--polkit", choices=["yes", "no", "off"], default="no")
    ap.add_argument("--google", action="store_true")
    opts = ap.parse_args(argv)
    if opts.cmd == "fix":
        if opts.id in FIXES:
            cmd = FIXES[opts.id]
        elif opts.id in ROOT_FIXES:
            cmd = ["pkexec"] + ROOT_FIXES[opts.id]
            if sharing.helper_installed():
                cmd = ["pkexec", sharing.HELPER, "fix", opts.id]
        else:
            print(json.dumps({"ok": False, "error": "There's no automatic fix for that; run the command shown."}))
            return 1
        try:
            p = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
            if cmd[1:2] == [sharing.HELPER] and (p.returncode == sharing.irice_root.REFUSED_EXIT or
                                                 (opts.id == "firewall" and sharing.older_firewall_fix(p))):
                # (an older helper without this fix: the command itself, with the password)
                cmd = ["pkexec"] + ROOT_FIXES[opts.id]
                p = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
            code, out, said = p.returncode, (p.stderr or p.stdout or ""), p.stdout or ""
        except FileNotFoundError:
            code, out, said = None, f"{cmd[0]} isn't installed", ""
        except subprocess.TimeoutExpired:
            code, out, said = None, "It took too long; try the command in a terminal", ""
        if cmd[0] == "pkexec" and code in (126, 127):
            err = "Cancelled: no password was given" if code == 126 else "Not allowed: your account can't do this; try the command in a terminal"
        else:
            err = "" if code == 0 else (out.strip()[:200] or "It didn't work; try the command in a terminal")
        note = ""
        if code == 0 and opts.id == "firewall":
            # (the rules can't be read without root: remember they're in, and for how long)
            note = sharing.remember(sharing.firewall()["kind"], said)
        print(json.dumps({"ok": code == 0, "error": err, "note": note}))
        return 0 if code == 0 else 1
    rows = check_all(opts)
    print(as_text(rows) if opts.cmd == "text" else json.dumps(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
