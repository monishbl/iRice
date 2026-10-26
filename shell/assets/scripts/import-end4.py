#!/usr/bin/env python3
"""Reads end4 (illogical-impulse) keybinds and gestures and prints them as
iRice binds, for Settings → Keyboard Shortcuts → Import from end4.

  python3 -I import-end4.py KEYBINDS.lua [MORE.lua ...] -- GENERAL.lua [MORE.lua ...]

Files before "--" are read for hl.bind(...), files after it for
hl.gesture({...}). Missing files are skipped. Read-only: nothing is run or
written. Prints one JSON object:

  { "binds":    [ { keys, group, label, action, opts?, source } ],
    "skipped":  [ { keys, reason, source, fallback? } ],
    "gestures": [ { fingers, direction, label, action } ],
    "gesturesSkipped": [ { fingers, direction, reason } ],
    "ignored":  N,            binds inside for loops (the defaults cover them)
    "files":    [ paths read ], "missing": [ paths not found ] }

Actions use iRice's format (hypr/irice/keymap.json): {shell}, {app}, {exec},
{dsp} or {zoom}. "fallback" marks a skipped bind that only runs when end4's
shell isn't running, on keys that another bind already imports.
"""
import json
import os
import re
import sys

# end4's quickshell:NAME globals → iRice shell actions
SHELL = {
    "searchToggleRelease": "spotlight",
    "searchToggle": "spotlight",
    "overviewWorkspacesToggle": "overview",
    "overviewToggle": "overview",
    "overviewClipboardToggle": "clipboard",
    "overviewEmojiToggle": "emoji",
    "sidebarLeftToggle": "sidebar",
    "sidebarRightToggle": "notificationCenter",
    "cheatsheetToggle": "cheatsheet",
    "oskToggle": "keyboard",
    "overlayToggle": "overlay",
    "sessionToggle": "session",
    "wallpaperSelectorToggle": "wallpaper",
    "toggleLightDark": "darkMode",
    "regionScreenshot": "screenshotRegion",
    "regionSearch": "lens",
    "regionOcr": "ocr",
    "screenTranslate": "screenTranslate",
    "regionRecord": "record",
    "lock": "lock",
}

# end4's app variables (hyprland/variables.lua) → iRice default-app roles
ROLES = {
    "terminal": "terminal",
    "fileManager": "files",
    "browser": "browser",
    "codeEditor": "code",
    "textEditor": "text",
    "volumeMixer": "mixer",
    "taskManager": "tasks",
    "officeSoftware": "office",
}

# Variables and paths that only exist in end4: commands using them can't work
END4_NAMES = {"qsScripts", "hyprScripts", "qsIsAlive", "qsIpcCall"}
END4_TEXT = ("$qsConfig", "${qsConfig}", "/.config/hypr/hyprland/scripts", "/.config/hypr/custom/", "/.config/quickshell/")

# end4 commands iRice does itself: same keys, iRice's action (checked in order)
EQUIVALENTS = [
    (re.compile(r"brightness increment"), {"shell": "brightnessUp"}),
    (re.compile(r"brightness decrement"), {"shell": "brightnessDown"}),
    (re.compile(r"^loginctl lock-session\b"), {"shell": "lock"}),
    (re.compile(r"set-volume @DEFAULT_(AUDIO_)?SINK@ [\d.]+%\+"), {"shell": "volumeUp"}),
    (re.compile(r"set-volume @DEFAULT_(AUDIO_)?SINK@ [\d.]+%-"), {"shell": "volumeDown"}),
    (re.compile(r"set-mute @DEFAULT_(AUDIO_)?SINK@ toggle"), {"shell": "volumeMute"}),
    (re.compile(r"set-mute @DEFAULT_(AUDIO_)?SOURCE@ toggle"), {"shell": "micMute"}),
    (re.compile(r"^playerctl next\b"), {"exec": "playerctl next"}),
    (re.compile(r"^grim\b(?!.*slurp)"), {"shell": "screenshotScreen"}),
]

OPTS = ("repeating", "locked", "release", "mouse")


# ── A small Lua scanner ────────────────────────────────────────────────────
class Src:
    def __init__(self, text):
        self.t = text
        self.n = len(text)

    def skip_string(self, i):
        """i at a quote or [[; returns (end index after it, value) or None."""
        t = self.t
        if t[i] in "\"'":
            q = t[i]
            j = i + 1
            out = []
            while j < self.n and t[j] != q:
                if t[j] == "\\" and j + 1 < self.n:
                    c = t[j + 1]
                    out.append({"n": "\n", "t": "\t", "\\": "\\", "\"": "\"", "'": "'"}.get(c, "\\" + c))
                    j += 2
                    continue
                if t[j] == "\n":
                    break
                out.append(t[j])
                j += 1
            return j + 1, "".join(out)
        m = re.match(r"\[(=*)\[", t[i:])
        if m:
            close = "]" + m.group(1) + "]"
            j = t.find(close, i + len(m.group(0)))
            if j < 0:
                j = self.n
            return j + len(close), t[i + len(m.group(0)):j]
        return None

    def skip_comment(self, i):
        """i at --; returns the index after the comment."""
        t = self.t
        m = re.match(r"--\[(=*)\[", t[i:])
        if m:
            close = "]" + m.group(1) + "]"
            j = t.find(close, i)
            return self.n if j < 0 else j + len(close)
        j = t.find("\n", i)
        return self.n if j < 0 else j

    def balanced(self, i):
        """i at an opening ( or {; returns the index after its closing bracket."""
        depth = 0
        t = self.t
        while i < self.n:
            c = t[i]
            if c in "\"'" or (c == "[" and re.match(r"\[=*\[", t[i:])):
                i = self.skip_string(i)[0]
                continue
            if t.startswith("--", i):
                i = self.skip_comment(i)
                continue
            if c in "({[":
                depth += 1
            elif c in ")}]":
                depth -= 1
                if depth == 0:
                    return i + 1
            i += 1
        return self.n


def split_args(text):
    """Top-level comma split of an argument list (without the brackets)."""
    s = Src(text)
    args, depth, start, i = [], 0, 0, 0
    while i < s.n:
        c = text[i]
        if c in "\"'" or (c == "[" and re.match(r"\[=*\[", text[i:])):
            i = s.skip_string(i)[0]
            continue
        if text.startswith("--", i):
            i = s.skip_comment(i)
            continue
        if c in "({[":
            depth += 1
        elif c in ")}]":
            depth -= 1
        elif c == "," and depth == 0:
            args.append(text[start:i].strip())
            start = i + 1
        i += 1
    if text[start:].strip():
        args.append(text[start:].strip())
    return args


def strip_comments(text):
    s = Src(text)
    out, i, last = [], 0, 0
    while i < s.n:
        c = text[i]
        if c in "\"'" or (c == "[" and re.match(r"\[=*\[", text[i:])):
            i = s.skip_string(i)[0]
            continue
        if text.startswith("--", i):
            out.append(text[last:i])
            i = s.skip_comment(i)
            last = i
            continue
        i += 1
    out.append(text[last:])
    return "".join(out)


class Value:
    """A Lua string expression: text, or why it can't be used."""

    def __init__(self, text=None, problem=None):
        self.text = text
        self.problem = problem


def evaluate(expr, env, lenient=False):
    """Evaluates "a" .. b .. "c" with string literals and known locals.
    lenient: end4's own variables read as empty (to recognise commands)."""
    expr = strip_comments(expr).strip()
    if expr.startswith("(") and Src(expr).balanced(0) == len(expr):
        expr = expr[1:-1].strip()
    parts, s, i, start, depth = [], Src(expr), 0, 0, 0
    while i < s.n:
        c = expr[i]
        if c in "\"'" or (c == "[" and re.match(r"\[=*\[", expr[i:])):
            i = s.skip_string(i)[0]
            continue
        if c in "({[":
            depth += 1
        elif c in ")}]":
            depth -= 1
        elif depth == 0 and expr.startswith("..", i):
            parts.append(expr[start:i].strip())
            i += 2
            start = i
            continue
        i += 1
    parts.append(expr[start:].strip())
    out = []
    for p in parts:
        if not p:
            return Value(problem="an empty expression")
        if p[0] in "\"'" or p.startswith("["):
            r = s.__class__(p).skip_string(0)
            if not r or r[0] != len(p):
                return Value(problem="an expression it can't read")
            out.append(r[1])
        elif re.fullmatch(r"[A-Za-z_]\w*", p):
            if p in END4_NAMES:
                if lenient:
                    continue
                return Value(problem="uses end4's " + p)
            v = env.get(p)
            if v is None:
                return Value(problem="uses the variable " + p)
            if v.problem:
                return v
            out.append(v.text)
        else:
            return Value(problem="builds the command with code")
    text = "".join(out)
    for marker in END4_TEXT if not lenient else ():
        if marker in text:
            return Value(problem="opens end4's own config" if marker == "/.config/hypr/custom/" else "runs an end4 script")
    return Value(text=text)


def parse_table(text):
    """Flat { key = value, ... } → dict of raw value texts."""
    text = text.strip()
    if text.startswith("{"):
        text = text[1:-1]
    out = {}
    for a in split_args(text):
        m = re.match(r"([A-Za-z_]\w*)\s*=\s*(.*)$", a, re.S)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


def lua_string(v):
    if v and v[0] in "\"'":
        r = Src(v).skip_string(0)
        if r:
            return r[1]
    return None


def norm_keys(k):
    parts = [p.strip() for p in k.split("+") if p.strip()]
    out = []
    for p in parts:
        u = p.upper()
        if u == "SUPER":
            out.append("$MOD")
        elif u in ("SUPER_L",):
            out.append("$MODKEY")
        elif u in ("CTRL", "CONTROL"):
            out.append("CTRL")
        elif u in ("ALT", "SHIFT"):
            out.append(u)
        else:
            out.append(p)
    return " + ".join(out)


def action_of(expr, env):
    """→ (action, None) or (None, reason)."""
    e = " ".join(strip_comments(expr).split())
    m = re.fullmatch(r"hl\.dsp\.global\(\s*(\"[^\"]*\"|'[^']*')\s*\)", e)
    if m:
        name = m.group(1)[1:-1]
        if name.startswith("quickshell:"):
            short = name.split(":", 1)[1]
            if short in SHELL:
                return {"shell": SHELL[short]}, None
            return None, "end4 shell action " + short + " has no iRice match"
        if name.startswith("irice:"):
            return {"shell": name.split(":", 1)[1]}, None
        return {"dsp": e}, None
    m = re.fullmatch(r"hl\.dsp\.exec_cmd\((.*)\)", e, re.S)
    if m:
        arg = m.group(1).strip()
        if arg in ROLES:
            return {"app": ROLES[arg]}, None
        if arg == "settingsApp":
            return {"shell": "settings"}, None
        # Known end4 commands first: some use end4 variables but iRice does the same thing
        raw = evaluate(arg, env, lenient=True)
        if raw.text is not None:
            for rx, act in EQUIVALENTS:
                if rx.search(raw.text.strip()):
                    return dict(act), None
        v = evaluate(arg, env)
        if v.problem:
            return None, "command " + v.problem
        return {"exec": v.text.strip()}, None
    m = re.fullmatch(r"function\s*\(\s*\)\s*zoomfunction\(\s*([-+]?[\d.]+)\s*\)\s*end", e)
    if m:
        return {"zoom": float(m.group(1))}, None
    if e.startswith("function"):
        return None, "runs Lua code"
    if re.fullmatch(r"hl\.dsp\.[\w.]+\(.*\)", e, re.S):
        return {"dsp": e}, None
    return None, "action it can't read"


def scan(path, kind, out):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            text = f.read()
    except OSError:
        out["missing"].append(path)
        return
    out["files"].append(path)
    s = Src(text)
    env = {}
    stack = []  # "loop" | "block" | "function"
    i = 0
    word = re.compile(r"[A-Za-z_]\w*")
    prev = ""
    while i < s.n:
        c = text[i]
        if c in "\"'" or (c == "[" and re.match(r"\[=*\[", text[i:])):
            i = s.skip_string(i)[0]
            continue
        if text.startswith("--", i):
            i = s.skip_comment(i)
            continue
        m = word.match(text, i)
        if m and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] in "_.")):
            w = m.group(0)
            j = m.end()
            if w in ("for", "while"):
                stack.append("loop")
                # their "do" belongs to them
                d = re.compile(r"\bdo\b").search(text, j)
                i = d.end() if d else s.n
                prev = "do"
                continue
            if w in ("if", "function", "repeat"):
                stack.append("function" if w == "function" else "block")
            elif w == "do":
                stack.append("block")
            elif w in ("end", "until"):
                if stack:
                    stack.pop()
            elif w == "local" and not stack:
                lm = re.compile(r"local\s+([A-Za-z_]\w*)\s*=\s*").match(text, i)
                if lm and not text.startswith("function", lm.end()):
                    # the expression runs to the end of the line, plus continuation lines
                    k = lm.end()
                    end = k
                    while True:
                        nl = text.find("\n", end)
                        nl = s.n if nl < 0 else nl
                        seg = strip_comments(text[k:nl]).rstrip()
                        if seg.endswith("..") or text[nl + 1:].lstrip().startswith(".."):
                            end = nl + 1
                            if end >= s.n:
                                break
                            continue
                        end = nl
                        break
                    expr = text[k:end]
                    if not re.match(r"\s*(\"|'|\[|[A-Za-z_])", expr) or re.match(r"\s*(require|function|hl\.|\{)", expr):
                        pass
                    else:
                        env[lm.group(1)] = evaluate(expr, env)
                    i = end
                    continue
            if w == "hl" and kind == "binds" and text.startswith("hl.bind(", i):
                start = i + len("hl.bind")
                stop = s.balanced(start)
                call = text[start + 1:stop - 1]
                eol = text.find("\n", stop)
                hidden = "[hidden]" in text[stop:eol if eol >= 0 else s.n]
                handle_bind(call, stack, env, path, out, hidden)
                i = stop
                continue
            if w == "hl" and kind == "gestures" and text.startswith("hl.gesture(", i):
                start = i + len("hl.gesture")
                stop = s.balanced(start)
                handle_gesture(text[start + 1:stop - 1], stack, out)
                i = stop
                continue
            i = j
            prev = w
            continue
        i += 1


def handle_bind(call, stack, env, path, out, hidden=False):
    args = split_args(call)
    if len(args) < 2:
        return
    keys = lua_string(args[0])
    if keys is None:
        if "loop" in stack:
            out["ignored"] += 1
        else:
            out["skipped"].append({"keys": args[0], "reason": "keys built with code", "source": path})
        return
    if "loop" in stack:
        out["ignored"] += 1
        return
    keys = norm_keys(keys)
    if "function" in stack:
        out["skipped"].append({"keys": keys, "reason": "only in a submap", "source": path})
        return
    opts = parse_table(args[2]) if len(args) > 2 and args[2].startswith("{") else {}
    desc = lua_string(opts.get("description", "")) or ""
    if hidden and not desc:
        out["skipped"].append({"keys": keys, "reason": "end4 hides it (a test bind)", "source": path})
        return
    action, reason = action_of(args[1], env)
    if action is None:
        out["skipped"].append({"keys": keys, "label": desc, "reason": reason, "source": path})
        return
    if desc and ": " in desc:
        group, label = desc.split(": ", 1)
    else:
        group, label = "", desc
    e = {"keys": keys, "group": group, "label": label, "action": action, "source": path}
    o = {k: True for k in OPTS if opts.get(k) == "true"}
    if o:
        e["opts"] = o
    out["binds"].append(e)


def handle_gesture(call, stack, out):
    t = parse_table(call)
    try:
        fingers = int(t.get("fingers", ""))
    except ValueError:
        fingers = 0
    direction = lua_string(t.get("direction", "")) or ""
    a = t.get("action", "")
    action, reason = None, None
    if a.startswith("\"") or a.startswith("'"):
        action = lua_string(a)
    else:
        m = re.search(r"hl\.dsp\.global\(\s*[\"']quickshell:(\w+)[\"']\s*\)", a)
        if m and m.group(1) in SHELL:
            action = {"shell": SHELL[m.group(1)]}
        elif m:
            reason = "end4 shell action " + m.group(1) + " has no iRice match"
        else:
            reason = "runs Lua code"
    if "loop" in stack or not fingers or not direction or action is None:
        out["gesturesSkipped"].append({"fingers": fingers, "direction": direction, "reason": reason or "can't read it"})
        return
    out["gestures"].append({"fingers": fingers, "direction": direction, "label": "", "action": action})


def main(argv):
    if "--" in argv:
        k = argv.index("--")
        bind_files, gesture_files = argv[:k], argv[k + 1:]
    else:
        bind_files, gesture_files = argv, []
    out = {"binds": [], "skipped": [], "gestures": [], "gesturesSkipped": [], "ignored": 0, "files": [], "missing": []}
    for p in bind_files:
        scan(os.path.expanduser(p), "binds", out)
    for p in gesture_files:
        scan(os.path.expanduser(p), "gestures", out)
    # Two binds on the same keys (end4 runs both): keep the one iRice does
    # itself (a shell action), else the first
    first = {}
    for b in out["binds"]:
        k = re.sub(r"\s", "", b["keys"]).upper()
        if k not in first or ("shell" in b["action"] and "shell" not in first[k]["action"]):
            first[k] = b
    keep = []
    for b in out["binds"]:
        if first[re.sub(r"\s", "", b["keys"]).upper()] is b:
            keep.append(b)
        else:
            out["skipped"].append({"keys": b["keys"], "label": b["label"], "reason": "a second action on the same keys", "source": b["source"], "fallback": True})
    out["binds"] = keep
    # A skipped bind on keys another bind imports is end4's fallback for
    # when its shell isn't running (or a second action on the same keys)
    taken = {re.sub(r"\s", "", b["keys"]).upper() for b in out["binds"]}
    for s in out["skipped"]:
        if re.sub(r"\s", "", s["keys"]).upper() in taken:
            s["fallback"] = True
    json.dump(out, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main(sys.argv[1:])
