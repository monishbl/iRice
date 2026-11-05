#!/usr/bin/env python3
"""Moves a surface written for the one-crate layout (main b19b699 and before:
src/surfaces/<name>/ + ui/<name>.slint) into its own crate in the workspace
(surfaces/<name>/), and registers it. For branches started before the split:
docs/rust-ui-surfaces.md, "Moving a branch from before the split".

  tools/adopt-surface.py NAME [--part TYPE] [--slint FILE.slint ...] [--export 'LINE' ...]
  tools/adopt-surface.py --tokens   only the theme tokens (below)

NAME     the surface (its folder under src/surfaces/, its name in KNOWN)
--part   the part's type (`Lock` for `lock::Lock::default()`); default: read
         from the branch's old make() line
--slint  its .slint files (default: NAME.slint), found in ui/ or core/ui/
--export its export lines for ui/main.slint; default: the old ui/main.slint's
         lines that name its .slint files

It moves the files (mod.rs becomes lib.rs), writes Cargo.toml, build.rs and
ui/main.slint, puts the crate header in lib.rs, points the Slint imports at the
shared `@tessera` library, rewrites the paths to helpers that moved into
tessera-core, adds the surface to Cargo.toml and src/surfaces.rs, and adds
theme tokens the merged theme.rs sets to the token list. Runs no
git command that changes anything (it reads ORIG_HEAD when it has to); check
`git status` and build afterwards.
"""
import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

WS = Path(__file__).resolve().parents[1]  # native/tessera-ui
CORE_MODS = "a11y appicon egl env icon_theme icons picture seed services settings shell stats surfaces theme util wallpaper wayland Msg".split()
# Helpers one surface borrowed from another, now tessera-core's
MOVED = [
    ("crate::surfaces::dock::on_path", "crate::util::on_path"),
    ("crate::surfaces::emoji::klipy::encode", "crate::util::encode"),
    ("crate::surfaces::emoji::emoji_font", "tessera_core::settings::emoji_font"),
    ("crate::surfaces::osd::settings::island_hud", "tessera_core::settings::island_hud"),
    ("crate::surfaces::background::second_frame", "crate::picture::second_frame"),
    ("crate::surfaces::background::picture", "crate::picture"),
    ("crate::surfaces::background::wallpaper", "crate::wallpaper"),
    ("crate::surfaces::notifications::model", "crate::services::notify::model"),
    ("crate::surfaces::notifications::markup", "crate::services::notify::markup"),
]
EXTERNAL = ["serde_json", "serde", "image", "libc", "resvg", "ureq", "zbus", "calloop", "wayland_client", "accesskit", "material_colors", "memmap2"]
HEADER = '''pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;
'''


def old_file(rel):
    """A file as the branch had it before the merge (the working tree's copy if
    the merge left it there, else ORIG_HEAD's)"""
    p = WS / rel
    if p.exists():
        return p.read_text()
    r = subprocess.run(["git", "show", f"ORIG_HEAD:native/tessera-ui/{rel}"], cwd=WS, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else ""


def sync_tokens():
    """Theme tokens theme.rs's `tokens()` sets that the token list doesn't
    have yet (a branch's new token: its property in core/ui/theme.slint and
    its `t.set_…` lines came along with the merge): added to the list.
    Returns their names."""
    theme_rs = WS / "core/src/theme.rs"
    rs = theme_rs.read_text()
    slint = (WS / "core/ui/theme.slint").read_text()
    glob = slint[slint.index("export global Theme {"):]
    glob = glob[: glob.index("\n}")]
    types = {m.group(2).replace("-", "_"): m.group(1) for m in re.finditer(r"in property <([a-z]+)> ([a-z0-9-]+)", glob)}
    rust = {"bool": "bool", "string": "slint::SharedString", "length": "f32", "float": "f32", "color": "slint::Color", "duration": "i64"}
    listed = set(re.findall(r"^ {12}(\w+) / set_\w+: ", rs, re.M))
    body = rs[rs.index("pub fn tokens("):]
    added = []
    for name in dict.fromkeys(re.findall(r"\bt\.set_(\w+)\(", body)):
        if name in listed:
            continue
        if name not in types:
            print(f"still to do: theme.rs sets {name}, which core/ui/theme.slint's Theme hasn't")
            continue
        last = list(re.finditer(r"^ {12}\w+ / set_\w+: [^,]+,\n", rs, re.M))[-1]
        rs = rs[: last.end()] + f"            {name} / set_{name}: {rust[types[name]]},\n" + rs[last.end():]
        added.append(name)
    theme_rs.write_text(rs)
    return added


def main():
    if sys.argv[1:] == ["--tokens"]:
        print("tokens added:", ", ".join(sync_tokens()) or "none")
        return
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("name")
    ap.add_argument("--part")
    ap.add_argument("--slint", nargs="*")
    ap.add_argument("--export", nargs="*")
    a = ap.parse_args()
    name = a.name
    if not re.fullmatch(r"[a-z][a-z0-9]*", name):
        sys.exit(f"{name}: a surface's name is lowercase letters and digits")
    dest = WS / "surfaces" / name
    if dest.exists():
        sys.exit(f"{dest} exists already")
    # (git's rename detection may have moved a new folder along with its neighbours)
    src = next((d for d in (WS / "src/surfaces" / name, WS / "core/src/surfaces" / name) if d.is_dir()), None)
    if not src:
        sys.exit(f"no src/surfaces/{name}/ (nor core/src/surfaces/{name}/)")
    slints = a.slint or [f"{name}.slint"]
    found = []
    for f in slints:
        p = next((d / f for d in (WS / "ui", WS / "core/ui") if (d / f).is_file()), None)
        if not p:
            sys.exit(f"{f}: not in ui/ nor core/ui/ (--slint names them)")
        found.append(p)

    # The part's type, and its exports, from the old registry and ui/main.slint
    part = a.part
    if not part:
        reg = old_file("src/surfaces/mod.rs")
        m = re.search(rf'"{name}" => Some\(Box::new\({name}::(\w+)::default\(\)\)\)', reg)
        if not m:
            sys.exit(f"the part's type wasn't in the old make(): give --part")
        part = m.group(1)
    exports = a.export
    if not exports:
        main_slint = old_file("ui/main.slint")
        exports = [l for l in main_slint.splitlines() if l.startswith("export") and any(f'"{f}"' in l for f in slints)]
        if not exports:
            sys.exit("its export lines weren't in the old ui/main.slint: give --export")

    # Rust: move, lib.rs's header, the helpers that moved
    (dest / "src").mkdir(parents=True)
    for f in sorted(src.rglob("*")):
        if f.is_file():
            to = dest / "src" / f.relative_to(src)
            if to == dest / "src/mod.rs":
                to = dest / "src/lib.rs"
            to.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(f), to)
    shutil.rmtree(src)
    rust = sorted((dest / "src").rglob("*.rs"))
    left = []
    for f in rust:
        s = f.read_text()
        for old, new in MOVED:
            s = s.replace(old, new)
        # (the crate sits two folders deeper: native/tessera-ui/surfaces/<name>)
        s = re.sub(r'(env!\("CARGO_MANIFEST_DIR"\)[^\n]*?)"/\.\./\.\.', r'\1"/../../../..', s)
        s = re.sub(r'(env!\("CARGO_MANIFEST_DIR"\)\)\.join\(")\.\./\.\.', r'\1../../../..', s)
        if f.name == "lib.rs":
            lines = s.split("\n")
            i = 0
            while i < len(lines) and lines[i].startswith("//!"):
                i += 1
            body = []
            for l in lines[i:]:
                m = re.match(r"use crate::(\w+)(.*)$", l)
                if m and m.group(1) == "ui" and m.group(2) == ";":
                    continue
                if m and m.group(1) in CORE_MODS:
                    l = f"use tessera_core::{m.group(1)}{m.group(2)}"
                body.append(l)
            s = "\n".join(lines[:i]) + "\n\n" + HEADER + "\n" + "\n".join(body).lstrip("\n")
        left += [f"{f.relative_to(WS)}: {m}" for m in re.findall(r"crate::surfaces::(?!shapes|tell|announce|on_screen|Surfaces)\w+::\w+", s)]
        f.write_text(s)

    # Slint: its files, imports from the shared library
    (dest / "ui").mkdir()
    for p in found:
        s = p.read_text()
        s = s.replace('from "theme.slint";', 'from "@tessera/theme.slint";').replace('from "widgets.slint";', 'from "@tessera/widgets.slint";')
        s = re.sub(r'import \{ *(SymIcon, Symbols|Symbols, SymIcon|SymIcon|Symbols) *\} from "island.slint";', r'import { \1 } from "@tessera/symbols.slint";', s)
        left += [f"{p.name}: imports {m} (another surface's: move what it needs to core/ui/)" for m in re.findall(r'from "((?!@tessera/)[^"]+\.slint)"', s) if m not in slints]
        (dest / "ui" / p.name).write_text(s)
        p.unlink()
    (dest / "ui/main.slint").write_text(
        "// This crate's Slint, for slint-build (build.rs) and Rust's `ui` module: the\n"
        "// shared Theme (filled by tessera_core::theme_global!) and its own components.\n\n"
        'export { Theme, Icons } from "@tessera/theme.slint";\n' + "\n".join(exports) + "\n")
    (dest / "build.rs").write_text((WS / "surfaces/dock/build.rs").read_text())
    text = "\n".join(f.read_text() for f in rust)
    deps = [d for d in EXTERNAL if re.search(rf"\b{d}::", text)]
    if ("Deserialize" in text or "Serialize" in text) and "serde" not in deps:
        deps.append("serde")
    names = {"wayland_client": "wayland-client", "material_colors": "material-colors"}
    (dest / "Cargo.toml").write_text(f'''[package]
name = "tessera-{name}"
description = "tessera-ui: the {name} surface"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
doctest = false # (comments\' code blocks are prose, not examples)

[dependencies]
tessera-core.workspace = true
slint.workspace = true
smithay-client-toolkit.workspace = true
''' + "".join(f"{names.get(d, d)}.workspace = true\n" for d in sorted(set(deps))) + '''
[build-dependencies]
slint-build.workspace = true
''')

    # Registration: the binary's dependency, KNOWN, make()
    cargo = (WS / "Cargo.toml").read_text()
    line = f'tessera-{name} = {{ path = "surfaces/{name}" }}\n'
    if line not in cargo:
        last = list(re.finditer(r'^tessera-[a-z0-9]+ = \{ path = "surfaces/[a-z0-9]+" \}\n', cargo, re.M))[-1]
        cargo = cargo[: last.end()] + line + cargo[last.end():]
        (WS / "Cargo.toml").write_text(cargo)
    reg = (WS / "src/surfaces.rs").read_text()
    if f'"{name}"' not in reg.split("pub const KNOWN")[1].split("];")[0]:
        reg = re.sub(r'(pub const KNOWN: &\[&str\] = &\[[^\]]*)\];', rf'\1, "{name}"];', reg, count=1)
    if f'"{name}" =>' not in reg:
        reg = reg.replace("        _ => None,", f'        "{name}" => Some(Box::new(tessera_{name}::{part}::default())),\n        _ => None,', 1)
    (WS / "src/surfaces.rs").write_text(reg)

    tokens = sync_tokens()
    print(f"surfaces/{name}/: {len(rust)} .rs, {len(found)} .slint; registered as tessera_{name}::{part}")
    if tokens:
        print(f"theme tokens added to the list in core/src/theme.rs: {', '.join(tokens)}")
    print(f"dependencies: {', '.join(sorted(set(deps))) or '(none beyond core, slint, sctk)'} (check Cargo.toml)")
    for l in left:
        print("still to do:", l)


if __name__ == "__main__":
    main()
