#!/usr/bin/env python3
"""What your old hyprland.conf set, for install.sh: it moves that file aside
(hyprland.conf.pre-irice) and iRice's Lua config takes over, so the
displays, keyboard layout, environment and programs you had there are
written as suggestions, every line commented out, to a file
hyprland.lua loads (~/.config/hypr/irice/from-your-old-config.lua).
Nothing is applied until you remove the "-- " in front of a line.

  old-hypr-config.py <hyprland.conf> <out.lua>

Prints a short summary (nothing when there was nothing to carry over, and
then writes no file). Files the config sources (source = …) are read too;
$variables are filled in.
"""
import glob
import os
import re
import sys

# Programs iRice runs itself, or that would fight with it: marked as such
OWN = ("waybar", "ags", "eww", "hyprpanel", "quickshell", "qs ", "swaync", "mako", "dunst", "hypridle", "swayidle",
       "hyprpaper", "swww", "swaybg", "wl-paste", "cliphist", "polkit", "gnome-keyring", "nm-applet", "blueman-applet",
       "dbus-update-activation-environment", "systemctl --user import-environment", "hyprpm reload", "nwg-dock")
NVIDIA = ("LIBVA_DRIVER_NAME", "__GLX_VENDOR_LIBRARY_NAME", "GBM_BACKEND", "NVD_BACKEND", "WLR_NO_HARDWARE_CURSORS",
          "__GL_GSYNC_ALLOWED", "__GL_VRR_ALLOWED")


def strip_comment(line):
    """Hyprland's comments start at "#"; "##" is a literal "#"."""
    line = line.replace("##", "\0")
    return line.split("#", 1)[0].replace("\0", "#").strip()


def read(path, seen, depth=0):
    """The config's lines, sourced files in their place."""
    path = os.path.realpath(path)
    if depth > 5 or path in seen:
        return []
    seen.add(path)
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            lines = f.read().splitlines()
    except OSError:
        return []
    out = []
    for raw in lines:
        line = strip_comment(raw)
        m = re.match(r"^source\s*=\s*(.+)$", line)
        if m:
            target = os.path.expanduser(m.group(1).strip())
            if not os.path.isabs(target):
                target = os.path.join(os.path.dirname(path), target)
            for p in sorted(glob.glob(target)) or [target]:
                out += read(p, seen, depth + 1)
            continue
        if line:
            out.append(line)
    return out


def parse(lines):
    """{"monitor": [...], "env": [(name, value)], "exec": [...], "input": {key: value}}"""
    found = {"monitor": [], "env": [], "exec": [], "input": {}}
    variables = {}
    stack = []

    def fill(v):
        # longest names first ($term before $t)
        for k in sorted(variables, key=len, reverse=True):
            v = v.replace("$" + k, variables[k])
        return v
    for line in lines:
        if line.endswith("{"):
            stack.append(line[:-1].strip().split(":")[0].strip())
            continue
        if line == "}":
            if stack:
                stack.pop()
            continue
        key, eq, value = line.partition("=")
        if not eq:
            continue
        key, value = key.strip(), value.strip()
        if key.startswith("$"):
            variables[key[1:]] = fill(value)
            continue
        value = fill(value)
        path = ":".join(stack + [key])
        if path == "monitor":
            found["monitor"].append(value)
        elif path == "env":
            name, _, v = value.partition(",")
            found["env"].append((name.strip(), v.strip()))
        elif path == "exec-once":
            found["exec"].append(value)
        elif path in ("input:kb_layout", "input:kb_variant", "input:kb_options", "input:kb_model"):
            found["input"][path.split(":")[1]] = value
    return found


def lua(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def monitor_lua(value):
    """monitor = name, mode, position, scale → hl.monitor({…}); anything else
    (disable, transform, mirror…) stays as it was, to change by hand."""
    parts = [p.strip() for p in value.split(",")]
    if len(parts) == 4 and parts[1] not in ("disable", "disabled", "addreserved"):
        scale = parts[3] if re.fullmatch(r"\d+(\.\d+)?", parts[3]) else lua(parts[3])
        return f"-- hl.monitor({{ output = {lua(parts[0])}, mode = {lua(parts[1])}, position = {lua(parts[2])}, scale = {scale} }})"
    return f"-- (by hand, from: monitor = {value})"


def write(found, out, source):
    lines = ["-- Settings from your old hyprland.conf (" + source + "), written once by the",
             "-- iRice installer as suggestions: none of them is on. To use one, remove the",
             '-- "-- " in front of it; Hyprland reloads by itself. iRice\'s own settings',
             "-- (Settings → Displays, Keyboard) may already cover them.", ""]
    if found["monitor"]:
        lines += ["-- Displays (Settings → Displays does this too)"] + [monitor_lua(m) for m in found["monitor"]] + [""]
    if found["input"]:
        fields = ", ".join(f"{k} = {lua(v)}" for k, v in sorted(found["input"].items()))
        lines += ["-- Keyboard (the installer already took your layout from localectl)",
                  f"-- hl.config({{ input = {{ {fields} }} }})", ""]
    if found["env"]:
        lines += ["-- Environment"]
        for name, v in found["env"]:
            note = " -- (NVIDIA: gpu.lua sets what iRice needs)" if name in NVIDIA else ""
            lines.append(f"-- hl.env({lua(name)}, {lua(v)}){note}")
        lines.append("")
    if found["exec"]:
        lines += ["-- Programs started with the session"]
        for c in found["exec"]:
            own = any(w in c for w in OWN)
            lines.append(f'-- hl.on("hyprland.start", function() hl.exec_cmd({lua(c)}) end)' +
                         (" -- (iRice does this itself)" if own else ""))
        lines.append("")
    with open(out + ".tmp", "w") as f:
        f.write("\n".join(lines))
    os.replace(out + ".tmp", out)


def summary(found, out):
    said = []
    if found["monitor"]:
        said.append(f"{len(found['monitor'])} display line{'s' * (len(found['monitor']) != 1)}")
    if found["input"].get("kb_layout"):
        layout = found["input"]["kb_layout"]
        said.append("keyboard " + layout + (f" ({found['input']['kb_variant']})" if found["input"].get("kb_variant") else ""))
    if found["env"]:
        nv = [n for n, _ in found["env"] if n in NVIDIA]
        said.append(f"{len(found['env'])} env line{'s' * (len(found['env']) != 1)}" + (f" (NVIDIA: {', '.join(nv)})" if nv else ""))
    if found["exec"]:
        said.append(f"{len(found['exec'])} exec-once")
    home = os.path.expanduser("~")
    shown = out.replace(home, "~", 1) if out.startswith(home + "/") else out
    return f"Your old hyprland.conf had {', '.join(said)}.\nThey're in {shown} as suggestions, turned off: remove \"-- \" to use one."


def main(argv):
    if len(argv) != 2:
        print("usage: old-hypr-config.py <hyprland.conf> <out.lua>", file=sys.stderr)
        return 2
    conf, out = argv
    found = parse(read(conf, set()))
    if not (found["monitor"] or found["env"] or found["exec"] or found["input"]):
        return 0
    write(found, out, os.path.basename(conf))
    print(summary(found, out))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
