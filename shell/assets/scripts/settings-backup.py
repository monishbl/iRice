#!/usr/bin/python3 -I
"""Your Tessera setup in one file, and back (Settings → About → Back up and
restore):

  settings-backup.py export <folder> --config DIR [--shell DIR]
  settings-backup.py inspect <file>
  settings-backup.py restore <file> --config DIR [--shell DIR] [--pictures DIR] [--no-undo]

The file ("Tessera settings YYYY-MM-DD.tessera", 0600) is a gzipped tar of
  manifest.json   what's in it: the format, when, the home it came from,
                  where each wallpaper went
  config.json     the settings, without keys, tokens or passwords, accounts
                  (Google, iCloud, Microsoft) or this computer's displays
  keybinds.json   the shortcuts and gestures you changed
  focus.json      the Focus modes (not whether one is on)
  wallpapers/…    the pictures in use that aren't Tessera's own (the bundled
                  ones go by name)
Restore keeps this computer's own keys, accounts, displays and sharing
name, moves paths from the old home to this one, puts carried pictures in
~/Pictures/Wallpapers/Restored, and turns off a time-of-day folder or live
video that isn't here. Before it writes anything it saves the setup it
replaces to <config>/backups/before-restore-<stamp>.tessera (Undo restores
that with --no-undo). Files are written whole (a temporary file, then
renamed): a running shell never reads half of one.

Prints one JSON object: {"ok": true, …} or {"ok": false, "error": "…"}
(what went wrong and how to fix it), exit 0 or 1.
"""
import argparse
import copy
import io
import json
import os
import re
import sys
import tarfile
import time

FORMAT = 1
MAX_PICTURE = 64 * 1024 * 1024
MAX_TOTAL = 256 * 1024 * 1024
MAX_JSON = 8 * 1024 * 1024
JSON_FILES = ("manifest.json", "config.json", "keybinds.json", "focus.json")
PICTURE_NAME = re.compile(r"^wallpapers/[A-Za-z0-9 _.,()+-]{1,160}$")
# A setting holding a key, a token or a password (camelCase or plain)
SECRET = re.compile(r"(^|[a-z_])(key|token|secret|password|passphrase)$|[a-z0-9](Key|Token|Secret|Password|Passphrase)$")
# Whole sections that belong to this computer or its accounts
LOCAL_SECTIONS = ("google", "cloud", "displays")
# Single settings that do: (section, key)
LOCAL_KEYS = (("sharing", "name"), ("ai", "extraModels"))
# The wallpaper settings that point at pictures
WALLPAPER_KEYS = ("path", "lightPath", "darkPath")
PICTURE_EXT = (".jpg", ".jpeg", ".png", ".webp", ".jxl", ".avif", ".bmp", ".gif")


class Refused(Exception):
    """A file or a request this won't act on; the message says how to fix it."""


# ── Settings in and out ─────────────────────────────────────────────────
def scrub(value):
    """A copy without any setting named like a key, token or password (only
    where it holds text: "passwordAfterSleep" is a number and stays)."""
    if isinstance(value, dict):
        return {k: scrub(v) for k, v in value.items() if not (SECRET.search(k) and isinstance(v, str))}
    if isinstance(value, list):
        return [scrub(v) for v in value]
    return value


def scrub_config(config):
    c = scrub(config if isinstance(config, dict) else {})
    for sec in LOCAL_SECTIONS:
        c.pop(sec, None)
    for sec, key in LOCAL_KEYS:
        if isinstance(c.get(sec), dict):
            c[sec].pop(key, None)
    # AI models are JSON records in a list: the same, inside each
    ai = c.get("ai")
    if isinstance(ai, dict) and isinstance(ai.get("models"), list):
        models = []
        for m in ai["models"]:
            try:
                rec = json.loads(m) if isinstance(m, str) else m
            except ValueError:
                continue
            if isinstance(rec, dict):
                models.append(json.dumps(scrub(rec)))
        ai["models"] = models
    return c


def secrets(value, path=()):
    """This computer's keys and passwords: [(path, value)]."""
    out = []
    if isinstance(value, dict):
        for k, v in value.items():
            if SECRET.search(k) and isinstance(v, str):
                if v:
                    out.append((path + (k,), v))
            else:
                out.extend(secrets(v, path + (k,)))
    return out


def put(d, path, value):
    for k in path[:-1]:
        if not isinstance(d.get(k), dict):
            d[k] = {}
        d = d[k]
    d[path[-1]] = value


def merge_local(restored, current):
    """The restored settings with this computer's own parts kept."""
    new = copy.deepcopy(restored)
    current = current if isinstance(current, dict) else {}
    for sec in LOCAL_SECTIONS:
        if sec in current:
            new[sec] = copy.deepcopy(current[sec])
        else:
            new.pop(sec, None)
    for sec, key in LOCAL_KEYS:
        here = current.get(sec, {}) if isinstance(current.get(sec), dict) else {}
        if key in here:
            put(new, (sec, key), copy.deepcopy(here[key]))
        elif isinstance(new.get(sec), dict):
            new[sec].pop(key, None)
    for path, v in secrets(current):
        put(new, path, v)
    return new


def rehome(value, old, new):
    """Paths under the old home moved to this one."""
    if not old or old == new:
        return value
    if isinstance(value, dict):
        return {k: rehome(v, old, new) for k, v in value.items()}
    if isinstance(value, list):
        return [rehome(v, old, new) for v in value]
    if isinstance(value, str) and (value == old or value.startswith(old + "/")):
        return new + value[len(old):]
    return value


def focus_modes(focus):
    modes = focus.get("modes") if isinstance(focus, dict) else None
    return [m for m in modes if isinstance(m, dict)] if isinstance(modes, list) else []


# ── Places ──────────────────────────────────────────────────────────────
def default_shell():
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.realpath(__file__))))


def bundled_dirs(shell):
    shell = os.path.realpath(shell)
    return [os.path.join(shell, "assets", "wallpapers"), os.path.join(os.path.dirname(shell), "material", "assets", "wallpapers")]


def bundled_name(path):
    """The bundled picture's name if this is one of Tessera's own (in any
    install: shell/ or material/ assets, or the Material look's root)."""
    if isinstance(path, str) and "/assets/wallpapers/" in path:
        return os.path.basename(path)
    return None


def find_bundled(shell, name):
    for d in bundled_dirs(shell):
        p = os.path.join(d, name)
        if os.path.isfile(p):
            return p
    return ""


def unique(folder, name):
    base, ext = os.path.splitext(name)
    p, n = os.path.join(folder, name), 2
    while os.path.lexists(p):
        p = os.path.join(folder, "%s %d%s" % (base, n, ext))
        n += 1
    return p


def write_whole(path, data, mode=0o600):
    """Written all at once: a temporary file in the same folder, renamed."""
    d = os.path.dirname(path) or "."
    os.makedirs(d, exist_ok=True)
    tmp = os.path.join(d, ".%s.%d.tmp" % (os.path.basename(path), os.getpid()))
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, mode)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.chmod(tmp, mode)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def read_json(path, default):
    try:
        with open(path, "rb") as f:
            data = f.read(MAX_JSON + 1)
        if len(data) > MAX_JSON:
            return default
        return json.loads(data)
    except (OSError, ValueError):
        return default


def mode_of(path, default=0o600):
    try:
        return os.stat(path).st_mode & 0o777
    except OSError:
        return default


# ── Export ──────────────────────────────────────────────────────────────
def build(config_dir, carry=True):
    """The archive's members: {name: bytes}. carry: pictures that aren't
    bundled go in the file (off for the Undo copy: they're still here)."""
    config = scrub_config(read_json(os.path.join(config_dir, "config.json"), {}))
    keybinds = read_json(os.path.join(config_dir, "keybinds.json"), None)
    focus = read_json(os.path.join(config_dir, "focus.json"), None)
    members, pictures, total = {}, {}, 0

    def carry_picture(where, path):
        nonlocal total
        name = bundled_name(path)
        if name:
            pictures[where] = {"bundled": name}
            return
        if not carry or not isinstance(path, str) or not path.lower().endswith(PICTURE_EXT):
            return
        try:
            size = os.path.getsize(path)
            if size > MAX_PICTURE or total + size > MAX_TOTAL:
                return
            with open(path, "rb") as f:
                data = f.read(MAX_PICTURE + 1)
        except OSError:
            return
        if len(data) > MAX_PICTURE:
            return
        clean = re.sub(r"[^A-Za-z0-9 _.,()+-]", "_", os.path.basename(path))[:120] or "picture.jpg"
        member = "wallpapers/%d-%s" % (len(pictures), clean)
        members[member] = data
        total += len(data)
        pictures[where] = {"file": member}

    wp = config.get("wallpaper") if isinstance(config.get("wallpaper"), dict) else {}
    for key in WALLPAPER_KEYS:
        if wp.get(key):
            carry_picture("wallpaper." + key, wp[key])
    if focus is not None:
        modes = focus_modes(focus)
        for m in modes:
            if m.get("wallpaper") and isinstance(m.get("id"), str):
                carry_picture("focus." + m["id"], m["wallpaper"])
        members["focus.json"] = json.dumps({"modes": modes}, indent=2).encode()
    if isinstance(keybinds, dict):
        members["keybinds.json"] = json.dumps(keybinds, indent=2).encode()
    members["config.json"] = json.dumps(config, indent=2).encode()
    manifest = {
        "app": "Tessera", "format": FORMAT, "created": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "home": os.path.expanduser("~"), "wallpapers": pictures,
    }
    members["manifest.json"] = json.dumps(manifest, indent=2).encode()
    return members


def pack(members):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for name in ["manifest.json"] + sorted(n for n in members if n != "manifest.json"):
            info = tarfile.TarInfo(name)
            info.size = len(members[name])
            info.mode = 0o600
            info.mtime = int(time.time())
            tar.addfile(info, io.BytesIO(members[name]))
    return buf.getvalue()


def export(folder, config_dir, name=None, carry=True):
    folder = os.path.abspath(os.path.expanduser(folder))
    if not os.path.isdir(folder):
        raise Refused("That folder isn't there any more. Choose another folder and try again.")
    if not os.access(folder, os.W_OK):
        raise Refused("Tessera can't write to that folder. Choose a folder in your home, like Documents.")
    out = unique(folder, name or "Tessera settings %s.tessera" % time.strftime("%Y-%m-%d"))
    write_whole(out, pack(build(config_dir, carry)))
    return out


# ── Reading a file ──────────────────────────────────────────────────────
NOT_TESSERA = "This isn't a Tessera settings file, or it's damaged. Choose a file made with Back Up Settings."


def unpack(path):
    """The archive's members, checked: only the names this writes, plain
    files, within the size limits."""
    try:
        if os.path.getsize(path) > MAX_TOTAL + MAX_JSON * 4:
            raise Refused("That file is too large to be Tessera settings. Choose a file made with Back Up Settings.")
        tar = tarfile.open(path, mode="r:gz")
    except (OSError, tarfile.TarError, EOFError):
        raise Refused(NOT_TESSERA)
    members, total = {}, 0
    with tar:
        try:
            for info in tar:
                name = info.name
                if name.startswith(("/", "./")) or ".." in name.split("/") or "\\" in name:
                    raise Refused(NOT_TESSERA)
                if name not in JSON_FILES and not PICTURE_NAME.match(name):
                    raise Refused(NOT_TESSERA)
                if not info.isreg() or name in members:
                    raise Refused(NOT_TESSERA)
                limit = MAX_JSON if name in JSON_FILES else MAX_PICTURE
                if info.size > limit or total + info.size > MAX_TOTAL:
                    raise Refused("A part of that file is too large. Choose a file made with Back Up Settings.")
                f = tar.extractfile(info)
                data = f.read(limit + 1) if f else b""
                if len(data) != info.size:
                    raise Refused(NOT_TESSERA)
                total += len(data)
                members[name] = data
        except (tarfile.TarError, EOFError, OSError):
            raise Refused(NOT_TESSERA)
    try:
        manifest = json.loads(members.get("manifest.json", b""))
        config = json.loads(members.get("config.json", b""))
    except ValueError:
        raise Refused(NOT_TESSERA)
    # ("iRice": made before iRice became Tessera, the same settings)
    if not isinstance(manifest, dict) or manifest.get("app") not in ("Tessera", "iRice") or not isinstance(config, dict):
        raise Refused(NOT_TESSERA)
    if not isinstance(manifest.get("format"), int) or manifest["format"] > FORMAT:
        raise Refused("That file was made by a newer Tessera. Update Tessera (Settings → Software Update), then try again.")
    parsed = {"manifest": manifest, "config": config, "keybinds": None, "focus": None, "pictures": {}}
    for key in ("keybinds", "focus"):
        if key + ".json" in members:
            try:
                parsed[key] = json.loads(members[key + ".json"])
            except ValueError:
                raise Refused(NOT_TESSERA)
    parsed["pictures"] = {n: d for n, d in members.items() if n.startswith("wallpapers/")}
    return parsed


def summary(parsed):
    c, kb = parsed["config"], parsed["keybinds"]
    ap = c.get("appearance") if isinstance(c.get("appearance"), dict) else {}
    wp = c.get("wallpaper") if isinstance(c.get("wallpaper"), dict) else {}
    shortcuts = 0
    if isinstance(kb, dict):
        shortcuts += len(kb.get("keybinds") or {}) + len(kb.get("shortcuts") or [])
        g = kb.get("gestures") if isinstance(kb.get("gestures"), dict) else {}
        shortcuts += len(g.get("changes") or {}) + len(g.get("custom") or [])
    picture = wp.get("path") or wp.get("darkPath") or wp.get("lightPath") or ""
    return {
        "created": parsed["manifest"].get("created", ""),
        "look": ap.get("look", "glass"),
        "mode": ap.get("mode", ""),
        "wallpaper": os.path.basename(picture) if picture else ("Changes through the day" if wp.get("dynamic") == "time" else "Tessera's own"),
        "shortcuts": shortcuts,
        "focusModes": [m.get("name", "") for m in focus_modes(parsed["focus"])],
        "pictures": len(parsed["pictures"]),
    }


# ── Restore ─────────────────────────────────────────────────────────────
def restore(path, config_dir, shell=None, pictures_dir=None, undo=True):
    parsed = unpack(path)
    shell = shell or default_shell()
    home = os.path.expanduser("~")
    pictures_dir = pictures_dir or os.path.join(home, "Pictures", "Wallpapers", "Restored")
    manifest = parsed["manifest"]
    old_home = manifest.get("home") if isinstance(manifest.get("home"), str) else ""

    config_path = os.path.join(config_dir, "config.json")
    current = read_json(config_path, {})
    # What it replaces, first, so Undo can bring it back
    undo_file = ""
    if undo:
        backups = os.path.join(config_dir, "backups")
        os.makedirs(backups, mode=0o700, exist_ok=True)
        undo_file = export(backups, config_dir, "before-restore-%s.tessera" % time.strftime("%Y%m%d-%H%M%S"), carry=False)

    config = merge_local(rehome(scrub_config(parsed["config"]), old_home, home), current)
    focus = rehome(parsed["focus"], old_home, home) if parsed["focus"] is not None else None
    keybinds = rehome(parsed["keybinds"], old_home, home) if isinstance(parsed["keybinds"], dict) else None

    # Pictures: the bundled ones from this install, carried ones copied here
    placed, skipped = {}, []
    for where, how in (manifest.get("wallpapers") or {}).items():
        if not isinstance(how, dict):
            continue
        target = ""
        if isinstance(how.get("bundled"), str):
            target = find_bundled(shell, os.path.basename(how["bundled"]))
        elif how.get("file") in parsed["pictures"]:
            os.makedirs(pictures_dir, exist_ok=True)
            target = unique(pictures_dir, how["file"].split("/", 1)[1].split("-", 1)[-1])
            write_whole(target, parsed["pictures"][how["file"]], 0o600)
        placed[where] = target
    wp = config.get("wallpaper")
    if isinstance(wp, dict):
        for key in WALLPAPER_KEYS:
            if "wallpaper." + key in placed:
                wp[key] = placed["wallpaper." + key]
            elif wp.get(key) and not os.path.isfile(wp[key]):
                wp[key] = ""  # (Tessera's own picture then)
        if wp.get("dynamicFolder") and not os.path.isdir(wp["dynamicFolder"]):
            skipped.append("the time-of-day wallpaper folder")
            wp["dynamicFolder"] = ""
            if wp.get("dynamic") == "time":
                wp["dynamic"] = "off"
        if wp.get("live") and not os.path.isfile(wp["live"]):
            skipped.append("the live wallpaper (its video isn't on this computer)")
            wp["live"] = ""
        if wp.get("directory") and not os.path.isdir(wp["directory"]):
            wp["directory"] = ""
    if focus is not None:
        modes = focus_modes(focus)
        for m in modes:
            key = "focus." + str(m.get("id"))
            if key in placed:
                m["wallpaper"] = placed[key]
            elif m.get("wallpaper") and not os.path.isfile(m["wallpaper"]):
                m["wallpaper"] = ""
        # (whether a mode is on stays this computer's, if that mode is still there)
        here = read_json(os.path.join(config_dir, "focus.json"), {})
        here = here if isinstance(here, dict) else {}
        keep = here.get("active") in [m.get("id") for m in modes]
        focus = {"modes": modes, "active": here.get("active", "") if keep else "", "until": here.get("until", 0) if keep else 0,
                 "source": here.get("source", "") if keep else "", "restore": here.get("restore", {}) if keep else {},
                 "skipScheduleUntil": here.get("skipScheduleUntil", 0)}

    os.makedirs(config_dir, exist_ok=True)
    written = ["settings"]
    if keybinds is not None:
        p = os.path.join(config_dir, "keybinds.json")
        write_whole(p, json.dumps(keybinds, indent=2).encode(), mode_of(p))
        written.append("shortcuts")
    if focus is not None:
        p = os.path.join(config_dir, "focus.json")
        write_whole(p, json.dumps(focus, indent=2).encode(), mode_of(p))
        written.append("Focus modes")
    if placed:
        written.append("wallpaper")
    # (the settings last: the shell reloads them as they change)
    write_whole(config_path, json.dumps(config, indent=4).encode(), mode_of(config_path))
    ap = config.get("appearance") if isinstance(config.get("appearance"), dict) else {}
    return {"restored": written, "skipped": skipped, "undo": undo_file, "look": ap.get("look", "glass")}


def main(argv):
    ap = argparse.ArgumentParser(description="Back up and restore Tessera settings")
    sub = ap.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("export")
    e.add_argument("folder")
    e.add_argument("--config", required=True)
    e.add_argument("--shell")
    i = sub.add_parser("inspect")
    i.add_argument("file")
    r = sub.add_parser("restore")
    r.add_argument("file")
    r.add_argument("--config", required=True)
    r.add_argument("--shell")
    r.add_argument("--pictures")
    r.add_argument("--no-undo", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.cmd == "export":
            out = {"file": export(a.folder, a.config)}
        elif a.cmd == "inspect":
            out = summary(unpack(a.file))
        else:
            out = restore(a.file, a.config, a.shell, a.pictures, undo=not a.no_undo)
        print(json.dumps(dict(ok=True, **out)))
        return 0
    except Refused as err:
        print(json.dumps({"ok": False, "error": str(err)}))
    except OSError as err:
        print(json.dumps({"ok": False, "error": "Couldn't %s: %s. Check there's free space and the folder is yours, then try again." % (
            {"export": "save the file", "inspect": "read the file", "restore": "restore the settings"}[a.cmd], err.strerror or err)}))
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
