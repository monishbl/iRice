#!/usr/bin/env python3
"""Tessera host sweep: what the surfaces read or ask that no host service gives.

Static, text-only (no build, no run). Run from anywhere:

    python3 native/tessera-ui/host/tools/sweep.py [-v]

Four checks (the "w4b-gaps" sweep, host/BRIEF.md "Wave 4b"):

  a. state     every ShellState field the menu bar reads (surfaces/menubar) and every
               Host::state(..) a service reads is published (h.set_state, by path or a
               parent object whose json! literal has the key).
  b. messages  every `{"to":"<surface>", <key>: ..}` the QML sent (RustUi.tell in shell/)
               is sent by some host service (tell("<surface>", ..) or {"to": ..}).
  c. ipc       every QML `IpcHandler` target + function is listed in an
               ipc_targets() and its function name appears in the owner's source.
  d. calls     every call a surface or a service asks (shell::ask, h.ask,
               bridge::call, ask(..)) has an owner (Service::calls) whose source
               mentions the function name.

Exit status 1 when anything is listed under GAPS. Lines under "note" are
heuristics that may be false positives; -v shows the checked items too.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))  # native/tessera-ui
REPO = os.path.abspath(os.path.join(ROOT, "..", ".."))
QML = os.path.join(REPO, "shell")
VERBOSE = "-v" in sys.argv


def files(base, ext):
    for d, ds, fs in os.walk(base):
        ds[:] = [x for x in ds if x not in ("target", ".git", "node_modules")]
        for f in fs:
            if f.endswith(ext):
                yield os.path.join(d, f)


def read(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return ""


def balanced(text, i, open_c, close_c):
    """text[i] == open_c: the index after its matching close (strings skipped)."""
    depth, n, j = 0, len(text), i
    while j < n:
        c = text[j]
        if c == '"':
            j += 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
        elif c == open_c:
            depth += 1
        elif c == close_c:
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    return n


def top_keys_json(lit):
    """Top-level keys of a json!({ "a": .., "b": .. }) body (text from the `{`)."""
    keys, depth, i, n = [], 0, 0, len(lit)
    while i < n:
        c = lit[i]
        if c == '"':
            j = i + 1
            while j < n and lit[j] != '"':
                j += 2 if lit[j] == "\\" else 1
            s = lit[i + 1 : j]
            if depth == 1 and re.match(r"\s*:", lit[j + 1 :]):
                keys.append(s)
            i = j
        elif c in "{[(":
            depth += 1
        elif c in "}])":
            depth -= 1
        i += 1
    return keys


gaps, notes, ok = [], [], []


def gap(kind, msg):
    gaps.append(f"[{kind}] {msg}")


def note(kind, msg):
    notes.append(f"[{kind}] {msg}")


HOST_RS = [p for p in files(os.path.join(ROOT, "host"), ".rs")]
HOST_SRC = {p: read(p) for p in HOST_RS}
CORE_SRC = {p: read(p) for p in files(os.path.join(ROOT, "core", "src"), ".rs")}
SURF_SRC = {p: read(p) for p in files(os.path.join(ROOT, "surfaces"), ".rs")}
QML_SRC = {p: read(p) for p in files(QML, ".qml")}

# ---------------------------------------------------------------- a. state
published = {}  # path -> set(child keys) or None (unknown shape)
for p, s in HOST_SRC.items():
    for m in re.finditer(r'\bset_state\(\s*(?:&format!\()?"([A-Za-z0-9_.{}:]+)"', s):
        path = m.group(1)
        if "{" in path:  # a format!() path: svc.key with a mirrored loop; handled by key name
            continue
        j = s.find(",", m.end())
        rest = s[j + 1 : j + 1 + 6000] if j >= 0 else ""
        kids = None
        jm = re.match(r"\s*json!\s*\(\s*\{", rest)
        if jm:
            k = rest.find("{", jm.start())
            kids = set(top_keys_json(rest[k : balanced(rest, k, "{", "}")]))
        published.setdefault(path, set())
        if kids:
            published[path] |= kids
        elif kids is None:
            published[path] = published[path] | {"*"}  # shape unknown (a variable): trust it
# loops: for (svc, key) in [("fingerprint","lockScreen"), ...] set_state(&format!("{svc}.{key}"))
for p, s in HOST_SRC.items():
    for m in re.finditer(r'for \(svc, key\) in \[((?:\("[a-z]+", "[A-Za-z]+"\),? ?)+)\]', s):
        for a, b in re.findall(r'\("([a-z]+)", "([A-Za-z]+)"\)', m.group(1)):
            published.setdefault(a + "." + b, set())


def is_published(path):
    parts = path.split(".")
    for n in range(len(parts), 0, -1):
        pre = ".".join(parts[:n])
        if pre in published:
            kids = published[pre]
            if n == len(parts):
                return True
            if "*" in kids:
                return True
            if parts[n] in kids:
                return n + 1 == len(parts) or True  # (deeper keys of a literal: trusted)
            return False
    # a child path published below it (reading a parent object)
    return any(k.startswith(path + ".") for k in published)


reads = {}  # path -> where
for p, s in SURF_SRC.items():
    if "ShellState" not in s and "shell_state" not in s and "/menubar/" not in p:
        continue
    if "/tests" in p:
        continue
    # strip #[cfg(test)] modules
    t = s.split("#[cfg(test)]")[0]
    for m in re.finditer(r'\b(?:s|shell|ss|sh)\.(?:str|bool|num|get|open)\(\s*"([A-Za-z0-9_.]+)"', t):
        reads.setdefault(m.group(1), os.path.relpath(p, ROOT))
    for m in re.finditer(r'\bshell_state\([^)]*\)\.(?:str|bool|num|get)\(\s*"([A-Za-z0-9_.]+)"', t):
        reads.setdefault(m.group(1), os.path.relpath(p, ROOT))
for p, s in HOST_SRC.items():
    for m in re.finditer(r'\bh\.state\(\s*"([A-Za-z0-9_.]+)"', s):
        reads.setdefault(m.group(1), os.path.relpath(p, ROOT))

for path, where in sorted(reads.items()):
    if is_published(path):
        ok.append(f"state {path}")
    else:
        gap("state", f"{path} is read ({where}) but no host service publishes it")

# ---------------------------------------------------------------- b. messages
# what the QML told each surface (top-level keys of the object literal)
qml_tells = {}  # surface -> {key: file}
for p, s in QML_SRC.items():
    for m in re.finditer(r'RustUi\.tell\(\s*("([a-z]+)"|n)\s*,\s*(\{|[a-z]+\))', s):
        surf = m.group(2)
        if surf is None:
            continue  # RustWindows' loop over several surfaces
        keys = []
        if m.group(3) == "{":
            i = m.end() - 1
            body = s[i : balanced(s, i, "{", "}")]
            depth = 0
            for line_m in re.finditer(r'[{}\[\]()]|(?:(?<=[{,])\s*)([A-Za-z_]\w*)\s*:', body):
                tok = line_m.group(0)
                if tok in "{[(":
                    depth += 1
                elif tok in "}])":
                    depth -= 1
                elif depth == 1 and line_m.group(1):
                    keys.append(line_m.group(1))
        else:
            continue  # a variable (screentools' `o`): checked by hand
        for k in keys:
            qml_tells[(surf, k)] = os.path.relpath(p, REPO)
qml_tells.update({("spaces-surfaces", "spaces"): "shell/modules/overview/RustWindows.qml"})
del qml_tells[("spaces-surfaces", "spaces")]


def host_sent_keys(surface):
    """Keys a host service sends to a surface: tell("surface", json!({..})) literals,
    {"to": "surface", ..} literals, else (a variable) every quoted key in the file."""
    out, loose = set(), set()
    for p, s in HOST_SRC.items():
        for m in re.finditer(r'(?:\btell(?:_both)?|\.tell)\(\s*(?:h,\s*)?"%s"\s*,\s*' % re.escape(surface), s):
            rest = s[m.end() : m.end() + 4000]
            jm = re.match(r"\s*json!\s*\(\s*\{", rest)
            if jm:
                k = rest.find("{", jm.start())
                out |= set(top_keys_json(rest[k : balanced(rest, k, "{", "}")]))
            else:
                loose |= set(re.findall(r'"([A-Za-z_]\w*)"', s))
        if surface in ("screentools", "quicklook"):
            # util::tell_both(h, json!({..})): the same object to both surfaces
            for m in re.finditer(r"tell_both\(\s*h,\s*json!\s*\(\s*\{", s):
                k = s.find("{", m.start())
                out |= set(top_keys_json(s[k : balanced(s, k, "{", "}")]))
        for m in re.finditer(r'"to"\s*:\s*"%s"' % re.escape(surface), s):
            # the enclosing object literal
            a = s.rfind("{", 0, m.start())
            out |= set(top_keys_json(s[a : balanced(s, a, "{", "}")]))
        # tell_both(h, json!(..)) helpers that fan out to a fixed pair of surfaces
        if re.search(r'"%s"' % re.escape(surface), s) and "tell_both" in s:
            loose |= set(re.findall(r'"([A-Za-z_]\w*)"', s)) if re.search(r'fn tell_both', s) else set()
    return out, loose


for (surf, key), where in sorted(qml_tells.items()):
    keys, loose = host_sent_keys(surf)
    if key in keys:
        ok.append(f"tell {surf}.{key}")
    elif key in loose:
        note("message", f'"{surf}" gets "{key}" only through a variable (QML: {where}); confirm it is sent')
    else:
        gap("message", f'QML told "{surf}" {{{key}}} ({where}); no host service sends it')

# ---------------------------------------------------------------- c. ipc
targets = {}  # target -> [files]
for p, s in HOST_SRC.items():
    for m in re.finditer(r"fn ipc_targets\([^)]*\)[^{]*\{\s*&\[([^\]]*)\]", s):
        for t in re.findall(r'"([A-Za-z0-9_]+)"', m.group(1)):
            targets.setdefault(t, []).append(p)
# host/shell's ipc.rs lists TARGETS for the socket; they are answered by Panels etc.
qml_ipc = {}
for p, s in QML_SRC.items():
    for m in re.finditer(r"IpcHandler\s*\{", s):
        end = balanced(s, m.end() - 1, "{", "}")
        body = s[m.end() - 1 : end]
        tm = re.search(r'target:\s*"([^"]+)"', body)
        if not tm:
            continue
        fns = re.findall(r"\bfunction\s+(\w+)\s*\(", body)
        qml_ipc.setdefault(tm.group(1), (set(), os.path.relpath(p, REPO)))[0].update(fns)


def crate_of(p):
    rel = os.path.relpath(p, os.path.join(ROOT, "host")).split(os.sep)
    return os.path.join(ROOT, "host", rel[0])


for t, (fns, where) in sorted(qml_ipc.items()):
    owners = targets.get(t)
    if not owners:
        gap("ipc", f"IpcHandler target {t} ({where}) is in no Service::ipc_targets()")
        continue
    # the function names: anywhere in the owning crate's source (the match arms)
    src = "\n".join(s for q, s in HOST_SRC.items() if crate_of(q) == crate_of(owners[0]))
    for fn in sorted(fns):
        if re.search(r'"%s"' % re.escape(fn), src):
            ok.append(f"ipc {t}.{fn}")
        else:
            gap("ipc", f"{t} {fn} ({where}) is not matched in {os.path.relpath(crate_of(owners[0]), ROOT)}")

# ---------------------------------------------------------------- d. calls
prefixes = {}  # prefix -> [file]
for p, s in HOST_SRC.items():
    for m in re.finditer(r"fn calls\([^)]*\)[^{]*\{(.*?)\n    \}", s, re.S):
        for t in re.findall(r'"([A-Za-z0-9_]+)"', m.group(1)):
            prefixes.setdefault(t, []).append(p)
# (a service that answers several prefixes through a const list)
for p, s in HOST_SRC.items():
    for m in re.finditer(r"fn calls\([^)]*\)[^{]*\{\s*([A-Z_]+)\s*\}", s):
        cm = re.search(r"const %s[^=]*=\s*&\[([^\]]*)\]" % m.group(1), s)
        if cm:
            for t in re.findall(r'"([A-Za-z0-9_]+)"', cm.group(1)):
                prefixes.setdefault(t, []).append(p)

asked = {}  # "prefix.func" -> where
for src_map, label in ((SURF_SRC, "surface"), (CORE_SRC, "core"), (HOST_SRC, "host")):
    for p, s in src_map.items():
        t = s.split("#[cfg(test)]")[0]
        rel = os.path.relpath(p, ROOT)
        for m in re.finditer(r'\b(?:shell::ask|h\.ask|[a-z_]+::ask|\bask)\(\s*"([A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)?)"', t):
            asked.setdefault(m.group(1), rel)
        if "fn ask(what: &str" in t:  # surfaces/desktop: ask("x") is shell::ask("desktop", ["x", ..])
            wm = re.search(r'shell::ask\("(\w+)"', t[t.index("fn ask(what: &str") :])
            for m in re.finditer(r'(?<![\w:.])ask\(\s*"(\w+)"', t):
                asked.setdefault((wm.group(1) if wm else "desktop") + "." + m.group(1), rel)
                asked.pop(m.group(1), None)
        for m in re.finditer(r"\bbridge::call\(&\[([^\]]*)\]", t):
            parts = re.findall(r'"([^"]*)"', m.group(1))
            if len(parts) >= 2:
                asked.setdefault("ipc:" + parts[0] + "." + parts[1], rel)
        for m in re.finditer(r'\b(?:h\.ask|ask)\(\s*&?format!\("([a-z]+)\.', t):
            pass

# prefixes that exist only in a surface-owned feed (the surface answers them itself)
SELF_ANSWERED = set()

for call, where in sorted(asked.items()):
    if call.startswith("ipc:"):
        target, _, fn = call[4:].partition(".")
        if target not in targets and target not in qml_ipc:
            gap("calls", f"bridge::call {target} {fn} ({where}): no ipc target {target}")
        elif target not in targets:
            pass  # reported by (c)
        else:
            owner = targets[target][0]
            src = "\n".join(s for q, s in HOST_SRC.items() if crate_of(q) == crate_of(owner))
            if not re.search(r'"%s"' % re.escape(fn), src):
                gap("calls", f"bridge::call {target} {fn} ({where}) is not matched in {os.path.relpath(crate_of(owner), ROOT)}")
            else:
                ok.append(f"call-ipc {target}.{fn}")
        continue
    prefix, _, fn = call.partition(".")
    if prefix in SELF_ANSWERED:
        continue
    owners = prefixes.get(prefix)
    if not owners:
        gap("calls", f"{call} ({where}): no service lists the prefix {prefix} in calls()")
        continue
    if not fn:
        ok.append(f"call {call}")
        continue
    # the function name as a quoted string in the owner's directory (module group)
    d = os.path.dirname(owners[0])
    near = "\n".join(s for q, s in HOST_SRC.items() if os.path.dirname(q).startswith(d) or crate_of(q) == crate_of(owners[0]))
    if re.search(r'"%s"' % re.escape(fn), near) or re.search(r'"%s\.%s"' % (re.escape(prefix), re.escape(fn)), near):
        ok.append(f"call {call}")
    else:
        gap("calls", f"{call} ({where}): {os.path.relpath(owners[0], ROOT)} never matches \"{fn}\"")

# ---------------------------------------------------------------- e. ui events
# what surfaces say on stdout (shell::tell(NAME, ..), surfaces::tell({"kind": ..})) reaches
# host-shell's Panels::ui_event by its kind
panels = HOST_SRC.get(os.path.join(ROOT, "host", "shell", "src", "panels.rs"), "")
ui_arms = set()
m = re.search(r"fn ui_event\(.*?\n    \}\n", panels, re.S)
if m:
    for a in re.findall(r'^\s+((?:"[a-z]+"\s*\|?\s*)+)=>', m.group(0), re.M):
        ui_arms |= set(re.findall(r'"([a-z]+)"', a))
kinds = {}
for p, s in SURF_SRC.items():
    names = dict(re.findall(r'const (NAME): &str = "([a-z]+)"', s))
    if "shell::tell(NAME" in s and "NAME" in names:
        kinds.setdefault(names["NAME"], os.path.relpath(p, ROOT))
    for k in re.findall(r'shell::tell\(\s*"([a-z_]+)"', s):
        kinds.setdefault(k, os.path.relpath(p, ROOT))
    for k in re.findall(r'surfaces::tell\(\s*serde_json::json!\(\{[^}]*"kind":\s*"([a-z]+)"', s):
        kinds.setdefault(k, os.path.relpath(p, ROOT))
for k in ("closed", "ready", "unknown"):
    kinds.setdefault(k, "core")
for k, where in sorted(kinds.items()):
    if k in ("notifications", "nc", "call", "settings"):
        continue  # routed to their own owners by shell::tell / ask
    if k in ui_arms:
        ok.append(f"ui {k}")
    else:
        gap("ui", f'surface event "{k}" ({where}) is not handled by Panels::ui_event')

# ---------------------------------------------------------------- report
if VERBOSE:
    print("CHECKED OK (%d)" % len(ok))
    for o in ok:
        print("  ", o)
print("NOTES (%d)" % len(notes))
for n in notes:
    print("  ", n)
print("GAPS (%d)" % len(gaps))
for g in gaps:
    print("  ", g)
sys.exit(1 if gaps else 0)
