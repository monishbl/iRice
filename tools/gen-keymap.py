#!/usr/bin/env python3
"""Writes iRice's default keymap and gestures (hypr/irice/keymap.json and
gestures.json). The defaults follow end4 (illogical-impulse) wherever both
have the same action; iRice-only actions sit on keys end4 leaves free.

Each bind: id (stable, used by the user's changes), keys ("$MOD" is Super, or
Alt in the sandbox), group and label (Settings and the cheatsheet), action,
and options. Actions:
  {"shell": name}   an iRice action (Hyprland global "irice:<name>")
  {"app": role}     the default app for a role (Settings → Default apps)
  {"exec": cmd}     a command
  {"dsp": lua}      a Hyprland dispatcher, as Lua (e.g. hl.dsp.window.close())
  {"zoom": step}    screen zoom in or out
Run again after editing: tools/gen-keymap.py
"""
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
binds = []


def b(id, keys, group, label, action, **opts):
    e = {"id": id, "keys": keys, "group": group, "label": label, "action": action}
    if opts:
        e["opts"] = opts
    binds.append(e)


def shell(n):
    return {"shell": n}


def dsp(l):
    return {"dsp": l}


# ── Shell ──────────────────────────────────────────────────────────────────
# A lone Super tap (release bind; skipped when another key was pressed)
b("spotlight-tap", "$MOD + $MODKEY", "Shell", "Spotlight (tap Super)", shell("spotlight"), release=True)
b("spotlight", "$MOD + Space", "Shell", "Spotlight", shell("spotlight"))
b("overview", "$MOD + Tab", "Shell", "Overview", shell("overview"))
b("switcher", "ALT + Tab", "Shell", "Switch windows", shell("switcher"))
b("switcher-back", "ALT + SHIFT + Tab", "Shell", "Switch windows backwards", shell("switcherBack"))
# Finishes a quick Alt+Tab tap that ends before the switcher has the keyboard
# (a release bind that lets apps still see Alt; does nothing otherwise)
b("switcher-commit", "ALT + Alt_L", "Shell", "Switch windows (finish when Alt is let go)", shell("switcherCommit"), release=True, non_consuming=True, transparent=True, ignore_mods=True)
b("mission-control", "$MOD + SHIFT + Tab", "Shell", "Mission Control", shell("missionControl"))
b("launchpad", "$MOD + SHIFT + Space", "Shell", "App Library", shell("launchpad"))
b("clipboard", "$MOD + V", "Shell", "Clipboard history", shell("clipboard"))
b("emoji", "$MOD + Period", "Shell", "Emoji and symbols", shell("emoji"))
b("sidebar", "$MOD + A", "Shell", "AI and tools sidebar", shell("sidebar"))
b("sidebar-b", "$MOD + B", "Shell", "AI and tools sidebar (alternative)", shell("sidebar"))
b("sidebar-o", "$MOD + O", "Shell", "AI and tools sidebar (alternative)", shell("sidebar"))
b("notifications", "$MOD + N", "Shell", "Notification Center", shell("notificationCenter"))
b("control-center", "$MOD + ALT + C", "Shell", "Control Center", shell("controlCenter"))
b("settings", "$MOD + I", "Shell", "Settings", shell("settings"))
b("settings-comma", "$MOD + Comma", "Shell", "Settings (macOS key)", shell("settings"))
b("cheatsheet", "$MOD + Slash", "Shell", "Keyboard shortcuts", shell("cheatsheet"))
b("keyboard", "$MOD + K", "Shell", "On-screen keyboard", shell("keyboard"))
b("overlay", "$MOD + G", "Shell", "Overlay widgets", shell("overlay"))
b("wallpaper", "CTRL + $MOD + T", "Shell", "Change wallpaper", shell("wallpaper"))
b("dark-mode", "CTRL + $MOD + SHIFT + D", "Shell", "Light or dark appearance", shell("darkMode"))
b("quick-note", "$MOD + ALT + N", "Shell", "Quick note", shell("quickNote"))
b("session", "CTRL + ALT + Delete", "Shell", "Power options", shell("session"))
b("reset", "$MOD + Escape", "Shell", "Close everything (if something is stuck)", shell("reset"))
b("repair", "CTRL + $MOD + R", "Shell", "Restart the shell (apps keep running)", {"exec": "setsid -f \"$IRICE_SHELL/assets/scripts/repair-shell.sh\""})
b("repair-escape", "$MOD + CTRL + Escape", "Shell", "Restart the shell (alternative)", {"exec": "setsid -f \"$IRICE_SHELL/assets/scripts/repair-shell.sh\""})

# ── Screen ─────────────────────────────────────────────────────────────────
b("snip", "$MOD + SHIFT + S", "Screen", "Screenshot of an area", shell("screenshotRegion"))
b("lens", "$MOD + SHIFT + A", "Screen", "Search an area with Google Lens", shell("lens"))
b("ocr", "$MOD + SHIFT + X", "Screen", "Copy text from the screen", shell("ocr"))
b("translate", "$MOD + SHIFT + T", "Screen", "Translate text on the screen", shell("screenTranslate"))
b("ask-screen", "$MOD + ALT + A", "Screen", "Ask AI about part of the screen", shell("askScreen"))
b("color-picker", "$MOD + SHIFT + C", "Screen", "Pick a colour", {"exec": "hyprpicker -a"})
b("record", "$MOD + SHIFT + R", "Screen", "Record an area (again to stop)", shell("record"), locked=True)
b("record-alt", "$MOD + ALT + R", "Screen", "Record an area (alternative)", shell("record"), locked=True)
b("screenshot-print", "Print", "Screen", "Screenshot of the screen", shell("screenshotScreen"), locked=True)
b("screenshot-ctrl-print", "CTRL + Print", "Screen", "Screenshot of the screen (alternative)", shell("screenshotScreen"), locked=True)
b("screen-tools", "$MOD + Print", "Screen", "Screenshot and recording tools", shell("screenTools"))
b("zoom-in", "$MOD + Equal", "Screen", "Zoom in", {"zoom": 0.3}, repeating=True)
b("zoom-out", "$MOD + Minus", "Screen", "Zoom out", {"zoom": -0.3}, repeating=True)
b("zoom-in-pad", "$MOD + code:86", "Screen", "Zoom in (keypad)", {"zoom": 0.3}, repeating=True)
b("zoom-out-pad", "$MOD + code:82", "Screen", "Zoom out (keypad)", {"zoom": -0.3}, repeating=True)

# ── Media ──────────────────────────────────────────────────────────────────
b("media-play", "$MOD + SHIFT + P", "Media", "Play or pause", {"exec": "playerctl play-pause"}, locked=True)
b("media-next", "$MOD + SHIFT + N", "Media", "Next track", {"exec": "playerctl next"}, locked=True)
b("media-prev", "$MOD + SHIFT + B", "Media", "Previous track", {"exec": "playerctl previous"}, locked=True)
b("mute", "$MOD + SHIFT + M", "Media", "Mute sound", shell("volumeMute"), locked=True)
b("mic-mute", "$MOD + ALT + M", "Media", "Mute microphone", shell("micMute"), locked=True)
b("vol-up", "XF86AudioRaiseVolume", "Media", "Volume up", shell("volumeUp"), locked=True, repeating=True)
b("vol-down", "XF86AudioLowerVolume", "Media", "Volume down", shell("volumeDown"), locked=True, repeating=True)
b("vol-mute", "XF86AudioMute", "Media", "Mute (key)", shell("volumeMute"), locked=True)
b("mic-key", "XF86AudioMicMute", "Media", "Mute microphone (key)", shell("micMute"), locked=True)
b("bright-up", "XF86MonBrightnessUp", "Media", "Brightness up", shell("brightnessUp"), locked=True, repeating=True)
b("bright-down", "XF86MonBrightnessDown", "Media", "Brightness down", shell("brightnessDown"), locked=True, repeating=True)
b("play-key", "XF86AudioPlay", "Media", "Play or pause (key)", {"exec": "playerctl play-pause"}, locked=True)
b("pause-key", "XF86AudioPause", "Media", "Pause (key)", {"exec": "playerctl play-pause"}, locked=True)
b("next-key", "XF86AudioNext", "Media", "Next track (key)", {"exec": "playerctl next"}, locked=True)
b("prev-key", "XF86AudioPrev", "Media", "Previous track (key)", {"exec": "playerctl previous"}, locked=True)

# ── Windows ────────────────────────────────────────────────────────────────
b("close", "$MOD + Q", "Window", "Close", dsp("hl.dsp.window.close()"))
b("close-altf4", "ALT + F4", "Window", "Close (alternative)", dsp("hl.dsp.window.close()"))
b("kill", "$MOD + SHIFT + ALT + Q", "Window", "Force quit", {"exec": "hyprctl kill"})
b("fullscreen", "$MOD + F", "Window", "Full screen", dsp('hl.dsp.window.fullscreen({ mode = "fullscreen", action = "toggle" })'))
b("maximize", "$MOD + D", "Window", "Maximise (zoom)", dsp('hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" })'))
b("fullscreen-spoof", "$MOD + ALT + F", "Window", "Full screen inside its tile", dsp('hl.dsp.window.fullscreen_state({ internal = 0, client = 3, action = "toggle" })'))
b("float", "$MOD + ALT + Space", "Window", "Float or tile", dsp('hl.dsp.window.float({ action = "toggle" })'))
b("pin", "$MOD + P", "Window", "Keep on all workspaces", dsp("hl.dsp.window.pin()"))
b("pip", "$MOD + ALT + P", "Window", "Picture in Picture", shell("pip"))
b("tabs", "$MOD + ALT + T", "Window", "Merge into tabs / move tab out", shell("tabs"))
b("tab-next", "$MOD + bracketright", "Window", "Next tab", dsp("hl.dsp.group.next()"))
b("tab-prev", "$MOD + bracketleft", "Window", "Previous tab", dsp("hl.dsp.group.prev()"))
b("split-less", "$MOD + Semicolon", "Window", "Shrink split", dsp('hl.dsp.layout("splitratio -0.1")'), repeating=True)
b("split-more", "$MOD + Apostrophe", "Window", "Grow split", dsp('hl.dsp.layout("splitratio +0.1")'), repeating=True)
b("drag", "$MOD + mouse:272", "Window", "Move with the mouse", dsp("hl.dsp.window.drag()"), mouse=True)
b("drag-middle", "$MOD + mouse:274", "Window", "Move with the mouse (middle button)", dsp("hl.dsp.window.drag()"), mouse=True)
b("resize", "$MOD + mouse:273", "Window", "Resize with the mouse", dsp("hl.dsp.window.resize()"), mouse=True)
for key, d in (("Left", "l"), ("Right", "r"), ("Up", "u"), ("Down", "d")):
    b("focus-" + key.lower(), "$MOD + " + key, "Window", "Focus " + key.lower(), dsp('hl.dsp.focus({ direction = "%s" })' % d))
    b("move-" + key.lower(), "$MOD + SHIFT + " + key, "Window", "Move " + key.lower(), dsp('hl.dsp.window.move({ direction = "%s" })' % d))
for name, label in (("left", "Tile left half"), ("right", "Tile right half")):
    b("snap-" + name, "$MOD + CTRL + ALT + " + name.capitalize(), "Window", label, shell("snap-" + name))
b("snap-top", "$MOD + CTRL + Up", "Window", "Tile top half", shell("snap-top"))
b("snap-bottom", "$MOD + CTRL + Down", "Window", "Tile bottom half", shell("snap-bottom"))
b("snap-fill", "$MOD + CTRL + F", "Window", "Fill the screen", shell("snap-fill"))
b("snap-center", "$MOD + CTRL + C", "Window", "Centre", shell("snap-center"))
b("snap-restore", "$MOD + CTRL + BackSpace", "Window", "Back to the previous size", shell("snap-restore"))

# ── Workspaces ─────────────────────────────────────────────────────────────
for i in range(1, 11):
    k = str(i % 10)
    b("ws-%d" % i, "$MOD + " + k, "Workspace", "Go to workspace %d" % i, dsp("hl.dsp.focus({ workspace = %d })" % i))
    b("ws-send-%d" % i, "$MOD + ALT + " + k, "Workspace", "Send window to workspace %d" % i, dsp("hl.dsp.window.move({ workspace = %d, follow = false })" % i))
    b("ws-move-%d" % i, "$MOD + SHIFT + " + k, "Workspace", "Move window to workspace %d (and follow)" % i, dsp("hl.dsp.window.move({ workspace = %d })" % i))
b("ws-prev", "CTRL + $MOD + Left", "Workspace", "Previous workspace", dsp('hl.dsp.focus({ workspace = "r-1" })'))
b("ws-next", "CTRL + $MOD + Right", "Workspace", "Next workspace", dsp('hl.dsp.focus({ workspace = "r+1" })'))
b("ws-send-prev", "$MOD + SHIFT + Page_Up", "Workspace", "Send window to the previous workspace", dsp('hl.dsp.window.move({ workspace = "r-1" })'))
b("ws-send-next", "$MOD + SHIFT + Page_Down", "Workspace", "Send window to the next workspace", dsp('hl.dsp.window.move({ workspace = "r+1" })'))
b("ws-scroll-down", "$MOD + mouse_down", "Workspace", "Next workspace (scroll)", dsp('hl.dsp.focus({ workspace = "e+1" })'))
b("ws-scroll-up", "$MOD + mouse_up", "Workspace", "Previous workspace (scroll)", dsp('hl.dsp.focus({ workspace = "e-1" })'))
b("scratchpad", "$MOD + S", "Workspace", "Show or hide the scratchpad", dsp('hl.dsp.workspace.toggle_special("special")'))
b("scratchpad-ctrl", "CTRL + $MOD + S", "Workspace", "Show or hide the scratchpad (alternative)", dsp('hl.dsp.workspace.toggle_special("special")'))
b("scratchpad-mouse", "$MOD + mouse:275", "Workspace", "Show or hide the scratchpad (mouse)", dsp('hl.dsp.workspace.toggle_special("special")'))
b("scratchpad-send", "$MOD + ALT + S", "Workspace", "Send window to the scratchpad", dsp('hl.dsp.window.move({ workspace = "special:special", follow = false })'))

# ── Session ────────────────────────────────────────────────────────────────
b("lock", "$MOD + L", "Session", "Lock", shell("lock"))
b("lock-mac", "$MOD + CTRL + Q", "Session", "Lock (macOS key)", shell("lock"))
b("sleep", "$MOD + SHIFT + L", "Session", "Sleep", {"exec": "systemctl suspend || loginctl suspend"}, locked=True)
b("poweroff", "CTRL + SHIFT + ALT + $MOD + Delete", "Session", "Shut down", {"exec": "systemctl poweroff || loginctl poweroff"})

# ── Apps ───────────────────────────────────────────────────────────────────
b("terminal", "$MOD + Return", "Apps", "Terminal", {"app": "terminal"})
b("terminal-t", "$MOD + T", "Apps", "Terminal (alternative)", {"app": "terminal"})
b("terminal-ctrl", "CTRL + ALT + T", "Apps", "Terminal (alternative)", {"app": "terminal"})
b("files", "$MOD + E", "Apps", "Files", {"app": "files"})
b("browser", "$MOD + W", "Apps", "Browser", {"app": "browser"})
b("code", "$MOD + C", "Apps", "Code editor", {"app": "code"})
b("text", "$MOD + X", "Apps", "Text editor", {"app": "text"})
b("office", "CTRL + $MOD + SHIFT + ALT + W", "Apps", "Office", {"app": "office"})
b("mixer", "CTRL + $MOD + V", "Apps", "Volume mixer", {"app": "mixer"})
b("tasks", "CTRL + SHIFT + Escape", "Apps", "Task manager", {"app": "tasks"})

# ── Gestures ───────────────────────────────────────────────────────────────
# end4's set, plus iRice's on finger counts end4 leaves free.
# action: a Hyprland gesture action ("workspace", "move", "fullscreen", ...)
# or {"shell": name}
gestures = [
    {"id": "move", "fingers": 3, "direction": "swipe", "label": "Move the window under the fingers", "action": "move"},
    {"id": "fullscreen", "fingers": 3, "direction": "pinch", "label": "Full screen", "action": "fullscreen"},
    {"id": "workspaces", "fingers": 4, "direction": "horizontal", "label": "Switch workspace", "action": "workspace"},
    {"id": "overview-up", "fingers": 4, "direction": "up", "label": "Overview", "action": {"shell": "overview"}},
    {"id": "overview-down", "fingers": 4, "direction": "down", "label": "Overview", "action": {"shell": "overview"}},
    {"id": "launchpad", "fingers": 4, "direction": "pinchin", "label": "App Library", "action": {"shell": "launchpad"}},
    {"id": "show-desktop", "fingers": 4, "direction": "pinchout", "label": "Show desktop", "action": {"shell": "showDesktop"}},
]

ids = [x["id"] for x in binds]
assert len(ids) == len(set(ids)), "duplicate bind ids"
keys = [x["keys"] for x in binds]
assert len(keys) == len(set(keys)), "two binds on one key: " + str({k for k in keys if keys.count(k) > 1})
for name, data in (("keymap.json", binds), ("gestures.json", gestures)):
    path = os.path.join(ROOT, "hypr", "irice", name)
    with open(path, "w") as f:
        json.dump(data, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print("wrote", path, len(data))
