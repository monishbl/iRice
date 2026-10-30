#!/usr/bin/env python3
"""Starts the apps that asked to start with your session (XDG autostart:
~/.config/autostart and /etc/xdg/autostart), once per session. Hyprland
doesn't do this itself; hypr/irice/execs.lua runs this at login.

  xdg-autostart.py [--wait-tray SECONDS] [--dry-run]

Which entries, as the XDG autostart spec has it: your own folder overrides
the system's by file name (and earlier XDG_CONFIG_DIRS the later ones);
Hidden=true or X-GNOME-Autostart-enabled=false turns one off; OnlyShowIn and
NotShowIn are matched against this desktop (Hyprland, iRice, and
XDG_CURRENT_DESKTOP); a TryExec that isn't installed skips it. Left out as
well: what iRice already runs or would fight with (DENY below: the keyring,
polkit agents, notification daemons, other bars and shells, idle daemons,
PipeWire's own autostart). Each one starts detached (setsid, like apps from
the shell), in its Path=, in a terminal when Terminal=true.

The first run in a session takes a lock in XDG_RUNTIME_DIR (by
HYPRLAND_INSTANCE_SIGNATURE), so a reload or a second call starts nothing.
--wait-tray waits up to that long for the shell's tray
(org.kde.StatusNotifierWatcher), so tray apps find it; --dry-run prints what
would start, as JSON, without the lock.
"""
import argparse
import json
import os
import shlex
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
DESKTOPS = {"hyprland", "irice"}
# Entries iRice runs itself or that would conflict, matched against the file
# name (without .desktop) and the program's name; "x*" matches anything
# starting with x, "*x*" anything with x in it
DENY = (
    "gnome-keyring*",  # (started at login: PAM, or execs.lua)
    "*polkit*",  # (any polkit agent: iRice is one)
    "mako", "dunst", "swaync", "fnott", "xfce4-notifyd", "notify-osd", "org.kde.plasma.notifications",
    "waybar", "ags", "astal", "eww", "hyprpanel", "nwg-panel", "nwg-dock*", "ironbar", "quickshell", "qs",
    "org.kde.plasmashell", "plasmashell", "xembedsniproxy", "gnome-shell*", "org.gnome.shell*", "lxqt-panel", "xfce4-panel",
    "hypridle", "swayidle", "xscreensaver",
    "pipewire*", "wireplumber",
)


def matches(name, pattern):
    name, pattern = name.lower(), pattern.lower()
    if pattern.startswith("*") and pattern.endswith("*"):
        return pattern.strip("*") in name
    if pattern.endswith("*"):
        return name.startswith(pattern[:-1])
    return name == pattern


def denied(stem, exe):
    return any(matches(stem, p) or (exe and matches(exe, p)) for p in DENY)


def dirs():
    """The autostart folders, most important first."""
    home = os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")
    system = [d for d in (os.environ.get("XDG_CONFIG_DIRS") or "/etc/xdg").split(":") if d]
    return [os.path.join(d, "autostart") for d in [home] + system]


def entries():
    """{file name: path}, the first folder holding a name winning."""
    out = {}
    for d in dirs():
        try:
            names = sorted(os.listdir(d))
        except OSError:
            continue
        for n in names:
            if n.endswith(".desktop") and n not in out:
                out[n] = os.path.join(d, n)
    return out


def unescape(value):
    """A desktop entry string value: \\s \\n \\t \\r \\\\."""
    out, i = [], 0
    while i < len(value):
        c = value[i]
        if c == "\\" and i + 1 < len(value):
            out.append({"s": " ", "n": "\n", "t": "\t", "r": "\r", "\\": "\\"}.get(value[i + 1], "\\" + value[i + 1]))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def parse(path):
    """The [Desktop Entry] group's keys (no localised ones)."""
    keys, group = {}, None
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if line.startswith("["):
                    group = line
                    continue
                if group == "[Desktop Entry]" and "=" in line:
                    k, _, v = line.partition("=")
                    k = k.strip()
                    if "[" not in k:
                        keys.setdefault(k, v.strip())
    except OSError:
        return {}
    return keys


def true(v):
    return (v or "").strip().lower() == "true"


def listed(v):
    return {x.strip().lower() for x in (v or "").split(";") if x.strip()}


def installed(prog):
    if not prog:
        return False
    if "/" in prog:
        return os.access(prog, os.X_OK)
    return shutil.which(prog) is not None


def command(keys, path):
    """Exec as an argument list: quoted as the spec says (shlex handles its
    double quotes and backslashes), field codes filled in or dropped."""
    try:
        args = shlex.split(unescape(keys.get("Exec", "")))
    except ValueError:
        return []
    out = []
    for a in args:
        if a in ("%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%v", "%m"):
            continue
        if a == "%i":
            if keys.get("Icon"):
                out += ["--icon", keys["Icon"]]
            continue
        a = a.replace("%c", keys.get("Name", "")).replace("%k", path)
        for code in ("%f", "%F", "%u", "%U"):
            a = a.replace(code, "")
        out.append(a.replace("%%", "%"))
    return out


def desktops():
    return DESKTOPS | {d.strip().lower() for d in os.environ.get("XDG_CURRENT_DESKTOP", "").split(":") if d.strip()}


def plan():
    """[(file name, argv, cwd, terminal)] to start, and {file name: why not} for the rest."""
    here = desktops()
    start, skipped = [], {}
    for name, path in entries().items():
        keys = parse(path)
        stem = name[:-len(".desktop")]
        argv = command(keys, path)
        why = ""
        if keys.get("Type", "Application") != "Application":
            why = "not an application"
        elif true(keys.get("Hidden")):
            why = "turned off (Hidden)"
        elif (keys.get("X-GNOME-Autostart-enabled") or "").strip().lower() == "false":
            why = "turned off (X-GNOME-Autostart-enabled)"
        elif "OnlyShowIn" in keys and not listed(keys["OnlyShowIn"]) & here:
            why = "only for " + keys["OnlyShowIn"].strip(";")
        elif listed(keys.get("NotShowIn")) & here:
            why = "not for this desktop"
        elif keys.get("TryExec") and not installed(unescape(keys["TryExec"])):
            why = "not installed"
        elif not argv:
            why = "no command"
        elif denied(stem, os.path.basename(argv[0])):
            why = "iRice does this itself"
        elif not installed(argv[0]):
            why = "not installed"
        if why:
            skipped[name] = why
            continue
        cwd = unescape(keys.get("Path", "")) or None
        start.append((name, argv, cwd if cwd and os.path.isdir(cwd) else None, true(keys.get("Terminal"))))
    return start, skipped


def lock():
    """True the first time in this session."""
    rt = os.environ.get("XDG_RUNTIME_DIR") or f"/tmp/irice-{os.getuid()}"
    session = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE") or os.environ.get("XDG_SESSION_ID") or "session"
    path = os.path.join(rt, f"irice-autostart.{session.replace('/', '_')}")
    try:
        os.makedirs(rt, mode=0o700, exist_ok=True)
        os.close(os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600))
        return True
    except FileExistsError:
        return False
    except OSError:
        return True  # (no runtime folder to note it in: start them anyway)


def wait_for_tray(seconds):
    if seconds <= 0 or not shutil.which("busctl"):
        return
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        p = subprocess.run(["busctl", "--user", "status", "org.kde.StatusNotifierWatcher"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if p.returncode == 0:
            return
        time.sleep(0.5)


def child_env():
    """As the shell starts apps (common/Launch.qml): in the tty test session,
    without its sandbox folders."""
    env = dict(os.environ)
    if env.get("IRICE_TTY") == "1":
        for k in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "IRICE_SANDBOX",
                  "IRICE_ROOT", "IRICE_TTY", "QML_IMPORT_PATH", "QML2_IMPORT_PATH"):
            env.pop(k, None)
    return env


def launch(argv, cwd, terminal):
    if terminal:
        argv = ["bash", os.path.join(HERE, "open-app.sh"), "terminal", "--"] + argv
    try:
        subprocess.run(["setsid", "-f"] + argv, cwd=cwd or os.path.expanduser("~"), env=child_env(),
                       stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        return True
    except (OSError, subprocess.TimeoutExpired):
        return False


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--wait-tray", type=float, default=0)
    ap.add_argument("--dry-run", action="store_true")
    opts = ap.parse_args(argv)
    start, skipped = plan()
    if opts.dry_run:
        print(json.dumps({"start": [{"file": n, "command": a, "cwd": c, "terminal": t} for n, a, c, t in start],
                          "skipped": skipped}, indent=2))
        return 0
    if not lock():
        return 0
    if start:
        wait_for_tray(opts.wait_tray)
    for name, args, cwd, terminal in start:
        if not launch(args, cwd, terminal):
            print(f"xdg-autostart: couldn't start {name}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
