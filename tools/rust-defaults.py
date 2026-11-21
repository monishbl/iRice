#!/usr/bin/env python3
"""The settings defaults the Rust shell reads (native/tessera-ui), as
tools/rust-defaults.json: {"section.key": default}. The updater's conflict
check (tools/tessera-update.py check) compares this file between the
installed version and the new one, as it does Config.qml, so a default that
changes only in the Rust shell is a question too (Config.qml is frozen with
the QML fallback).

    tools/rust-defaults.py           print what the Rust sources read now
    tools/rust-defaults.py --write   write it to tools/rust-defaults.json
    tools/rust-defaults.py --check   exit 1 when the file is behind the sources

tests/test_rust_defaults.py fails when the file drifts from the sources.
The scan reads the readers written the usual ways:
  cfg["a"]["b"].as_bool().unwrap_or(X)        (and cfg(), c[..], a[..])
  cfg_num(cfg, "a.b", X), cfg_bool(..), cfg_str(..)
  let a = &cfg["appearance"]; ... a["glass"].as_str().unwrap_or(X)
A reader that isn't asking for the default (a one-time migration looking
for an old value) is in NOT_DEFAULTS, with why.
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST = os.path.join(ROOT, "native", "tessera-ui")
OUT = os.path.join(ROOT, "tools", "rust-defaults.json")

# (file, setting): readers that compare with something other than the default
NOT_DEFAULTS = {
    # (Config.qml's onLoaded migrations: looking for old values)
    ("host/shell/src/common.rs", "appearance.menuIcon"),
    ("host/shell/src/common.rs", "screenshot.editor"),
    ("host/shell/src/common.rs", "workspaces.dimOthers"),
    ("host/shell/src/common.rs", "appearance.iconStyle"),
    ("host/shell/src/common.rs", "appearance.cursor"),
    # ("" reads as the dynamic face, Config.qml's default, in material_clock)
    ("surfaces/greeter/src/view.rs", "lock.clockFace"),
    # ("" reads as metric, Config.qml's default, in weather::settings)
    ("host/desktop/src/today/weather.rs", "weather.units"),
    # (only tells solid and custom from the rest: "auto" and "frosted" read the same there)
    ("surfaces/notifications/src/panel.rs", "appearance.glass"),
}

VALUE = r'("(?:[^"\\]|\\.)*"|-?[0-9.]+|true|false)'
INDEXED = re.compile(r'(?:cfg\(\)|cfg|\bc|\ba)((?:\["[A-Za-z0-9_]+"\]){2,3})\s*\.as_(?:bool|f64|i64|u64|str)\(\)\s*'
                     r'\.unwrap_or\(\s*' + VALUE + r'\s*\)')
HELPER = re.compile(r'cfg_(?:num|bool|str)\(\s*[a-z_&.()]+,\s*"([A-Za-z0-9_.]+)",\s*' + VALUE + r'\s*\)')
# let a = &cfg["appearance"];  (a section, read through a name)
ALIAS = re.compile(r'let\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*(?::\s*&\S+\s*)?=\s*&\s*(?:h\.|self\.|st\.|host\.)?'
                   r'(?:cfg\(\)|cfg|config|c)\["([A-Za-z0-9_]+)"\]\s*;')
THROUGH = re.compile(r'\b([a-z_][a-z0-9_]*)((?:\["[A-Za-z0-9_]+"\]){1,2})\s*\.as_(?:bool|f64|i64|u64|str)\(\)\s*'
                     r'\.unwrap_or\(\s*' + VALUE + r'\s*\)')


def literal(v):
    if v in ("true", "false"):
        return v == "true"
    if v.startswith('"'):
        return json.loads(v)
    f = float(v)
    return int(f) if f.is_integer() else f


def sources(rust=RUST):
    for d, dirs, names in os.walk(rust):
        dirs[:] = sorted(x for x in dirs if x not in ("target", "vendor"))
        for n in sorted(names):
            if n.endswith(".rs"):
                p = os.path.join(d, n)
                with open(p, errors="replace") as f:
                    yield os.path.relpath(p, rust), f.read()


def sections(rust=RUST):
    """The top-level sections settings live in: those of Config.qml's
    adapter, as the shell's config.json has them"""
    try:
        with open(os.path.join(ROOT, "shell", "common", "Config.qml")) as f:
            text = f.read()
    except OSError:
        return set()
    return set(re.findall(r"^\s*property\s+JsonObject\s+(\w+)\s*:\s*JsonObject\s*\{", text, re.M))


def readers(rust=RUST, secs=None):
    """[(file, line, "section.key", default)] for every reader found"""
    secs = sections(rust) if secs is None else secs
    out = []
    for rel, text in sources(rust):
        found = []
        for m in INDEXED.finditer(text):
            found.append((m.start(), ".".join(re.findall(r'"([^"]+)"', m.group(1))), m.group(2)))
        for m in HELPER.finditer(text):
            found.append((m.start(), m.group(1), m.group(2)))
        named = [(m.start(), m.group(1), m.group(2)) for m in ALIAS.finditer(text)]
        for m in THROUGH.finditer(text):
            bound = [a for a in named if a[0] < m.start() and a[1] == m.group(1)]
            if bound:
                found.append((m.start(), bound[-1][2] + "." + ".".join(re.findall(r'"([^"]+)"', m.group(2))), m.group(3)))
        for at, key, value in found:
            if key.split(".")[0] not in secs or (rel, key) in NOT_DEFAULTS:
                continue
            out.append((rel, text[:at].count("\n") + 1, key, literal(value)))
    return out


def same(a, b):
    if isinstance(a, bool) or isinstance(b, bool):
        return a is b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) < 1e-9
    return a == b


def scan(rust=RUST):
    """({"section.key": default}, ["file:line key: X, elsewhere Y", ...]):
    the defaults, and the readers of one setting that disagree"""
    found, where, clashes = {}, {}, []
    for rel, line, key, value in readers(rust):
        if key in found and not same(found[key], value):
            clashes.append(f"{rel}:{line} {key}: {value!r}, {where[key]} {found[key]!r}")
            continue
        found.setdefault(key, value)
        where.setdefault(key, f"{rel}:{line}")
    return dict(sorted(found.items())), clashes


def text_of(defaults):
    return json.dumps(defaults, indent=1, sort_keys=True, ensure_ascii=False) + "\n"


def main(argv):
    defaults, clashes = scan()
    for c in clashes:
        print("rust-defaults: two defaults for one setting: " + c, file=sys.stderr)
    if "--write" in argv:
        with open(OUT, "w") as f:
            f.write(text_of(defaults))
        return 1 if clashes else 0
    if "--check" in argv:
        try:
            with open(OUT) as f:
                ok = f.read() == text_of(defaults)
        except OSError:
            ok = False
        if not ok:
            print("tools/rust-defaults.json is behind the Rust sources: tools/rust-defaults.py --write", file=sys.stderr)
        return 0 if ok and not clashes else 1
    sys.stdout.write(text_of(defaults))
    return 1 if clashes else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
