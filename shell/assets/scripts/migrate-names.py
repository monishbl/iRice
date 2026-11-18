#!/usr/bin/env python3
"""Tessera's old name, moved out of your files: the one place in Python
(with the root helper's own section, tessera-root.py) that still knows it.

Tessera was called iRice until 4 November 2026. An installed machine that
updates across that rename needs, once, everything it kept under the old
name moved to the new one; after that nothing uses the old name again.
REMOVE AFTER 2027-02-01 (REMOVE_AFTER): delete this file, the bash, Rust and
QML modules named in docs/rename-to-tessera.md, and their callers' lines.

What it does, each time the shell starts (launch-shell.sh, sandbox/
tty-shell.sh; also the installer, the build scripts and Software Update),
doing nothing once everything is done:

  folders   $XDG_CONFIG_HOME/irice … $XDG_CACHE_HOME/irice (and the same
            under your home, when a session points XDG_* elsewhere) become
            …/tessera. Only the old one: renamed. Both: what's only in the
            old one moves over, the rest is kept as irice.before-tessera.
            The old name is left as a link to the new folder.
  checkout  the folder Tessera runs from, ~/iRice, becomes ~/tessera, with
            ~/iRice left as a link to it; paths to it saved in your settings,
            the sandbox home and Tessera's own files point at the new place.
  settings  values saved under the old name (the menu icon, the screenshot
            editor, shortcuts that run "$IRICE_SHELL/…", state.json's
            revisions, spaces.lua) take the new one; Hyprland files, backups,
            fish snippets, theme includes, file-manager menus and the battery
            guard's unit installed under the old name are renamed or replaced.
  leftovers builds made under the old name that nothing uses any more
            (irice-glass.so, the IRice native module) go; the events helper
            takes Tessera's name when there's no newer one.
  links     an old-name link (…/irice, ~/iRice) goes once it's at least 14
            days old and nothing uses it: no process of yours has it open or
            on its command line or in its environment, and no session entry,
            Hyprland file or user unit mentions it.

A note, RENAMED.txt, in each new folder says what moved.

    migrate-names.py [--dry-run] [--checkout DIR]… [--no-checkout]
(--no-checkout: everything but moving the checkout, for a build running in it)
Never fails the caller: problems are printed and it exits 0.
"""
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import time

REMOVE_AFTER = datetime.date(2027, 2, 1)
OLD, NEW = "irice", "tessera"
OLD_CHECKOUT, NEW_CHECKOUT = "iRice", "tessera"  # (under your home)
NOTE = "RENAMED.txt"
OLD_NOTE = "MOVED-FROM-IRICE.txt"  # (the 4 Nov note: folded into NOTE)
LINK_AGE = 14 * 24 * 3600  # an old-name link is kept at least this long
HERE = os.path.dirname(os.path.abspath(__file__))

# ── Old names other parts still read until their removal (same date) ────
# The keyring: AI keys (application irice) and iCloud passwords kept under
# the old name are looked up there too, never moved (a move would ask to
# unlock the keyring as the shell starts)
OLD_KEYRING_APP = "irice"
OLD_ICLOUD_SERVICE = "irice-icloud"
# The old name as it was written for people
OLD_DISPLAY_NAME = "iRice"
# A settings backup made before the rename: *.irice, "app": "iRice"
OLD_BACKUP_EXT = ".irice"
OLD_BACKUP_APP = OLD_DISPLAY_NAME
# The system pieces until the root setup (tessera-root.py setup) moves them
OLD_ROOT_HELPER = "/usr/lib/irice/irice-root"
OLD_ROOT_PREFIX = "irice-root: "
OLD_ETC = "/etc/irice"
OLD_DBUS_CONF = "zzz-irice-quickshare"
OLD_FP_MARK = "# iRice fingerprint"
OLD_NFT = ("/etc/nftables.d/irice.nft", "/etc/nftables.d/irice.conf")
# Software Update: where the shortcuts were in a version from before
OLD_KEYMAP = "hypr/irice/keymap.json"


def system_path(path, env=None):
    """path under TESSERA_SYSTEM_ROOT (the tests' stand-in for /)."""
    env = os.environ if env is None else env
    return os.path.join(env.get("TESSERA_SYSTEM_ROOT", "/"), path.lstrip("/"))


def root_helper(env=None):
    """The root helper to run: Tessera's, else the old one while only it is
    installed (the root setup hasn't run yet)."""
    new = system_path("/usr/lib/tessera/tessera-root", env)
    old = system_path(OLD_ROOT_HELPER, env)
    return old if not os.access(new, os.X_OK) and os.access(old, os.X_OK) else new


def without_helper_prefix(text):
    """A root helper's message without its name in front."""
    text = str(text or "")
    for p in ("tessera-root: ", OLD_ROOT_PREFIX):
        text = text.replace(p, "")
    return text


# ── Folders ─────────────────────────────────────────────────────────────
def bases(env=None):
    """The four XDG folders, as (what, folder): the session's, then your
    home's own when the session points elsewhere (a console session's
    sandbox home), each once."""
    env = os.environ if env is None else env
    home = env.get("HOME") or os.path.expanduser("~")
    out, seen = [], set()
    for key, default, what in (("XDG_CONFIG_HOME", ".config", "config"), ("XDG_DATA_HOME", ".local/share", "data"),
                               ("XDG_STATE_HOME", ".local/state", "state"), ("XDG_CACHE_HOME", ".cache", "cache")):
        for i, folder in enumerate((env.get(key) or os.path.join(home, default), os.path.join(home, default))):
            real = os.path.realpath(folder)
            if real in seen:
                continue
            seen.add(real)
            out.append((what if i == 0 else "home " + what, folder))
    return out


def _spare(path):
    """path, or path.2, path.3 … : the first name that's free."""
    if not os.path.lexists(path):
        return path
    n = 2
    while os.path.lexists(f"{path}.{n}"):
        n += 1
    return f"{path}.{n}"


def note(folder, lines):
    """Adds to the folder's RENAMED.txt (and folds the 4 Nov note into it)."""
    if not os.path.isdir(folder) or not lines:
        return
    try:
        old = os.path.join(folder, OLD_NOTE)
        before = ""
        if os.path.isfile(old) and not os.path.islink(old):
            with open(old, encoding="utf-8", errors="replace") as f:
                before = f.read()
            os.unlink(old)
        with open(os.path.join(folder, NOTE), "a", encoding="utf-8") as f:
            f.write(before)
            f.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')}: Tessera's old name was iRice.\n")
            for line in lines:
                f.write(f"  {line}\n")
    except OSError:
        pass


def migrate_one(base, dry=False):
    """Moves base/irice to base/tessera. Returns what happened: "none",
    "linked" (already done), "moved", "merged" or "error: …"."""
    old, new = os.path.join(base, OLD), os.path.join(base, NEW)
    if os.path.islink(old) or not os.path.isdir(old):
        return "linked" if os.path.islink(old) else "none"
    if dry:
        return "would merge" if os.path.isdir(new) else "would move"
    try:
        if not os.path.lexists(new):
            try:
                os.rename(old, new)
            except FileNotFoundError:
                return "linked"  # (another start moved it just now)
            try:
                os.symlink(NEW, old)
            except FileExistsError:
                pass
            retarget_links(new, old, new)
            note(new, [f"Moved here from {old}; that name is a link here for now."])
            return "moved"
        if not os.path.isdir(new):
            return f"error: {new} isn't a folder"
        moved, kept = [], []
        for name in sorted(os.listdir(old)):
            src, dst = os.path.join(old, name), os.path.join(new, name)
            if name == OLD_NOTE or os.path.lexists(dst):
                kept.append(name)
                continue
            os.rename(src, dst)
            moved.append(name)
        lines = [f"Both {old} and {new} existed: moved over from the old one: {', '.join(moved) or 'nothing'}."]
        if kept:
            spare = _spare(os.path.join(base, OLD + ".before-" + NEW))
            os.rename(old, spare)
            lines.append(f"In both, so this folder's copy was kept and the old one is in {spare}: {', '.join(kept)}.")
        else:
            os.rmdir(old)
        try:
            os.symlink(NEW, old)
        except FileExistsError:
            pass
        retarget_links(new, old, new)
        note(new, lines)
        return "merged"
    except OSError as e:
        return f"error: {e}"


def migrate(env=None, dry=False):
    """Each XDG folder: {what: result}."""
    return {what: migrate_one(base, dry) for what, base in bases(env)}


# ── The checkout: ~/iRice → ~/tessera ───────────────────────────────────
def is_checkout(path):
    return os.path.isfile(os.path.join(path, "shell", "shell.qml")) and os.path.exists(os.path.join(path, ".git"))


def checkouts(env=None, given=()):
    """The checkouts this install runs from: the ones given, this script's
    own (when it's in one), and the ones the installer noted (…/source)."""
    env = os.environ if env is None else env
    found = list(given)
    mine = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
    if is_checkout(mine):
        found.append(mine)
    for what, base in bases(env):
        if not what.endswith("data"):
            continue
        for name in (NEW, OLD):
            try:
                with open(os.path.join(base, name, "source"), encoding="utf-8") as f:
                    found.append(f.read().strip())
            except OSError:
                pass
    out = []
    for c in found:
        if c and c not in out:
            out.append(c)
    return out


def move_checkout(path, env=None, dry=False, run=subprocess.run):
    """~/iRice (the checkout itself, not a link to it) to ~/tessera, the old
    name left as a link. Returns (result, old, new): "none", "moved",
    "would move" or "error: …"."""
    env = os.environ if env is None else env
    home = env.get("HOME") or os.path.expanduser("~")
    old, new = os.path.join(home, OLD_CHECKOUT), os.path.join(home, NEW_CHECKOUT)
    if os.path.islink(old) or not os.path.isdir(old) or os.path.realpath(path) != os.path.realpath(old) or not is_checkout(old):
        return "none", old, new
    if os.path.lexists(new):
        return f"error: {new} is there already, so {old} stays where it is", old, new
    if dry:
        return "would move", old, new
    try:
        os.rename(old, new)
    except FileNotFoundError:
        return "none", old, new  # (another start moved it just now)
    except OSError as e:
        return f"error: {e}", old, new
    try:
        os.symlink(NEW_CHECKOUT, old)
    except FileExistsError:
        pass
    # (worktrees of it find their main checkout at its new place)
    try:
        run(["git", "-C", new, "worktree", "repair"], capture_output=True, timeout=30, check=False)
    except (OSError, subprocess.SubprocessError):
        pass
    return "moved", old, new


def path_pattern(old):
    """old as a whole path (old, old/…), never a longer name (old2)."""
    return re.compile(re.escape(old) + r"(?=$|[/\"'\s:;,)\]}])")


def rewrite_text(path, old, new, limit=1 << 20):
    """Points old's path at new in a text file. Returns True when changed."""
    try:
        if os.path.islink(path) or not os.path.isfile(path) or os.path.getsize(path) > limit:
            return False
        with open(path, "rb") as f:
            data = f.read()
        if b"\0" in data or old.encode() not in data:
            return False
        text = data.decode("utf-8")
    except (OSError, UnicodeDecodeError):
        return False
    changed = path_pattern(old).sub(new.replace("\\", "\\\\"), text)
    if changed == text:
        return False
    tmp = path + ".rename-tmp"
    try:
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(changed)
        shutil.copymode(path, tmp)
        os.replace(tmp, path)
    except OSError:
        if os.path.exists(tmp):
            os.unlink(tmp)
        return False
    return True


def retarget_links(top, old, new, max_depth=6):
    """Links under top whose (absolute) target is in old point into new."""
    done = []
    pat = path_pattern(old)
    top_depth = top.rstrip("/").count("/")
    for d, dirs, files in os.walk(top):
        if d.count("/") - top_depth >= max_depth:
            dirs[:] = []
        for name in dirs + files:
            p = os.path.join(d, name)
            if not os.path.islink(p):
                continue
            try:
                target = os.readlink(p)
            except OSError:
                continue
            if target.startswith("/") and pat.match(target):
                try:
                    tmp = p + ".rename-tmp"
                    os.symlink(new + target[len(old):], tmp)
                    os.replace(tmp, p)
                    done.append(p)
                except OSError:
                    pass
    return done


def text_files(top, skip=("backups",), max_depth=4):
    """Text-ish files under top (settings, units, app configs), not in skip."""
    top_depth = top.rstrip("/").count("/")
    for d, dirs, files in os.walk(top):
        dirs[:] = [x for x in dirs if x not in skip and not os.path.islink(os.path.join(d, x))]
        if d.count("/") - top_depth >= max_depth:
            dirs[:] = []
        for name in files:
            yield os.path.join(d, name)


def repoint(old, new, env=None, run=subprocess.run):
    """Paths to the old checkout saved in settings and Tessera's files point
    at the new one. Returns the files changed."""
    env = os.environ if env is None else env
    home = env.get("HOME") or os.path.expanduser("~")
    changed = []
    files = []
    sandbox = os.path.join(new, ".sandbox", "home")
    if os.path.isdir(sandbox):
        files += list(text_files(os.path.join(sandbox, ".config")))
        files += [os.path.join(sandbox, ".local/share", n, "source") for n in (NEW, OLD)]
    for what, base in bases(env):
        if what.endswith("config"):
            files += [os.path.join(base, NEW, n) for n in ("config.json", "keybinds.json")]
            files += [os.path.join(base, "qt6ct", "qt6ct.conf")]
        if what.endswith("data"):
            files += [os.path.join(base, NEW, "source")]
            apps = os.path.join(base, "applications")
            if os.path.isdir(apps):
                files += [os.path.join(apps, n) for n in os.listdir(apps) if n.startswith(NEW)]
    units = os.path.join(home, ".config", "systemd", "user")
    if os.path.isdir(units):
        files += [os.path.join(units, n) for n in os.listdir(units) if n.startswith(NEW)]
    for f in dict.fromkeys(files):
        if rewrite_text(f, old, new):
            changed.append(f)
    if any(f.startswith(units + os.sep) for f in changed):
        try:  # (the user manager reads the changed units again; running ones go on as they are)
            run(["systemctl", "--user", "daemon-reload"], capture_output=True, timeout=30, check=False)
        except (OSError, subprocess.SubprocessError):
            pass
    tops = [os.path.join(sandbox, ".config"), os.path.join(sandbox, ".local/share", NEW)]
    tops += [os.path.join(base, NEW) for what, base in bases(env) if what.endswith("data")]
    for top in tops:
        if os.path.isdir(top):
            changed += retarget_links(top, old, new)
    return changed


# ── Settings and files saved under the old name ─────────────────────────
def _json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def _write_json(path, data):
    tmp = path + ".rename-tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=4)
        f.write("\n")
    shutil.copymode(path, tmp)
    os.replace(tmp, path)


def settings(folder):
    """Values saved under the old name in a Tessera settings folder (config,
    keybinds, spaces.lua) or state folder (state.json). Returns what changed."""
    done = []
    cfg = os.path.join(folder, "config.json")
    c = _json(cfg)
    if isinstance(c, dict):
        app, shot = c.get("appearance"), c.get("screenshot")
        hit = False
        if isinstance(app, dict) and app.get("menuIcon") == OLD:
            app["menuIcon"] = NEW
            hit = True
        if isinstance(shot, dict) and shot.get("editor") == OLD:
            shot["editor"] = NEW
            hit = True
        if hit:
            _write_json(cfg, c)
            done.append(cfg)
    keys = os.path.join(folder, "keybinds.json")
    try:
        with open(keys, encoding="utf-8") as f:
            text = f.read()
        if "$IRICE_SHELL" in text:
            data = json.loads(text.replace("$IRICE_SHELL", "$TESSERA_SHELL"))
            _write_json(keys, data)
            done.append(keys)
    except (OSError, ValueError):
        pass
    spaces = os.path.join(folder, "spaces.lua")
    if rewrite_word(spaces, "IRICE_SPACE_BASES", "TESSERA_SPACE_BASES"):
        done.append(spaces)
    state = os.path.join(folder, "state.json")
    s = _json(state)
    if isinstance(s, dict) and "_irice_revisions" in s:
        old = s.pop("_irice_revisions")
        if not isinstance(s.get("_tessera_revisions"), dict) and isinstance(old, dict):
            s["_tessera_revisions"] = old
        _write_json(state, s)
        done.append(state)
    return done


def rewrite_word(path, old, new):
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
    except (OSError, UnicodeDecodeError):
        return False
    if old not in text:
        return False
    tmp = path + ".rename-tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(text.replace(old, new))
    shutil.copymode(path, tmp)
    os.replace(tmp, path)
    return True


THEME_INCLUDES = ("gtk-3.0/gtk.css", "gtk-4.0/gtk.css", "foot/foot.ini", "kitty/kitty.conf", "alacritty/alacritty.toml")
THEME_OLD_FILES = ("gtk-3.0/irice-colors.css", "gtk-4.0/irice-colors.css", "foot/irice-colors.ini", "kitty/irice-colors.conf",
                   "alacritty/irice-colors.toml", "qt6ct/colors/iRice.conf")


def user_files(env=None, run=subprocess.run):
    """What the installer and the shell wrote under the old name in your
    own folders. Returns what changed."""
    env = os.environ if env is None else env
    home = env.get("HOME") or os.path.expanduser("~")
    done = []
    for what, base in bases(env):
        if what.endswith("config"):
            hypr = os.path.join(base, "hypr")
            # Hyprland's files the installer put in hypr/irice
            o, n = os.path.join(hypr, OLD), os.path.join(hypr, NEW)
            if os.path.isdir(o) and not os.path.islink(o) and not os.path.lexists(n):
                os.rename(o, n)
                os.symlink(NEW, o)
                done.append(n)
            # your own Hyprland config, as the installer kept it (the uninstaller puts it back)
            for p in (hypr, os.path.join(hypr, "hyprland.conf"), os.path.join(hypr, "hyprland.lua")):
                if os.path.lexists(p + ".pre-irice") and not os.path.lexists(p + ".pre-tessera"):
                    os.rename(p + ".pre-irice", p + ".pre-tessera")
                    done.append(p + ".pre-tessera")
            for a, b in (("irice.fish", "tessera.fish"), ("irice-start-hyprland.fish", "tessera-start-hyprland.fish")):
                o, n = os.path.join(base, "fish/conf.d", a), os.path.join(base, "fish/conf.d", b)
                if os.path.isfile(o) and not os.path.lexists(n):
                    os.rename(o, n)
                    done.append(n)
            # the apps' colours: configs include Tessera's file, the old ones go
            for rel in THEME_INCLUDES:
                if rewrite_word(os.path.join(base, rel), "irice-colors", "tessera-colors"):
                    done.append(os.path.join(base, rel))
            for rel in THEME_OLD_FILES:
                p = os.path.join(base, rel)
                if os.path.isfile(p):
                    os.unlink(p)
                    done.append(p)
            for rel in ("qt6ct/qt6ct.conf",):
                if rewrite_word(os.path.join(base, rel), "/colors/iRice.conf", "/colors/Tessera.conf"):
                    done.append(os.path.join(base, rel))
            for rel, group in (("kdeglobals", "General"), ("dolphinrc", "UiSettings")):
                if color_scheme(os.path.join(base, rel), group):
                    done.append(os.path.join(base, rel))
            done += file_manager_menus(base, None, run)
        if what.endswith("data"):
            p = os.path.join(base, "color-schemes", "iRice.colors")
            if os.path.isfile(p):
                os.unlink(p)
                done.append(p)
            done += file_manager_menus(None, base, run)
            # builds under the old name: the events helper under Tessera's
            # name when there's no build of it under that (the same program),
            # else it goes; the old glass plugin goes (it answers only to the
            # old name: the next build, or Software Update, makes Tessera's)
            t = os.path.join(base, NEW)
            p, n = os.path.join(t, "bin/irice-events"), os.path.join(t, "bin/tessera-events")
            if os.path.isfile(p) and not os.path.islink(p):
                if not os.path.lexists(n):
                    os.rename(p, n)
                    done.append(n)
                else:
                    os.unlink(p)
                    done.append(p)
            for old in ("irice-glass.so", "irice-glass.built-for", "irice-glass.so.new", "bin/irice-events.new"):
                p = os.path.join(t, old)
                if os.path.isfile(p) and not os.path.islink(p):
                    os.unlink(p)
                    done.append(p)
            # autostart entries written for the old name's desktop
            done += autostart(os.path.join(home, ".config", "autostart"))
    # the command under its old name (the installer puts in `tessera`)
    cmd = os.path.join(home, ".local/bin", OLD)
    try:
        if os.path.isfile(cmd) and not os.path.islink(cmd) and os.path.getsize(cmd) < (1 << 20):
            with open(cmd, encoding="utf-8", errors="replace") as f:
                ours = "iRice" in f.read()
            if ours:
                os.unlink(cmd)
                done.append(cmd)
    except OSError:
        pass
    # the battery guard's unit under its old name (the shell installs Tessera's)
    unit = os.path.join(home, ".config/systemd/user/irice-battery-guard.service")
    if os.path.isfile(unit):
        try:
            run(["systemctl", "--user", "disable", "--now", "irice-battery-guard.service"], capture_output=True, timeout=20, check=False)
        except (OSError, subprocess.SubprocessError):
            pass
        os.unlink(unit)
        try:
            run(["systemctl", "--user", "daemon-reload"], capture_output=True, timeout=20, check=False)
        except (OSError, subprocess.SubprocessError):
            pass
        done.append(unit)
    return done


def color_scheme(path, group):
    """An ini's ColorScheme=iRice… (in [group]) is Tessera's same scheme."""
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().split("\n")
    except (OSError, UnicodeDecodeError):
        return False
    cur, hit = None, False
    for i, line in enumerate(lines):
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            cur = s[1:-1]
        elif cur == group and s.startswith("ColorScheme=iRice"):
            lines[i] = line.replace("ColorScheme=iRice", "ColorScheme=Tessera", 1)
            hit = True
    if hit:
        tmp = path + ".rename-tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
        shutil.copymode(path, tmp)
        os.replace(tmp, path)
    return hit


MENUS = {  # file-manager-share.py's menus under the old name: manager -> its files
    "nautilus": ("nautilus-python/extensions/irice-share.py",),
    "dolphin": ("kio/servicemenus/irice-share.desktop",),
    "nemo": ("nemo/actions/irice-share.nemo_action",),
}


def file_manager_menus(config, data, run=subprocess.run):
    """"Send to a nearby device…" menus written under the old name: added
    again under Tessera's (file-manager-share.py), the old ones gone."""
    done, again = [], []
    if data:
        for manager, rels in MENUS.items():
            for rel in rels:
                p = os.path.join(data, rel)
                if os.path.isfile(p):
                    os.unlink(p)
                    done.append(p)
                    again.append(manager)
    if config:
        uca = os.path.join(config, "Thunar", "uca.xml")
        try:
            import xml.etree.ElementTree as ET
            tree = ET.parse(uca)
            olds = [a for a in tree.getroot().findall("action") if (a.findtext("unique-id") or "") == "irice-share"]
            if olds:
                for a in olds:
                    tree.getroot().remove(a)
                tree.write(uca, encoding="utf-8", xml_declaration=True)
                done.append(uca)
                again.append("thunar")
        except (OSError, ImportError, Exception):  # (no file, or one that isn't ours to fix)
            pass
    if again:
        try:
            run([sys.executable, "-I", os.path.join(HERE, "file-manager-share.py"), "install", *dict.fromkeys(again)],
                capture_output=True, timeout=60, check=False)
        except (OSError, subprocess.SubprocessError):
            pass
    return done


def autostart(folder):
    """OnlyShowIn=/NotShowIn= naming the old desktop name name Tessera."""
    done = []
    try:
        names = [n for n in os.listdir(folder) if n.endswith(".desktop")]
    except OSError:
        return done
    pat = re.compile(r"^((?:OnlyShowIn|NotShowIn)=)(.*)$", re.M)
    for n in names:
        p = os.path.join(folder, n)
        try:
            with open(p, encoding="utf-8") as f:
                text = f.read()
        except (OSError, UnicodeDecodeError):
            continue
        new = pat.sub(lambda m: m.group(1) + ";".join("Tessera" if w.lower() == OLD else w for w in m.group(2).split(";")), text)
        if new != text and not os.path.islink(p):
            with open(p + ".rename-tmp", "w", encoding="utf-8") as f:
                f.write(new)
            os.replace(p + ".rename-tmp", p)
            done.append(p)
    return done


OLD_REMOTE = re.compile(r"^(?P<pre>(?:https://|git@|ssh://git@)(?:github|gitlab)\.com[:/]monishbl/)iRice(?P<post>(?:\.git)?/?)$", re.I)


def remotes(path, run=subprocess.run):
    """The checkout's remotes at the repository's old address (GitHub
    redirects it) point at its own: the updater's mirrors only count a
    remote of Tessera's own (tools/tessera_sources.py)."""
    done = []
    try:
        out = run(["git", "-C", path, "remote", "-v"], capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return done
    seen = set()
    for line in out.splitlines():
        parts = line.split()
        if len(parts) < 2 or (parts[0], parts[1]) in seen:
            continue
        seen.add((parts[0], parts[1]))
        m = OLD_REMOTE.match(parts[1])
        if not m:
            continue
        url = m.group("pre") + "tessera" + m.group("post")
        push = len(parts) > 2 and parts[2] == "(push)"
        args = ["git", "-C", path, "remote", "set-url"] + (["--push"] if push else []) + [parts[0], url, parts[1]]
        try:
            if run(args, capture_output=True, timeout=30).returncode == 0:
                done.append(f"{parts[0]} {url}")
        except (OSError, subprocess.SubprocessError):
            pass
    return done


def checkout_leftovers(path):
    """Build folders the checkout made under the old name (untracked)."""
    done = []
    for rel in ("hypr/plugin/irice-glass", "shell/native/build/qml/IRice"):
        p = os.path.join(path, rel)
        if os.path.isdir(p) and not os.path.islink(p):
            try:
                tracked = subprocess.run(["git", "-C", path, "ls-files", "--", rel], capture_output=True, text=True, timeout=30).stdout.strip()
            except (OSError, subprocess.SubprocessError):
                continue
            if not tracked:
                shutil.rmtree(p, ignore_errors=True)
                done.append(p)
    return done


# ── Old-name links: gone once nothing uses them ─────────────────────────
def in_use(path, proc="/proc"):
    """Does any process of yours have path (or something under it) open as
    its folder or program, or on its command line or in its environment?"""
    me = os.getuid()
    needle = path.encode()
    try:
        pids = [p for p in os.listdir(proc) if p.isdigit()]
    except OSError:
        return True
    for pid in pids:
        d = os.path.join(proc, pid)
        try:
            if os.stat(d).st_uid != me:
                continue
            for link in ("cwd", "exe"):
                try:
                    t = os.readlink(os.path.join(d, link))
                    if t == path or t.startswith(path + "/"):
                        return True
                except OSError:
                    pass
            for f in ("cmdline", "environ"):
                try:
                    with open(os.path.join(d, f), "rb") as fh:
                        if needle in fh.read():
                            return True
                except OSError:
                    pass
        except OSError:
            continue
    return False


def mentioned(path, env=None):
    """Is path named in a login session entry, your Hyprland files or your
    user units?"""
    env = os.environ if env is None else env
    home = env.get("HOME") or os.path.expanduser("~")
    places = [system_path("/usr/share/wayland-sessions", env), os.path.join(home, ".config/systemd/user")]
    for what, base in bases(env):
        if what.endswith("config"):
            places += [os.path.join(base, "hypr"), os.path.join(base, "hypr", NEW)]
    pat = path_pattern(path)
    for place in places:
        try:
            names = os.listdir(place)
        except OSError:
            continue
        for n in names:
            f = os.path.join(place, n)
            try:
                if os.path.isfile(f) and os.path.getsize(f) < (1 << 20):
                    with open(f, encoding="utf-8", errors="replace") as fh:
                        if pat.search(fh.read()):
                            return True
            except OSError:
                continue
    return False


def old_link(path, target, env=None, now=None, proc="/proc", dry=False):
    """Removes the old-name link path (→ target) once it's old enough and
    unused. Returns "removed", "kept" or "none"."""
    if not os.path.islink(path):
        return "none"
    try:
        if os.path.realpath(path) != os.path.realpath(target):
            return "none"  # (not ours: a link you made elsewhere)
        age = (time.time() if now is None else now) - os.lstat(path).st_mtime
    except OSError:
        return "none"
    if age < LINK_AGE or in_use(path, proc) or mentioned(path, env):
        return "kept"
    if dry:
        return "would remove"
    try:
        os.unlink(path)
    except OSError:
        return "kept"
    return "removed"


# ── All of it ───────────────────────────────────────────────────────────
def run_all(env=None, dry=False, given=(), run=subprocess.run, now=None, proc="/proc", move=True):
    """Everything, once (move=False: never the checkout itself, for a build
    running in it). Returns {step: result} for what did something."""
    env = os.environ if env is None else env
    out = {}
    for what, result in migrate(env, dry).items():
        if result not in ("none", "linked"):
            out[what] = result
    homes = env.get("HOME") or os.path.expanduser("~")
    for c in checkouts(env, given) if move else ():
        result, old, new = move_checkout(c, env, dry, run)
        if result != "none":
            out["checkout"] = result
        if result == "moved":
            changed = repoint(old, new, env, run)
            # (the note goes with the settings: never into the checkout itself)
            for _, base in bases(env):
                if _.endswith("config") and os.path.isdir(os.path.join(base, NEW)):
                    note(os.path.join(base, NEW), [f"Tessera's folder moved from {old} to {new}; {old} is a link to it for now.",
                                                   *(f"Now points at {new}: {f}" for f in changed)])
                    break
    if dry:
        return out
    real_checkout = os.path.join(homes, NEW_CHECKOUT)
    # every checkout this install runs from (by its real path), and ~/tessera
    mine = []
    for c in checkouts(env, given) + [real_checkout]:
        r = os.path.realpath(c)
        if is_checkout(r) and r not in mine:
            mine.append(r)
    sandboxes = [os.path.join(c, ".sandbox/home", sub) for c in mine for sub in (".config", ".local/share", ".local/state", ".cache")]
    for b in sandboxes:  # (a console session's own folders: moved as its shell starts, else here)
        r = migrate_one(b)
        if r not in ("none", "linked"):
            out[b] = r
    done = []
    try:
        for what, base in bases(env):
            if what.endswith(("config", "state")):
                done += settings(os.path.join(base, NEW))
        for b in sandboxes:
            if os.path.isdir(os.path.join(b, NEW)) and b.endswith((".config", ".local/state")):
                done += settings(os.path.join(b, NEW))
        for c in mine:
            done += checkout_leftovers(c)
            done += remotes(c, run)
        done += user_files(env, run)
    except OSError as e:
        out["files"] = f"error: {e}"
    # the 4 Nov notes, folded into RENAMED.txt
    for folder in [os.path.join(base, NEW) for _, base in bases(env)] + [os.path.join(b, NEW) for b in sandboxes]:
        if os.path.isfile(os.path.join(folder, OLD_NOTE)):
            note(folder, ["(the notes above are from the first move)"])
            done.append(os.path.join(folder, OLD_NOTE))
    if done:
        out["files"] = f"{len(done)} renamed"
    # links from the move (this one or 4 Nov's), once nothing uses them
    links = [(os.path.join(base, OLD), os.path.join(base, NEW)) for _, base in bases(env)]
    links += [(os.path.join(b, OLD), os.path.join(b, NEW)) for b in sandboxes]
    links.append((os.path.join(homes, OLD_CHECKOUT), real_checkout))
    for what, base in bases(env):
        if what.endswith("config"):
            links.append((os.path.join(base, "hypr", OLD), os.path.join(base, "hypr", NEW)))
    for link, target in links:
        if old_link(link, target, env, now, proc) == "removed":
            out[link] = "removed"
    return out


def main(argv):
    dry = "--dry-run" in argv
    given = [argv[i + 1] for i, a in enumerate(argv[:-1]) if a == "--checkout"]
    for what, result in run_all(dry=dry, given=given, move="--no-checkout" not in argv).items():
        print(f"migrate-names: {what}: {result}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Exception as e:  # never stop a shell from starting
        print(f"migrate-names: {e}", file=sys.stderr)
        sys.exit(0)
