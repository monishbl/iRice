#!/usr/bin/env python3
"""Starts the apps that asked to start with your session (XDG autostart:
~/.config/autostart and /etc/xdg/autostart), once per session. Hyprland
doesn't do this itself; hypr/irice/execs.lua runs this at login.

  xdg-autostart.py [--wait-tray SECONDS] [--dry-run]
  xdg-autostart.py list | set FILE on|off | add DESKTOP-ID | remove FILE
                   | restore FILE | start FILE

The subcommands are for Settings → Login Items; each prints one JSON object
(list: {startAtLogin, items: [...]}; the others {ok, error?}). FILE is a
file name only (foo.desktop), never a path, and only
$XDG_CONFIG_HOME/autostart is written: turning a system entry off writes a
user copy of it there with Hidden=true (the XDG way; X-iRice-Override marks
it, and turning it back on removes it), turning your own off sets Hidden=true
in it (the file stays). add copies Name/Exec/Icon from an installed app's
desktop entry; remove moves your file aside (restore brings it back, for
Undo). start runs one now. Settings → Login Items → "Start them when you log
in" (loginItems.startAtLogin in $XDG_CONFIG_HOME/irice/config.json) off: the
login run starts nothing. IRICE_SYSTEM_ROOT (tests, the sandbox) puts the
system folder (/etc/xdg, when XDG_CONFIG_DIRS isn't set) under another root.

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
import re
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


def config_home():
    return os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")


def user_dir():
    return os.path.join(config_home(), "autostart")


def system_dirs():
    """XDG_CONFIG_DIRS, else /etc/xdg (under IRICE_SYSTEM_ROOT when set)."""
    system = [d for d in (os.environ.get("XDG_CONFIG_DIRS") or "").split(":") if d]
    if not system:
        system = [os.path.join(os.environ.get("IRICE_SYSTEM_ROOT") or "/", "etc/xdg")]
    return [os.path.join(d, "autostart") for d in system]


def dirs():
    """The autostart folders, most important first."""
    return [user_dir()] + system_dirs()


def start_at_login():
    """Settings → Login Items → Start them when you log in (default on)."""
    try:
        with open(os.path.join(config_home(), "irice", "config.json"), encoding="utf-8") as f:
            v = json.load(f).get("loginItems", {}).get("startAtLogin", True)
        return v is not False
    except (OSError, ValueError, AttributeError):
        return True


def entries(folders=None):
    """{file name: path}, the first folder holding a name winning."""
    out = {}
    for d in folders if folders is not None else dirs():
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


def why_not(name, keys, path, argv, here):
    """Why an entry doesn't start ("" when it does)."""
    stem = name[:-len(".desktop")]
    if keys.get("Type", "Application") != "Application":
        return "not an application"
    if true(keys.get("Hidden")):
        return "turned off (Hidden)"
    if (keys.get("X-GNOME-Autostart-enabled") or "").strip().lower() == "false":
        return "turned off (X-GNOME-Autostart-enabled)"
    if "OnlyShowIn" in keys and not listed(keys["OnlyShowIn"]) & here:
        return "only for " + keys["OnlyShowIn"].strip(";")
    if listed(keys.get("NotShowIn")) & here:
        return "not for this desktop"
    if keys.get("TryExec") and not installed(unescape(keys["TryExec"])):
        return "not installed"
    if not argv:
        return "no command"
    if denied(stem, os.path.basename(argv[0])):
        return "iRice does this itself"
    if not installed(argv[0]):
        return "not installed"
    return ""


def plan():
    """[(file name, argv, cwd, terminal)] to start, and {file name: why not} for the rest."""
    here = desktops()
    start, skipped = [], {}
    for name, path in entries().items():
        keys = parse(path)
        argv = command(keys, path)
        why = why_not(name, keys, path, argv, here)
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


# ── Settings → Login Items ──────────────────────────────────────────────

FILE_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+@-]{0,200}\.desktop$")
ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+@-]{0,200}$")
OVERRIDE = "X-iRice-Override"  # (a user copy of a system entry that only turns it on or off)
DESKTOP_NAMES = {"kde": "KDE Plasma", "gnome": "GNOME", "xfce": "Xfce", "lxqt": "LXQt", "lxde": "LXDE",
                 "mate": "MATE", "x-cinnamon": "Cinnamon", "cinnamon": "Cinnamon", "unity": "Unity",
                 "budgie": "Budgie", "pantheon": "Pantheon", "deepin": "Deepin", "enlightenment": "Enlightenment",
                 "cosmic": "COSMIC", "sway": "Sway", "hyprland": "Hyprland", "x-generic": "other desktops"}


class Refused(Exception):
    pass


def valid_file(name):
    if not isinstance(name, str) or not FILE_RE.fullmatch(name) or ".." in name:
        raise Refused("Not a login item file name: " + repr(name)[:80])
    return name


def first_system(name):
    for d in system_dirs():
        p = os.path.join(d, name)
        if os.path.isfile(p):
            return p
    return None


def friendly(why, keys):
    """The reason as Settings shows it."""
    if why.startswith("only for "):
        names = []
        for d in keys.get("OnlyShowIn", "").split(";"):
            d = d.strip()
            if d:
                n = DESKTOP_NAMES.get(d.lower(), d)
                if n not in names:
                    names.append(n)
        return "Only for " + (" and ".join(names) if len(names) < 3 else ", ".join(names[:-1]) + " and " + names[-1])
    return {"not an application": "Not an app", "not for this desktop": "Not for Hyprland",
            "not installed": "Not installed", "no command": "No command to run",
            "iRice does this itself": "iRice does this itself"}.get(why, why[:1].upper() + why[1:])


def items():
    here = desktops()
    users = entries([user_dir()])
    systems = entries(system_dirs())
    out = []
    for name in sorted(set(users) | set(systems)):
        path = users.get(name) or systems[name]
        keys = parse(path)
        base = parse(systems[name]) if name in systems else {}
        argv = command(keys, path)
        why = why_not(name, keys, path, argv, here)
        off = why.startswith("turned off")
        # the switch: what turning it on would leave (the rest of the checks
        # don't depend on it)
        if off:
            on_keys = dict(keys)
            on_keys.pop("Hidden", None)
            on_keys.pop("X-GNOME-Autostart-enabled", None)
            later = why_not(name, on_keys, path, argv, here)
        else:
            later = why
        status = "start" if not why else "off" if off else "skipped"
        out.append({
            "file": name,
            "name": keys.get("Name") or base.get("Name") or name[:-len(".desktop")],
            "comment": keys.get("Comment") or base.get("Comment") or "",
            "icon": keys.get("Icon") or base.get("Icon") or "",
            "exec": unescape(keys.get("Exec") or base.get("Exec") or ""),
            "origin": "system" if name in systems else "user",
            "overridden": name in systems and name in users,
            "enabled": not off,
            "status": status,
            "reason": "" if not why else "Turned off" if off else friendly(why, keys),
            "skippedWhenOn": friendly(later, keys) if off and later else "",
            "canStart": bool(argv) and installed(argv[0]) and not denied(name[:-len(".desktop")], os.path.basename(argv[0])),
            "removable": name in users and name not in systems,
            "added": true(keys.get("X-iRice-Added")),
        })
    return sorted(out, key=lambda i: (i["name"].casefold(), i["file"]))


def edit_keys(text, changes):
    """The file with [Desktop Entry] keys set (value) or removed (None)."""
    lines = text.splitlines()
    out, group, done, header = [], None, set(), -1
    for line in lines:
        st = line.strip()
        if st.startswith("["):
            group = st
            if st == "[Desktop Entry]":
                header = len(out)
            out.append(line)
            continue
        if group == "[Desktop Entry]" and "=" in st and not st.startswith("#"):
            k = st.partition("=")[0].strip()
            if k in changes:
                if k not in done and changes[k] is not None:
                    out.append(f"{k}={changes[k]}")
                done.add(k)
                continue
        out.append(line)
    missing = [f"{k}={v}" for k, v in changes.items() if v is not None and k not in done]
    if header < 0:
        out = ["[Desktop Entry]"] + missing + out
    else:
        # after the group's last key line
        end = len(out)
        for i in range(header + 1, len(out)):
            if out[i].strip().startswith("["):
                end = i
                break
        while end > header + 1 and not out[end - 1].strip():
            end -= 1
        out[end:end] = missing
    return "\n".join(out) + "\n"


def write(name, text):
    d = user_dir()
    os.makedirs(d, exist_ok=True)
    tmp = os.path.join(d, "." + name + ".tmp")
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(text)
    os.replace(tmp, os.path.join(d, name))


def read(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read()


def set_enabled(name, on):
    name = valid_file(name)
    user = os.path.join(user_dir(), name)
    system = first_system(name)
    has_user = os.path.isfile(user)
    if not has_user and not system:
        raise Refused(f"{name} isn't a login item")
    if on:
        if has_user:
            keys = parse(user)
            sys_keys = parse(system) if system else {}
            sys_on = system and not true(sys_keys.get("Hidden")) and (sys_keys.get("X-GNOME-Autostart-enabled") or "").lower() != "false"
            if true(keys.get(OVERRIDE)) and sys_on:
                os.remove(user)
                return {"ok": True}
            changes = {"Hidden": None}
            if "X-GNOME-Autostart-enabled" in keys or (system and "X-GNOME-Autostart-enabled" in sys_keys):
                changes["X-GNOME-Autostart-enabled"] = "true"
            if system and true(sys_keys.get("Hidden")):
                changes["Hidden"] = "false"
            write(name, edit_keys(read(user), changes))
        else:
            sys_keys = parse(system)
            changes = {OVERRIDE: "true"}
            if true(sys_keys.get("Hidden")):
                changes["Hidden"] = "false"
            if "X-GNOME-Autostart-enabled" in sys_keys:
                changes["X-GNOME-Autostart-enabled"] = "true"
            if len(changes) == 1:
                return {"ok": True}  # (already on)
            write(name, edit_keys(read(system), changes))
    else:
        if has_user:
            write(name, edit_keys(read(user), {"Hidden": "true"}))
        else:
            write(name, edit_keys(read(system), {"Hidden": "true", OVERRIDE: "true"}))
    return {"ok": True}


def data_dirs():
    home = os.environ.get("XDG_DATA_HOME") or os.path.join(os.path.expanduser("~"), ".local", "share")
    return [home] + [d for d in (os.environ.get("XDG_DATA_DIRS") or "/usr/local/share:/usr/share").split(":") if d]


def find_app(app):
    """An installed app's desktop entry by its id (org.foo.Bar, or kde-foo for kde/foo.desktop)."""
    parts = app.split("-")
    for d in data_dirs():
        base = os.path.join(d, "applications")
        for i in range(len(parts)):
            p = os.path.join(base, *parts[:i], "-".join(parts[i:]) + ".desktop")
            if os.path.isfile(p):
                return p
    return None


def add(app):
    if not isinstance(app, str):
        raise Refused("No app given")
    if app.endswith(".desktop"):
        app = app[:-len(".desktop")]
    if not ID_RE.fullmatch(app) or ".." in app:
        raise Refused("Not an app id: " + repr(app)[:80])
    name = app + ".desktop"
    src = find_app(app)
    if not src:
        raise Refused(f"{app} isn't installed")
    keys = parse(src)
    if not keys.get("Exec"):
        raise Refused(f"{keys.get('Name') or app} has no command to start")
    if name in entries():
        set_enabled(name, True)
        return {"ok": True, "file": name, "existed": True}
    body = ["[Desktop Entry]", "Type=Application"]
    for k in ("Name", "GenericName", "Comment", "Icon", "Exec", "TryExec", "Path", "Terminal"):
        if keys.get(k):
            body.append(f"{k}={keys[k]}")
    body.append("X-iRice-Added=true")
    write(name, "\n".join(body) + "\n")
    return {"ok": True, "file": name}


def removed_dir():
    state = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return os.path.join(state, "irice", "autostart-removed")


def remove(name):
    name = valid_file(name)
    user = os.path.join(user_dir(), name)
    if not os.path.isfile(user):
        raise Refused(f"{name} isn't one of your login items")
    if first_system(name):
        raise Refused("A system login item can't be removed; turn it off instead")
    os.makedirs(removed_dir(), exist_ok=True)
    os.replace(user, os.path.join(removed_dir(), name))
    return {"ok": True}


def restore(name):
    name = valid_file(name)
    kept = os.path.join(removed_dir(), name)
    if not os.path.isfile(kept):
        raise Refused(f"{name} can't be brought back")
    if os.path.exists(os.path.join(user_dir(), name)):
        raise Refused(f"{name} is already back")
    os.makedirs(user_dir(), exist_ok=True)
    os.replace(kept, os.path.join(user_dir(), name))
    return {"ok": True}


def start_now(name):
    name = valid_file(name)
    path = entries().get(name)
    if not path:
        raise Refused(f"{name} isn't a login item")
    keys = parse(path)
    argv = command(keys, path)
    if not argv or not installed(argv[0]):
        raise Refused(f"{keys.get('Name') or name} isn't installed")
    cwd = unescape(keys.get("Path", "")) or None
    if not launch(argv, cwd if cwd and os.path.isdir(cwd) else None, true(keys.get("Terminal"))):
        raise Refused(f"{keys.get('Name') or name} didn't start")
    return {"ok": True}


def manage(argv):
    cmd, args = argv[0], argv[1:]
    try:
        if cmd == "list" and not args:
            out = {"startAtLogin": start_at_login(), "items": items()}
        elif cmd == "set" and len(args) == 2 and args[1] in ("on", "off"):
            out = set_enabled(args[0], args[1] == "on")
        elif cmd == "add" and len(args) == 1:
            out = add(args[0])
        elif cmd == "remove" and len(args) == 1:
            out = remove(args[0])
        elif cmd == "restore" and len(args) == 1:
            out = restore(args[0])
        elif cmd == "start" and len(args) == 1:
            out = start_now(args[0])
        else:
            raise Refused("Usage: list | set FILE on|off | add DESKTOP-ID | remove FILE | restore FILE | start FILE")
    except Refused as e:
        print(json.dumps({"ok": False, "error": str(e)}))
        return 2
    except OSError as e:
        print(json.dumps({"ok": False, "error": f"Couldn't change it: {e.strerror or e}"}))
        return 1
    print(json.dumps(out))
    return 0


def main(argv):
    if argv and argv[0] in ("list", "set", "add", "remove", "restore", "start"):
        return manage(argv)
    ap = argparse.ArgumentParser()
    ap.add_argument("--wait-tray", type=float, default=0)
    ap.add_argument("--dry-run", action="store_true")
    opts = ap.parse_args(argv)
    start, skipped = plan()
    if opts.dry_run:
        print(json.dumps({"startAtLogin": start_at_login(), "start": [{"file": n, "command": a, "cwd": c, "terminal": t} for n, a, c, t in start],
                          "skipped": skipped}, indent=2))
        return 0
    if not start_at_login() or not lock():
        return 0
    if start:
        wait_for_tray(opts.wait_tray)
    for name, args, cwd, terminal in start:
        if not launch(args, cwd, terminal):
            print(f"xdg-autostart: couldn't start {name}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
