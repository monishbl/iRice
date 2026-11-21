#!/usr/bin/env python3
"""The open/save picker follows your Files app.

Apps that ask the desktop for a file picker (GTK, Electron, Flatpak, Firefox
and Chrome on Wayland: through xdg-desktop-portal) got GTK's or GNOME's,
whatever your Files app (21 Nov: Dolphin chosen, Nautilus' picker shown).
This writes Hyprland's portal preferences so the picker is your Files app's:

    Dolphin (or another Qt one) → KDE's picker   (xdg-desktop-portal-kde)
    Nautilus                    → GNOME's picker (xdg-desktop-portal-gnome)
    anything else               → GTK's picker   (xdg-desktop-portal-gtk)

when that portal is installed (else GTK's). The rest stays Hyprland's and
GTK's, as Hyprland's own file says. A hyprland-portals.conf of yours (no
Tessera header) is left alone.

    file-picker.py apply [--app NAME] [--json]    write it (and restart the portal when it changed);
                                     --app: the Files app just chosen ("" Automatic)
    file-picker.py show  [--json]    what it would write, nothing changed
    file-picker.py remove            Tessera's file gone (tessera uninstall)

The Files app is Settings → Desktop & Widgets → Files app
(desktop.fileManager), else the first installed, as the shell picks it
(host/desktop/src/screen/util.rs file_manager).
"""
import json
import os
import shutil
import subprocess
import sys

HEADER = "# Tessera: the file picker follows your Files app"
# (the shell's order for Automatic: util.rs file_manager)
MANAGERS = ("nautilus", "dolphin", "thunar", "nemo", "caja", "pcmanfm-qt", "pcmanfm")
PICKER = {"dolphin": "kde", "pcmanfm-qt": "kde", "nautilus": "gnome"}
PORTALS = os.environ.get("TESSERA_PORTALS_DIR", "/usr/share/xdg-desktop-portal/portals")


def config_dir():
    return os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")


def target():
    return os.path.join(config_dir(), "xdg-desktop-portal", "hyprland-portals.conf")


def files_app(chosen=None):
    """The Files app: CHOSEN (just picked in Settings; "" is Automatic), else
    Settings' choice, else the first installed"""
    want = chosen
    if want is None:
        try:
            with open(os.path.join(config_dir(), "tessera", "config.json")) as f:
                want = (json.load(f).get("desktop") or {}).get("fileManager") or ""
        except (OSError, ValueError, AttributeError):
            want = ""
    if want and shutil.which(want):
        return want
    return next((m for m in MANAGERS if shutil.which(m)), "")


def has_portal(name):
    return os.path.exists(os.path.join(PORTALS, name + ".portal"))


def picker_for(app):
    p = PICKER.get(app, "gtk")
    return p if has_portal(p) else "gtk"


def text_for(app, picker):
    return "\n".join([
        HEADER + " (" + (app or "none installed") + ").",
        "# Written by Tessera when you pick a Files app (Settings → Desktop & Widgets);",
        "# remove this header to keep your own lines here.",
        "[preferred]",
        "default=hyprland;gtk",
        "org.freedesktop.impl.portal.FileChooser=" + picker,
        "",
    ])


def ours(path):
    try:
        with open(path) as f:
            return f.read().startswith(HEADER)
    except FileNotFoundError:
        return True  # (nothing there: ours to write)
    except OSError:
        return False


def restart_portal():
    # (the portal reads its preferences when it starts; a running one keeps
    # the old picker until then)
    subprocess.run(["systemctl", "--user", "try-restart", "xdg-desktop-portal.service"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15, check=False)


def apply(dry=False, chosen=None):
    app = files_app(chosen)
    picker = picker_for(app)
    path = target()
    out = {"app": app, "picker": picker, "path": path, "changed": False}
    if not ours(path):
        out["skipped"] = "your own " + path + " is kept"
        return out
    text = text_for(app, picker)
    try:
        with open(path) as f:
            cur = f.read()
    except OSError:
        cur = None
    if cur == text or dry:
        return out
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tessera-tmp"
    with open(tmp, "w") as f:
        f.write(text)
    os.replace(tmp, path)
    out["changed"] = True
    if os.environ.get("TESSERA_PICKER_NO_RESTART") != "1":
        restart_portal()
    return out


def remove():
    path = target()
    if os.path.exists(path) and ours(path):
        os.remove(path)
        if os.environ.get("TESSERA_PICKER_NO_RESTART") != "1":
            restart_portal()
        return {"removed": path}
    return {}


def main(argv):
    cmd = argv[1] if len(argv) > 1 else "show"
    chosen = argv[argv.index("--app") + 1] if "--app" in argv and argv.index("--app") + 1 < len(argv) else None
    if cmd == "apply":
        out = apply(chosen=chosen)
    elif cmd == "show":
        out = apply(dry=True)
    elif cmd == "remove":
        out = remove()
    else:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    if "--json" in argv:
        print(json.dumps(out))
    elif out.get("skipped"):
        print(out["skipped"])
    elif "picker" in out:
        print("The file picker: " + out["picker"] + "'s (Files app: " + (out["app"] or "none") + ")")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
