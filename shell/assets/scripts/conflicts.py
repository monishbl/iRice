#!/usr/bin/env python3
"""What else is running that does iRice's job twice, for services/Conflicts.qml
(an island alert soon after the shell starts, and Settings → About → System
health). Read-only; prints a JSON list of
  { "id", "name", "short", "what", "fix" }
where `id` is stable (for "don't show again"), `name` is what people know it
as ("waybar", "mako"), `short` and `what` say what it duplicates (for the island, for Settings),
`fix` how to stop it.

Looks for:
  - another owner of org.freedesktop.Notifications (iRice shows notifications)
  - a bar or shell drawn by something else (Hyprland layers whose namespace
    isn't iRice's)
  - more than one hypridle, or a second cliphist recorder (wl-paste --watch)
    in this Hyprland session
  - user units that start those with the session (systemctl --user is-enabled)
The shell that runs this is its parent: its own layers and names are iRice's.
"""
import json
import os
import subprocess
import sys

SHELL_PID = os.getppid()
OUR_SESSION = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "")

# Layer namespaces (or process names) of other bars and shells
BARS = ("waybar", "ags", "astal", "eww", "hyprpanel", "nwg-panel", "nwg-dock", "ironbar", "yambar", "sfwbar",
        "gbar", "polybar", "swaybar", "quickshell", "fabric", "ignis", "mewline", "dms", "caelestia", "noctalia")
NOTIFIERS = {"mako": "mako", "dunst": "dunst", "swaync": "SwayNotificationCenter", "fnott": "fnott",
             "xfce4-notifyd": "xfce4-notifyd", "notify-osd": "notify-osd"}
UNITS = ("hypridle", "waybar", "cliphist", "swaync", "mako", "dunst", "ags", "hyprpanel")


def run(cmd, timeout=3):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def comm(pid):
    try:
        with open(f"/proc/{pid}/comm") as f:
            return f.read().strip()
    except OSError:
        return ""


def cmdline(pid):
    try:
        with open(f"/proc/{pid}/cmdline", "rb") as f:
            return [a.decode(errors="replace") for a in f.read().split(b"\0") if a]
    except OSError:
        return []


def session_of(pid):
    """The Hyprland session a process belongs to: "" when it doesn't say (a
    systemd user service), None when its environment can't be read."""
    try:
        with open(f"/proc/{pid}/environ", "rb") as f:
            for kv in f.read().split(b"\0"):
                if kv.startswith(b"HYPRLAND_INSTANCE_SIGNATURE="):
                    return kv.split(b"=", 1)[1].decode(errors="replace")
    except OSError:
        return None
    return ""


def ours(pid):
    """In this Hyprland session, or started outside any (a user service, which
    counts). One whose environment can't be read isn't counted."""
    s = session_of(pid)
    return s is not None and (not s or not OUR_SESSION or s == OUR_SESSION)


def processes():
    uid = os.getuid()
    for p in os.listdir("/proc"):
        if p.isdigit():
            try:
                if os.stat(f"/proc/{p}").st_uid != uid:
                    continue
            except OSError:
                continue
            yield int(p)


def notifications(found):
    out = run(["busctl", "--user", "call", "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
               "GetConnectionUnixProcessID", "s", "org.freedesktop.Notifications"]).split()
    if len(out) != 2 or not out[1].isdigit():
        return
    pid = int(out[1])
    if pid == SHELL_PID or cmdline(pid) == cmdline(SHELL_PID):
        return
    name = comm(pid) or "another app"
    if name in ("qs", "quickshell"):
        name = "quickshell"  # (another Quickshell config)
    found.append({"id": "notifications:" + name, "name": NOTIFIERS.get(name, name),
                  "short": "Shows notifications instead of iRice",
                  "what": "shows notifications, so iRice's Notification Center and banners stay empty",
                  "fix": f"Stop {name} from starting with your session (remove it from your autostart, or "
                         f"systemctl --user disable --now {name}), then restart iRice."})


def layers(found):
    try:
        data = json.loads(run(["hyprctl", "layers", "-j"]) or "{}")
    except ValueError:
        return
    seen = set()
    for mon in data.values():
        for level in (mon.get("levels") or {}).values():
            for layer in level:
                ns = layer.get("namespace", "") or ""
                pid = layer.get("pid", 0)
                if pid == SHELL_PID or ns.startswith("irice:") or ns.startswith("irice-"):
                    continue
                who = comm(pid) if pid else ""
                base = ns.split(":")[0].split("-")[0].lower()
                name = next((b for b in BARS if base.startswith(b) or who.lower().startswith(b)), "")
                if not name or name in seen:
                    continue
                seen.add(name)
                proc = who or name
                found.append({"id": "bar:" + name, "name": name,
                              "short": "Draws a second bar or shell",
                              "what": "draws a bar or shell over iRice's menu bar and dock",
                              "fix": f"Remove {proc} from your Hyprland autostart (exec-once) or "
                                     f"systemctl --user disable --now {proc}, then log in again."})


def duplicates(found):
    idle, clips = [], {}
    for pid in processes():
        c = comm(pid)
        if c == "hypridle" and ours(pid):
            idle.append(pid)
        elif c == "wl-paste":
            args = cmdline(pid)
            if "--watch" in args and any("cliphist" in a for a in args) and ours(pid):
                # one recorder per kind (text, images) is right
                kind = args[args.index("--type") + 1] if "--type" in args and args.index("--type") + 1 < len(args) else (
                    args[args.index("-t") + 1] if "-t" in args and args.index("-t") + 1 < len(args) else "")
                clips.setdefault(kind, []).append(pid)
    if len(idle) > 1:
        found.append({"id": "hypridle", "name": "hypridle", "short": f"Runs {len(idle)} times",
                      "what": f"is running {len(idle)} times, so the screen may dim, lock or sleep twice",
                      "fix": "iRice starts its own hypridle. Remove exec-once = hypridle from your Hyprland config, "
                             "or systemctl --user disable --now hypridle."})
    if any(len(v) > 1 for v in clips.values()):
        found.append({"id": "cliphist", "name": "a second clipboard recorder",
                      "short": "Saves what you copy twice",
                      "what": "saves everything you copy twice (wl-paste --watch cliphist store)",
                      "fix": "iRice records the clipboard itself. Remove the extra exec-once = wl-paste --watch cliphist store "
                             "from your Hyprland config, or systemctl --user disable --now cliphist."})


def units(found):
    out = run(["systemctl", "--user", "is-enabled"] + [u + ".service" for u in UNITS]).split("\n")
    if len(out) < len(UNITS):
        return
    have = {f["id"] for f in found}
    for unit, state in zip(UNITS, out):
        if state.strip() not in ("enabled", "enabled-runtime", "linked"):
            continue
        if any(i.endswith(":" + unit) or i == unit for i in have):
            continue  # (already said: it's running)
        found.append({"id": "unit:" + unit, "name": unit, "short": "Starts with your session",
                      "what": "starts with your session (a systemd user service) and does a job iRice does",
                      "fix": f"systemctl --user disable --now {unit}"})


def main():
    found = []
    for check in (notifications, layers, duplicates, units):
        try:
            check(found)
        except Exception:  # one check failing never hides the others
            pass
    json.dump(found, sys.stdout)


if __name__ == "__main__":
    main()
