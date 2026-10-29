#!/usr/bin/env python3
"""Keep desktop light/dark preferences and Dolphin's KDE palette in sync.

Independent of matugen and Match apps. --scheme applies a generated KDE
scheme instead; --keep-kde only updates the desktop light/dark preference.
Only palette keys are changed; fonts, icons, styles and other settings stay.
"""
import argparse
import configparser
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def update_ini(path, groups):
    """Update selected keys without rewriting unrelated settings or comments.

    Returns whether the file changed, so apps are only told to reload (which
    repaints every KDE window) when something really did.
    """
    path = Path(path).resolve()  # keep symlinked dotfiles linked to their source
    text = path.read_text() if path.exists() else ""
    for group, entries in groups.items():
        header = re.compile(r"^" + re.escape("[" + group + "]") + r"\s*$", re.M)
        match = header.search(text)
        if match:
            start = match.end()
            following = re.search(r"^\[", text[start:], re.M)
            end = start + following.start() if following else len(text)
            body = text[start:end]
            for key, value in entries.items():
                pattern = re.compile(r"^" + re.escape(key) + r"\s*=.*$", re.M)
                line = key + "=" + value
                if pattern.search(body):
                    body = pattern.sub(lambda _m: line, body)
                else:
                    body = body.rstrip("\n") + "\n" + line + "\n"
            text = text[:start] + body + text[end:]
        else:
            text = text.rstrip("\n") + "\n\n[" + group + "]\n"
            text += "".join(key + "=" + value + "\n" for key, value in entries.items())
    if path.exists() and path.read_text() == text:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=".irice-theme-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            stream.write(text.lstrip("\n"))
        if path.exists():
            shutil.copymode(path, temporary)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return True


def current(path, group, key):
    """A setting's value in an ini file, or "" when it isn't there."""
    parser = configparser.ConfigParser(interpolation=None, strict=False)
    parser.optionxform = str
    try:
        parser.read(path)
        return parser.get(group, key, fallback="")
    except configparser.Error:
        return ""


# Schemes iRice may switch between; anything else was chosen by the user and
# is left alone (a palette set in KDE's settings or qt6ct isn't ours to undo)
OURS = re.compile(r"^(BreezeDark|BreezeLight|Breeze|iRice.*)?$")


def fallback_scheme(path, dark):
    bg, view, button, fg, muted = ("30,30,33", "24,24,27", "48,48,52", "235,235,240", "160,160,168") if dark else ("245,245,247", "255,255,255", "235,235,240", "30,30,33", "100,100,108")
    groups = {"General": {"Name": "iRice Dark" if dark else "iRice Light", "ColorScheme": path.stem}}
    for name, background in [("Window", bg), ("View", view), ("Button", button), ("Tooltip", button), ("Header", bg), ("Complementary", bg), ("Selection", "0,100,210")]:
        groups["Colors:" + name] = {
            "BackgroundNormal": background, "BackgroundAlternate": button,
            "ForegroundNormal": "255,255,255" if name == "Selection" else fg,
            "ForegroundInactive": "225,225,230" if name == "Selection" else muted,
            "ForegroundActive": fg, "ForegroundLink": "80,160,255" if dark else "0,90,190",
            "ForegroundVisited": "180,130,230", "ForegroundNegative": "230,80,80",
            "ForegroundNeutral": "200,140,30", "ForegroundPositive": "40,170,90",
            "DecorationFocus": "0,122,255", "DecorationHover": "0,122,255",
        }
    update_ini(path, groups)


def apply_kde(mode, scheme=None):
    cfg = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    data = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
    if scheme is None:
        name = "BreezeDark" if mode == "dark" else "BreezeLight"
        roots = [data] + [Path(p) for p in os.environ.get("XDG_DATA_DIRS", "/usr/local/share:/usr/share").split(":") if p]
        scheme = next((p / "color-schemes" / (name + ".colors") for p in roots if (p / "color-schemes" / (name + ".colors")).is_file()), None)
        if scheme is None:
            scheme = data / "color-schemes" / ("iRiceDesktopDark.colors" if mode == "dark" else "iRiceDesktopLight.colors")
            fallback_scheme(scheme, mode == "dark")
    scheme = Path(scheme).resolve()
    palette = configparser.ConfigParser(interpolation=None)
    palette.optionxform = str
    palette.read(scheme)
    groups = {s: dict(palette[s]) for s in palette.sections() if s.startswith(("Colors:", "ColorEffects:"))}
    if "Colors:View" not in groups or "ForegroundNormal" not in groups["Colors:View"]:
        raise ValueError("Incomplete KDE color scheme: " + str(scheme))
    groups["General"] = {"ColorScheme": scheme.stem}
    changed = False
    if OURS.match(current(cfg / "kdeglobals", "General", "ColorScheme")):
        changed |= update_ini(cfg / "kdeglobals", groups)
    # KColorSchemeManager also reads Dolphin's per-app scheme, independently
    # of the Qt style and kdeglobals. Keep both on the same complete palette.
    if OURS.match(current(cfg / "dolphinrc", "UiSettings", "ColorScheme")):
        changed |= update_ini(cfg / "dolphinrc", {"UiSettings": {"ColorScheme": scheme.stem}})
    # Qt apps using qt6ct must use the same palette as KDE widgets.
    def color(group, key):
        value = groups["Colors:" + group][key]
        if value.startswith("#"):
            return value
        return "#" + "".join(f"{int(v):02x}" for v in value.split(",")[:3])
    fg = color("Window", "ForegroundNormal")
    bg = color("Window", "BackgroundNormal")
    colors = [fg, color("Button", "BackgroundNormal"), bg, bg, bg, bg,
              color("View", "ForegroundNormal"), fg, color("Button", "ForegroundNormal"),
              color("View", "BackgroundNormal"), bg, "#000000",
              color("Selection", "BackgroundNormal"), color("Selection", "ForegroundNormal"),
              groups["Colors:View"].get("ForegroundLink", fg), groups["Colors:View"].get("ForegroundVisited", fg),
              color("View", "BackgroundAlternate"), "#000000", color("Tooltip", "BackgroundNormal"),
              color("Tooltip", "ForegroundNormal"), color("Window", "ForegroundInactive"), color("Selection", "BackgroundNormal")]
    # Link colors can be RGB triples too.
    for i, key in [(14, "ForegroundLink"), (15, "ForegroundVisited")]:
        if key in groups["Colors:View"]:
            colors[i] = color("View", key)
    disabled = colors.copy()
    for i in [0, 6, 8, 19]:
        disabled[i] = colors[20]
    qt_palette = cfg / "qt6ct/colors/iRiceDesktop.conf"
    # qt6ct: only when no palette is chosen, or one of ours (iRice.conf from
    # Match apps, or this one)
    chosen = current(cfg / "qt6ct/qt6ct.conf", "Appearance", "color_scheme_path")
    if not chosen or Path(chosen).name.startswith("iRice"):
        changed |= update_ini(qt_palette, {"ColorScheme": {"active_colors": ", ".join(colors), "inactive_colors": ", ".join(colors), "disabled_colors": ", ".join(disabled)}})
        changed |= update_ini(cfg / "qt6ct/qt6ct.conf", {"Appearance": {"custom_palette": "true", "color_scheme_path": str(qt_palette)}})
    # Existing KDE apps that watch the desktop palette can reload it.
    if changed and shutil.which("gdbus"):
        subprocess.run(["gdbus", "emit", "--session", "--object-path", "/KGlobalSettings", "--signal", "org.kde.KGlobalSettings.notifyChange", "0", "0"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5, check=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["dark", "light"])
    parser.add_argument("--scheme", type=Path)
    parser.add_argument("--keep-kde", action="store_true")
    args = parser.parse_args()
    if shutil.which("gsettings"):
        subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "color-scheme", "prefer-" + args.mode], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5, check=False)
    if not args.keep_kde:
        apply_kde(args.mode, args.scheme)


if __name__ == "__main__":
    main()
