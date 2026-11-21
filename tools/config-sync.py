#!/usr/bin/env python3
"""The installed copies of Tessera's files brought up to the checkout after
an update (a default install copies them; install.sh --dev links them):

  ~/.config/quickshell/tessera   a copy of shell/: always replaced whole
                                 (a link, a dev install's, is left alone)
  ~/.config/hypr/hyprland.lua    and ~/.config/hypr/tessera/*: hypr/'s files,
                                 which you may have edited: merged, three ways

    config-sync.py sync [--old-rev REV] [--backup DIR] [--json]
    config-sync.py base [--json]     record what's shipped now as the base
                                     (install.sh, after it copies the files)

The merge, per file, between the base (what Tessera shipped last time: kept
in $XDG_DATA_HOME/tessera/hypr-base/), yours, and the new version:
  - lines only you changed stay yours; lines only Tessera changed are
    updated; lines you deleted stay deleted; new files are added
  - both changed the same lines (a clash): your lines win, the new version
    is saved beside yours as <file>.new, and a notification says so
  - JSON files (keymap.json, gestures.json): by line when that fits, else by
    key (lists of objects by their "id"); the result must be valid JSON (Lua
    must pass `luac -p`, when luac is there), or yours stays with a .new
  - a file you deleted stays deleted, unless the config loads it without a
    pcall (hyprland.lua's dofile lines, a module's dofile or json.read): then
    it's put back, and a notification says so
  - a file Tessera no longer ships goes only if you never edited it
  - files the installer writes for this computer (keyboard.lua, gpu.lua,
    vm.lua, from-your-old-config.lua) are never touched
  - no base yet (an install from before this): a file the same as the new
    one is fine; one that differs merges against the old version's (git, the
    commit the update started from); without that, yours stays, with a .new

Everything it changes in hypr is first copied to the backup folder (the
update's, else $XDG_DATA_HOME/tessera/backups/config-sync-<time>). Running
it again changes nothing more. Its last report: $XDG_STATE_HOME/tessera/config-sync.json.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# Written by install.sh for this computer: never shipped, never touched
GENERATED = frozenset({"tessera/keyboard.lua", "tessera/gpu.lua", "tessera/vm.lua", "tessera/from-your-old-config.lua"})
BASE_MARK = ".base.json"  # (in the base folder: which commit it is)
NOTIFY = None  # tests: a function (title, body) instead of notify-send


def _xdg(var, default):
    return os.environ.get(var) or os.path.expanduser(default)


def config_home():
    return _xdg("XDG_CONFIG_HOME", "~/.config")


def data_dir():
    return os.path.join(_xdg("XDG_DATA_HOME", "~/.local/share"), "tessera")


def state_dir():
    return os.path.join(_xdg("XDG_STATE_HOME", "~/.local/state"), "tessera")


def base_dir():
    return os.path.join(data_dir(), "hypr-base")


def _read(path):
    try:
        with open(path, "rb") as f:
            return f.read()
    except (FileNotFoundError, IsADirectoryError, NotADirectoryError):
        return None


def _write(path, data, like=None):
    """PATH becomes DATA in one rename (its mode kept, or LIKE's)"""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    mode = None
    for p in (path, like):
        if p and os.path.isfile(p):
            mode = os.stat(p).st_mode & 0o7777
            break
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".tessera-sync-")
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.chmod(tmp, 0o644 if mode is None else mode)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def git_show(repo, rev, path):
    if not rev:
        return None
    try:
        r = subprocess.run(["git", "-C", repo, "show", f"{rev}:{path}"], capture_output=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    return r.stdout if r.returncode == 0 else None


def git_rev(repo):
    try:
        r = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], capture_output=True, text=True, timeout=30)
        return r.stdout.strip() if r.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


# ── What's shipped ──────────────────────────────────────────────────────

def shipped(repo):
    """{rel: bytes}: hypr/hyprland.lua as "hyprland.lua" and each file of
    hypr/tessera as "tessera/<name>" (what install.sh copies)"""
    out = {}
    lua = _read(os.path.join(repo, "hypr", "hyprland.lua"))
    if lua is not None:
        out["hyprland.lua"] = lua
    d = os.path.join(repo, "hypr", "tessera")
    if os.path.isdir(d):
        for n in sorted(os.listdir(d)):
            p = os.path.join(d, n)
            if os.path.isfile(p) and not os.path.islink(p) and "tessera/" + n not in GENERATED:
                out["tessera/" + n] = _read(p)
    return out


def shipped_at(repo, rev):
    """shipped() as it was at commit REV (git), or None"""
    if not rev:
        return None
    try:
        r = subprocess.run(["git", "-C", repo, "ls-tree", "-r", "--name-only", "-z", rev, "--", "hypr/hyprland.lua", "hypr/tessera"],
                           capture_output=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    if r.returncode != 0:
        return None
    out = {}
    for path in r.stdout.decode("utf-8", "replace").split("\0"):
        if not path:
            continue
        rel = path[len("hypr/"):]
        if rel != "hyprland.lua" and not (rel.startswith("tessera/") and "/" not in rel[len("tessera/"):]):
            continue
        if rel in GENERATED:
            continue
        data = git_show(repo, rev, path)
        if data is not None:
            out[rel] = data
    return out or None


def read_base():
    """{rel: bytes} of the base folder, or None when there's none"""
    d = base_dir()
    if not os.path.isfile(os.path.join(d, BASE_MARK)):
        return None
    out = {}
    rels = ["hyprland.lua"]
    if os.path.isdir(os.path.join(d, "tessera")):
        rels += ["tessera/" + n for n in sorted(os.listdir(os.path.join(d, "tessera")))]
    for rel in rels:
        data = _read(os.path.join(d, rel))
        if data is not None:
            out[rel] = data
    return out


def write_base(files, rev):
    """The base folder becomes FILES (what was shipped), swapped in whole"""
    d = base_dir()
    os.makedirs(os.path.dirname(d), exist_ok=True)
    new = tempfile.mkdtemp(dir=os.path.dirname(d), prefix=".hypr-base-")
    for rel, data in files.items():
        _write(os.path.join(new, rel), data)
    with open(os.path.join(new, BASE_MARK), "w") as f:
        json.dump({"rev": rev, "at": int(time.time())}, f)
    old = d + ".old"
    shutil.rmtree(old, ignore_errors=True)
    if os.path.lexists(d):
        os.replace(d, old)
    os.replace(new, d)
    shutil.rmtree(old, ignore_errors=True)


# ── Merging ─────────────────────────────────────────────────────────────

def merge_lines(mine, base, new):
    """git merge-file of three texts: (merged, clashes). Clashes are the
    hunks both changed, resolved as yours (--ours); clashes 0: a clean merge.
    None when it can't be done (not text, no git)."""
    try:
        texts = [x.decode("utf-8") for x in (mine, base, new)]
    except UnicodeDecodeError:
        return None
    with tempfile.TemporaryDirectory(prefix="tessera-sync-") as d:
        paths = []
        for name, text in zip(("mine", "base", "new"), texts):
            p = os.path.join(d, name)
            with open(p, "w") as f:
                f.write(text)
            paths.append(p)
        try:
            plain = subprocess.run(["git", "merge-file", "-p", "--quiet", *paths], capture_output=True, timeout=30)
            if plain.returncode < 0 or plain.returncode > 127:
                return None
            if plain.returncode == 0:
                return plain.stdout, 0
            ours = subprocess.run(["git", "merge-file", "-p", "--quiet", "--ours", *paths], capture_output=True, timeout=30)
        except (OSError, subprocess.SubprocessError):
            return None
        if ours.returncode < 0:
            return None
        return ours.stdout, plain.returncode


def _by_id(v):
    """A list of objects each with its own "id": {id: object}, else None"""
    if not isinstance(v, list) or not v or not all(isinstance(x, dict) and isinstance(x.get("id"), str) for x in v):
        return None
    ids = [x["id"] for x in v]
    return None if len(set(ids)) != len(ids) else {x["id"]: x for x in v}


_GONE = object()


def merge_values(mine, base, new):
    """(merged, clashes) of three JSON values, key by key: a key only one
    side changed takes that side's; both changed it: yours (a clash).
    Lists of objects with ids are merged by id, other lists are values."""
    if mine == new:
        return mine, 0
    if mine == base:
        return new, 0
    if new == base:
        return mine, 0
    as_ids = [_by_id(x) for x in (mine, base, new)]
    if all(x is not None for x in as_ids):
        merged, clashes = merge_values(*as_ids)
        return list(merged.values()), clashes
    if isinstance(mine, dict) and isinstance(new, dict) and isinstance(base, dict):
        out, clashes = {}, 0
        keys = list(new.keys()) + [k for k in mine if k not in new]
        for k in keys + [k for k in base if k not in keys]:
            m, b, n = mine.get(k, _GONE), base.get(k, _GONE), new.get(k, _GONE)
            if m is _GONE and n is _GONE:
                continue
            if m is _GONE or n is _GONE or b is _GONE:
                # (added, or deleted on one side)
                if m == b:
                    v = n
                elif n == b:
                    v = m
                elif m is _GONE:
                    v, clashes = _GONE, clashes + 1  # (you deleted it; the update changed it: yours)
                else:
                    v, clashes = m, clashes + (0 if n == m else 1)
            else:
                v, c = merge_values(m, b, n)
                clashes += c
            if v is not _GONE:
                out[k] = v
        return out, clashes
    return mine, 1


def dump_json(v):
    return (json.dumps(v, indent=1, ensure_ascii=False) + "\n").encode()


def valid(rel, data):
    """Is DATA a well-formed file for REL? JSON parses; Lua passes luac -p
    (when luac is there: True without it)"""
    if rel.endswith(".json"):
        try:
            json.loads(data.decode("utf-8"))
            return True
        except (ValueError, UnicodeDecodeError):
            return False
    if rel.endswith(".lua"):
        return lua_ok(data) is not False
    return True


def lua_ok(data):
    """True or False from luac -p, None when there's no luac"""
    luac = shutil.which("luac") or shutil.which("luac5.4")
    if not luac:
        return None
    with tempfile.NamedTemporaryFile(suffix=".lua") as f:
        f.write(data)
        f.flush()
        try:
            r = subprocess.run([luac, "-p", f.name], capture_output=True, timeout=30)
        except (OSError, subprocess.SubprocessError):
            return None
    return r.returncode == 0


def merge_file(rel, mine, base, new):
    """(result, clashes, how) for one file: result the bytes yours become,
    clashes how many of your changes were kept over Tessera's, how: "same",
    "theirs", "yours", "merged", "fallback" (yours kept whole: no merge)."""
    if mine == new:
        return mine, 0, "same"
    if base is None:
        return mine, 1, "fallback"
    if mine == base:
        return new, 0, "theirs"
    if new == base:
        return mine, 0, "yours"
    # (validity is judged only when the new file passes the same check: a
    # luac of another Lua than Hyprland's says nothing then)
    check = valid(rel, new)
    got = merge_lines(mine, base, new)
    if got is not None:
        text, clashes = got
        if not check or valid(rel, text):
            if clashes == 0 or not rel.endswith(".json"):
                return text, clashes, "merged"
    if rel.endswith(".json"):
        try:
            m, b, n = (json.loads(x.decode("utf-8")) for x in (mine, base, new))
        except (ValueError, UnicodeDecodeError):
            return mine, 1, "fallback"
        merged, clashes = merge_values(m, b, n)
        out = dump_json(merged)
        if valid(rel, out):
            return out, clashes, "merged"
    return mine, 1, "fallback"


# ── What the config can't do without ────────────────────────────────────
# (`pcall(dofile, …)` passes dofile without calling it: only `dofile(…)` counts)
NEEDS = [re.compile(r'\bdofile\(\s*(?:dir\s*\.\.\s*"/tessera/|TESSERA_HYPR_DIR\s*\.\.\s*"/)([\w.-]+)"\s*\)'),
         re.compile(r'\bdofile\(\s*here\s*\.\.\s*"/([\w.-]+)"\s*\)'),
         re.compile(r'json\.read\(\s*here\s*\.\.\s*"/([\w.-]+)"\s*\)(?!\s*or\b)')]


def required(files):
    """The files ({rel: bytes}) the config loads without a pcall, from
    hyprland.lua on: a missing one stops Hyprland's config"""
    need, todo = {"hyprland.lua"}, ["hyprland.lua"]
    while todo:
        text = (files.get(todo.pop()) or b"").decode("utf-8", "replace")
        text = "\n".join(l.split("--", 1)[0] for l in text.splitlines())  # (not commented-out lines)
        for pat in NEEDS:
            for m in pat.finditer(text):
                rel = "tessera/" + m.group(1)
                if rel not in need:
                    need.add(rel)
                    if rel.endswith(".lua"):
                        todo.append(rel)
    return need


# ── Notifications ───────────────────────────────────────────────────────

def notify(title, body):
    """notify-send (-a Tessera), once a notification server answers: at a
    login the shell, which shows them, starts after this"""
    if NOTIFY:
        return NOTIFY(title, body)
    if os.environ.get("TESSERA_NO_NOTIFY") == "1" or not shutil.which("notify-send"):
        return None
    script = ('i=0; while [ $i -lt 150 ]; do busctl --user status org.freedesktop.Notifications >/dev/null 2>&1 && break; '
              'sleep 2; i=$((i+1)); done; exec notify-send -a Tessera -i system-software-update -- "$1" "$2"')
    try:
        subprocess.Popen(["sh", "-c", script, "sh", title, body], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL, start_new_session=True)
    except OSError:
        pass
    return None


# ── The shell's copy ────────────────────────────────────────────────────

def sync_shell(repo, config=None):
    """~/.config/quickshell/tessera replaced by the checkout's shell/ (a
    link, a dev install's, is left alone; no copy there, nothing made).
    Returns "replaced", "linked", "none" or "same"."""
    dest = os.path.join(config or config_home(), "quickshell", "tessera")
    src = os.path.join(repo, "shell")
    if os.path.islink(dest):
        return "linked"
    if not os.path.isdir(dest) or not os.path.isfile(os.path.join(src, "shell.qml")):
        return "none"
    if os.path.realpath(dest) == os.path.realpath(src):
        return "same"
    parent = os.path.dirname(dest)
    new = tempfile.mkdtemp(dir=parent, prefix=".tessera.new-")
    try:
        os.rmdir(new)
        shutil.copytree(src, new, symlinks=True, ignore=shutil.ignore_patterns("__pycache__"))
        old = tempfile.mkdtemp(dir=parent, prefix=".tessera.old-")
        os.rmdir(old)
        os.replace(dest, old)
        os.replace(new, dest)
    except BaseException:
        shutil.rmtree(new, ignore_errors=True)
        raise
    # (a script of the old copy still running keeps its open file)
    shutil.rmtree(old, ignore_errors=True)
    return "replaced"


# ── hypr ────────────────────────────────────────────────────────────────

def sync_hypr(repo, old_rev=None, config=None, backup=None, say=None):
    """The three-way merge of ~/.config/hypr's Tessera files (see above).
    Returns {"done", "changed", "added", "removed", "restored", "kept",
    "clashes", "new_files", "notes", "base"}."""
    say = say or (lambda s: None)
    hypr = os.path.join(config or config_home(), "hypr")
    out = {"done": False, "changed": [], "added": [], "removed": [], "restored": [], "kept": [], "clashes": {},
           "new_files": [], "notes": [], "base": ""}
    if os.path.islink(os.path.join(hypr, "tessera")):
        out["why"] = "linked"  # (hypr/tessera is the checkout's own: nothing to bring up)
        return out
    if not os.path.isdir(os.path.join(hypr, "tessera")):
        out["why"] = "none"  # (not installed here)
        return out
    new = shipped(repo)
    if "hyprland.lua" not in new:
        out["why"] = "no hypr/ in the checkout"
        return out
    base = read_base()
    out["base"] = "kept" if base is not None else "none"
    old = None
    if base is None:
        old = shipped_at(repo, old_rev)
        out["base"] = "git" if old is not None else "none"
    known = base if base is not None else old  # (what was shipped before, when known)

    results = {}  # rel -> bytes yours become (None: removed)
    for rel in sorted(set(new) | set(known or {})):
        if rel in GENERATED:
            continue
        path = os.path.join(hypr, rel)
        if os.path.islink(path):
            out["kept"].append(rel)  # (a link of yours: left as it is)
            continue
        mine = _read(path)
        n = new.get(rel)
        b = (known or {}).get(rel)
        if n is None:
            # Tessera no longer ships it: it goes only if you never edited it
            if mine is not None and b is not None and mine == b:
                results[rel] = None
                out["removed"].append(rel)
            elif mine is not None:
                out["kept"].append(rel)
                out["notes"].append(f"{rel}: Tessera no longer uses it; your edited copy is kept")
            continue
        if mine is None:
            if known is not None and b is not None:
                continue  # (you deleted it: it stays deleted, unless it's needed: below)
            results[rel] = n
            out["added"].append(rel)
            continue
        merged, clashes, how = merge_file(rel, mine, b, n)
        if merged != mine:
            results[rel] = merged
            out["changed"].append(rel)
        if how == "fallback" or clashes:
            out["clashes"][rel] = clashes
            name = os.path.basename(rel)
            if _read(path + ".new") != n:
                results[rel + ".new"] = n
            out["new_files"].append(rel + ".new")
            if how == "fallback":
                out["notes"].append(f"{name}: kept as yours (Tessera's changes didn't merge) — see {name}.new")
            else:
                out["notes"].append(f"{name}: {clashes} change{'s' if clashes != 1 else ''} kept as yours — see {name}.new")
        elif _read(path + ".new") is not None and merged == n:
            # (an earlier clash, settled: the .new is the same as yours now)
            results[rel + ".new"] = None

    # What the config loads without a pcall, if gone: back, with a note
    after = {}
    for rel in set(new) | set(known or {}):
        p = os.path.join(hypr, rel)
        after[rel] = results[rel] if rel in results else _read(p)
    for rel in sorted(required({k: v for k, v in after.items() if v is not None})):
        if after.get(rel) is None and rel in new:
            results[rel] = new[rel]
            out["restored"].append(rel)
            out["notes"].append(f"{os.path.basename(rel)}: put back — your deleted copy is needed by Tessera's config")

    # Back up what changes, then change it
    touched = [rel for rel in results if os.path.lexists(os.path.join(hypr, rel))]
    if touched:
        backup = backup or os.path.join(data_dir(), "backups", "config-sync-" + time.strftime("%Y%m%d-%H%M%S"))
        for rel in touched:
            dst = os.path.join(backup, "hypr", rel)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copy2(os.path.join(hypr, rel), dst)
        out["backup"] = backup
    for rel, data in sorted(results.items()):
        p = os.path.join(hypr, rel)
        if data is None:
            if os.path.lexists(p):
                os.remove(p)
        else:
            _write(p, data, like=None)
    write_base(new, git_rev(repo))
    out["done"] = True
    out["any"] = bool(results)
    for n in out["notes"]:
        say(n)
    return out


def sync(repo=REPO, old_rev=None, config=None, backup=None, say=None, shell=True):
    """Both: the shell's copy and hypr. Never raises: a part that fails says
    why in the report. Notifies what you need to know (clashes, files put
    back) and writes the report to $XDG_STATE_HOME/tessera/config-sync.json."""
    report = {"at": int(time.time()), "rev": git_rev(repo)}
    if shell:
        try:
            report["shell"] = sync_shell(repo, config)
        except OSError as e:
            report["shell"] = "failed"
            report["shell_why"] = str(e)
    try:
        report["hypr"] = sync_hypr(repo, old_rev, config, backup, say)
    except (OSError, ValueError) as e:
        report["hypr"] = {"done": False, "why": str(e), "notes": []}
    notes = report["hypr"].get("notes") or []
    if any("kept as yours" in n or "put back" in n for n in notes):
        shown = [n for n in notes if "kept as yours" in n or "put back" in n]
        notify("Your Hyprland settings were kept", "\n".join(shown[:4]) + ("\n…" if len(shown) > 4 else ""))
    try:
        os.makedirs(state_dir(), exist_ok=True)
        with open(os.path.join(state_dir(), "config-sync.json"), "w") as f:
            json.dump(report, f, indent=1)
    except OSError:
        pass
    return report


def main(argv):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["sync", "base"])
    ap.add_argument("--old-rev", default="", help="sync: the commit the update started from (the base when none is kept yet)")
    ap.add_argument("--backup", default="", help="sync: where to copy the files it changes first")
    ap.add_argument("--repo", default=REPO)
    ap.add_argument("--no-shell", action="store_true", help="sync: hypr only (install.sh copies the shell itself)")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args(argv)
    if a.command == "base":
        write_base(shipped(a.repo), git_rev(a.repo))
        out = {"base": base_dir()}
    else:
        out = sync(a.repo, a.old_rev or None, backup=a.backup or None, say=None if a.json else print, shell=not a.no_shell)
    if a.json:
        print(json.dumps(out, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
