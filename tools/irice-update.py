#!/usr/bin/env python3
"""iRice updater: gets the latest iRice from its git remote without changing
anyone's settings or shortcuts. Anything an update would change for you is a
conflict you decide, and nothing is applied until all of them are decided.

    irice-update.py check  --config <dir>            -> JSON report on stdout
    irice-update.py apply  --config <dir> --choices <file.json>

<dir> is where the shell keeps config.json and keybinds.json. The repository
is the one this script lives in.

Conflicts (each with the choices given):
  code      a file you changed that the update changes too:
            both (only when the two sets of changes fit together, never for
            binary files) | mine (the update's change to it is dropped) |
            new (yours is set aside in the backup folder)
            Deleted on one side and edited on the other: mine | new, where
            "new" follows the update (deletes it, or brings it back)
            A file of yours that isn't in git and the update adds: mine | new
  setting   a setting whose default changes:                 keep | new
            (your config.json holds every value, so "keep" changes nothing)
  preset    the material you use (Glass, Frost) looks different in the update:
            keep (your look, as a Custom material) | new
  removed   a setting you changed that the update drops or renames:
            drop | move (to its new name, when it was renamed)
  shortcut  a new or changed default shortcut uses keys you gave to something
            else:  mine (the new default stays off) | new (yours goes)

The choices file maps each conflict's "id" to a choice.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONFIG_QML = "shell/common/Config.qml"
THEME_QML = "shell/common/Theme.qml"
KEYMAP = "hypr/irice/keymap.json"
MIGRATIONS = "tools/settings-migrations.json"  # {"old.path": "new.path"}


def git(*args, check=True):
    r = subprocess.run(["git", "-C", REPO, *args], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise RuntimeError("git " + " ".join(args) + ": " + (r.stderr.strip() or r.stdout.strip()))
    return r.stdout


def show(rev, path):
    """A text file at a revision (None when it isn't there). Non-text bytes
    are replaced, so a binary file never stops a check."""
    b = show_bytes(rev, path)
    return None if b is None else b.decode("utf-8", "replace")


def show_bytes(rev, path):
    r = subprocess.run(["git", "-C", REPO, "show", f"{rev}:{path}"], capture_output=True)
    return r.stdout if r.returncode == 0 else None


def read_bytes(path):
    try:
        with open(os.path.join(REPO, path), "rb") as fh:
            return fh.read()
    except OSError:
        return None


def is_binary(*blobs):
    return any(b is not None and b"\0" in b[:8000] for b in blobs)


def numstat(*args):
    """(added, removed) lines from git diff --numstat; None for binary."""
    out = git("diff", "--numstat", *args, check=False).split("\n")[0].split("\t")
    if len(out) < 2 or out[0] == "-":
        return None
    try:
        return int(out[0]), int(out[1])
    except ValueError:
        return None


def describe_change(st):
    return "binary" if st is None else f"+{st[0]} −{st[1]}"


# ── Defaults out of the QML ────────────────────────────────────────────────

LEAF = re.compile(r"^\s*(?:readonly\s+)?property\s+([\w<>]+)\s+(\w+)\s*:\s*(.+?)\s*$")
SECTION = re.compile(r"^\s*property\s+JsonObject\s+(\w+)\s*:\s*JsonObject\s*\{")


def strip_comment(v):
    out, quote = [], None
    i = 0
    while i < len(v):
        c = v[i]
        if quote:
            out.append(c)
            if c == "\\" and i + 1 < len(v):
                out.append(v[i + 1])
                i += 1
            elif c == quote:
                quote = None
        elif c in "\"'":
            quote = c
            out.append(c)
        elif v.startswith("//", i):
            break
        else:
            out.append(c)
        i += 1
    return "".join(out).strip()


def literal(v):
    v = strip_comment(v)
    try:
        return json.loads(v)
    except ValueError:
        return None  # an expression: no fixed default


def config_defaults(text):
    """{"section.key": default} from Config.qml's JsonAdapter."""
    out, path, depths, depth = {}, [], [], 0
    inside = False
    for line in (text or "").splitlines():
        if not inside:
            if "JsonAdapter" in line and "{" in line:
                inside, depth = True, 1
            continue
        m = SECTION.match(line)
        if m:
            path.append(m.group(1))
            depth += 1
            depths.append(depth)
            continue
        m = LEAF.match(line)
        if m and path and "{" not in m.group(3):
            val = literal(m.group(3))
            if val is not None:
                out[".".join(path + [m.group(2)])] = val
        depth += line.count("{") - line.count("}")
        while depths and depth < depths[-1]:
            depths.pop()
            path.pop()
        if depth <= 0:
            break
    return out


def glass_presets(text):
    m = re.search(r"presets:\s*\((\{.*?\})\)", text or "")
    if not m:
        return {}
    try:
        return json.loads(re.sub(r"(\w+):", r'"\1":', m.group(1)))
    except ValueError:
        return {}


# ── Shortcuts ──────────────────────────────────────────────────────────────

MODS = {"$MOD": "SUPER", "SUPER": "SUPER", "MOD4": "SUPER", "WIN": "SUPER", "META": "SUPER",
        "CTRL": "CTRL", "CONTROL": "CTRL", "ALT": "ALT", "MOD1": "ALT", "SHIFT": "SHIFT"}


def canonical(keys):
    mods, rest = [], []
    for p in [s.strip() for s in str(keys or "").split("+") if s.strip()]:
        u = p.upper()
        if u in MODS:
            if MODS[u] not in mods:
                mods.append(MODS[u])
        elif u in ("$MODKEY", "SUPER_L"):
            rest.append("$TAP")
        else:
            rest.append(p.lower())
    return "+".join(sorted(mods)) + "|" + "+".join(rest)


def keymap(text):
    try:
        d = json.loads(text or "[]")
        return {b["id"]: b for b in d if isinstance(b, dict) and b.get("id")}
    except ValueError:
        return {}


# ── Settings files ─────────────────────────────────────────────────────────

def load_json(path, fallback):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return fallback


def get_path(d, path):
    for k in path.split("."):
        if not isinstance(d, dict) or k not in d:
            return None, False
        d = d[k]
    return d, True


def set_path(d, path, value):
    keys = path.split(".")
    for k in keys[:-1]:
        d = d.setdefault(k, {})
    d[keys[-1]] = value


def del_path(d, path):
    keys = path.split(".")
    for k in keys[:-1]:
        d = d.get(k, {})
    d.pop(keys[-1], None)


def write_json(path, data):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(data, f, indent=2)
    os.replace(tmp, path)  # all at once: a running shell never reads half


# ── Check ──────────────────────────────────────────────────────────────────

def progress(percent, text):
    """A step for the update screen (shell/assets/updater), on stderr so
    stdout stays the JSON report."""
    print(f"PROGRESS {int(percent)} {text}", file=sys.stderr, flush=True)


def upstream():
    r = subprocess.run(["git", "-C", REPO, "rev-parse", "--abbrev-ref", "@{u}"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else "origin/main"


# ── Update notes (CHANGELOG.md) ────────────────────────────────────────────
# "# iRice changelog", then entries newest first: "## <Title> · <YYYY-MM-DD>"
# and markdown under it. An entry may end with <!-- packages: +a -b --> (the
# iRice packages it adds / drops).
ENTRY = re.compile(r"^## (.+?)\s*$", re.M)
PKGS = re.compile(r"<!--\s*packages:(.*?)-->", re.S)


def changelog_entries(text):
    if not text:
        return []
    heads = list(ENTRY.finditer(text))
    out = []
    for i, m in enumerate(heads):
        body = text[m.end():heads[i + 1].start() if i + 1 < len(heads) else len(text)]
        add, drop = [], []
        for pm in PKGS.finditer(body):
            for w in pm.group(1).split():
                (add if w.startswith("+") else drop if w.startswith("-") else add).append(w.lstrip("+-"))
        body = PKGS.sub("", body).strip()
        head = m.group(1).strip()
        title, _, date = head.rpartition(" · ")
        if not title:
            title, date = head, ""
        out.append({"heading": m.group(0).strip(), "title": title.strip(), "date": date.strip(),
                    "body": body, "packages": {"add": add, "drop": drop}})
    return out


def new_entries(base, new):
    """The entries in the new version's changelog that this copy doesn't have
    (by their exact heading line), newest first."""
    have = {e["heading"] for e in changelog_entries(show(base, "CHANGELOG.md"))}
    return [e for e in changelog_entries(show(new, "CHANGELOG.md")) if e["heading"] not in have]


def check(config_dir, fetch=True):
    if not os.path.isdir(os.path.join(REPO, ".git")):
        return {"error": "This copy of iRice isn't a git checkout, so it can't update itself. Get it with the install command from irice.monishbl.in."}
    if fetch:
        try:
            git("fetch", "--quiet")
        except RuntimeError as e:
            return {"error": "Couldn't reach the update server: " + str(e).split(": ", 1)[-1]}
    base, new = "HEAD", upstream()
    base_id, new_id = git("rev-parse", base).strip(), git("rev-parse", new).strip()
    behind = int(git("rev-list", "--count", f"{base}..{new}").strip() or 0)
    ahead = int(git("rev-list", "--count", f"{new}..{base}").strip() or 0)
    report = {"current": base_id[:8], "latest": new_id[:8], "upstream": new, "available": behind > 0,
              "ahead": ahead, "notes": [], "conflicts": [], "keeps": []}
    if behind == 0:
        return report
    if ahead > 0:
        report["error"] = f"This copy has {ahead} commit(s) of its own, so the update can't be applied on top automatically. Ask for help, or move your changes out first."
    report["notes"] = [l for l in git("log", "--format=%s", f"{base}..{new}").splitlines() if l][:30]
    # Proper notes when the changelog has them (the commit subjects otherwise)
    report["changelog"] = new_entries(base, new)

    # Code: files changed here that the update changes too. Every one is a
    # question: nothing of yours or the update's is dropped without asking
    local = set(l for l in git("diff", "--name-only", "-z", "HEAD").split("\0") if l)
    incoming = set(l for l in git("diff", "--name-only", "-z", base, new).split("\0") if l)
    for f in sorted(local & incoming):
        report["conflicts"].append(code_conflict(f, base, new))
    # Files of yours git doesn't track, that the update adds (taking the
    # update would otherwise stop on them)
    untracked = set(l for l in git("ls-files", "--others", "--exclude-standard", "-z").split("\0") if l)
    for f in sorted(untracked & incoming):
        theirs, mine = show_bytes(new, f), read_bytes(f)
        if theirs is None or theirs == mine:
            continue
        report["conflicts"].append({"id": "code:" + f, "kind": "code", "title": f, "change": "untracked",
                                    "detail": "You have a file here that iRice doesn't track, and the update adds one with the same name.",
                                    "mine": "your file", "theirs": "new in the update",
                                    "choices": ["mine", "new"],
                                    "labels": {"mine": "Keep mine", "new": "Use the update's (yours goes to the backup folder)"}})
    report["keeps"] = sorted(local - incoming)  # your edits the update doesn't touch: kept

    config = load_json(os.path.join(config_dir, "config.json"), {})
    old_def, new_def = config_defaults(show(base, CONFIG_QML)), config_defaults(show(new, CONFIG_QML))

    # Changed defaults for settings left as they were (not the Custom
    # material's sliders: they start from the current material when opened)
    skip = {"appearance.glassLens", "appearance.glassFrost", "appearance.glassSolid", "appearance.glassTint"}
    for key in sorted(set(old_def) & set(new_def)):
        if old_def[key] == new_def[key] or key in skip:
            continue
        mine, present = get_path(config, key)
        if present and mine != old_def[key]:
            continue  # you changed it: yours stays, nothing to ask
        report["conflicts"].append({"id": "setting:" + key, "kind": "setting", "title": key,
                                    "detail": "The default changes from " + json.dumps(old_def[key]) + " to " + json.dumps(new_def[key]) + ".",
                                    "old": old_def[key], "new": new_def[key],
                                    "choices": ["keep", "new"], "labels": {"keep": "Keep " + json.dumps(old_def[key]), "new": "Use " + json.dumps(new_def[key])}})

    # Removed or renamed settings you changed
    migrations = load_json_text(show(new, MIGRATIONS), {})
    for key in sorted(set(old_def) - set(new_def)):
        mine, present = get_path(config, key)
        if not present or mine == old_def[key]:
            continue
        target = migrations.get(key)
        report["conflicts"].append({"id": "removed:" + key, "kind": "removed", "title": key,
                                    "detail": ("Renamed to " + target + ". " if target else "The update no longer has this setting. ") + "Yours is " + json.dumps(mine) + ".",
                                    "value": mine, "target": target,
                                    "choices": ["move", "drop"] if target else ["drop"],
                                    "labels": {"move": "Move it to " + str(target), "drop": "Let it go"}})

    # Material presets that look different
    glass, _ = get_path(config, "appearance.glass")
    chosen = {"auto": "liquid", None: "liquid"}.get(glass, glass)
    old_p, new_p = glass_presets(show(base, THEME_QML)), glass_presets(show(new, THEME_QML))
    if chosen in old_p and chosen in new_p and old_p[chosen] != new_p[chosen]:
        report["conflicts"].append({"id": "preset:" + chosen, "kind": "preset", "title": "The " + {"liquid": "Glass", "frosted": "Frost"}.get(chosen, chosen) + " material",
                                    "detail": "It looks different in the update: " + describe_preset(old_p[chosen], new_p[chosen]) + ".",
                                    "old": old_p[chosen], "new": new_p[chosen],
                                    "choices": ["keep", "new"], "labels": {"keep": "Keep my look", "new": "Use the new look"}})

    # Shortcut clashes
    user = load_json(os.path.join(config_dir, "keybinds.json"), {})
    overrides = user.get("keybinds", {}) if isinstance(user.get("keybinds"), dict) else {}
    custom = [s for s in user.get("shortcuts", []) if isinstance(s, dict)]
    old_k, new_k = keymap(show(base, KEYMAP)), keymap(show(new, KEYMAP))
    yours = {}  # canonical keys -> (kind, id, label) you chose
    for bid, c in overrides.items():
        if isinstance(c, dict) and c.get("keys") and not c.get("disabled"):
            yours[canonical(c["keys"])] = ("override", bid, new_k.get(bid, old_k.get(bid, {})).get("label", bid))
    for s in custom:
        if s.get("keys") and not s.get("disabled"):
            yours[canonical(s["keys"])] = ("custom", s.get("id"), s.get("label") or s.get("id"))
    for bid, b in sorted(new_k.items()):
        o = old_k.get(bid)
        if o and canonical(o.get("keys")) == canonical(b.get("keys")):
            continue  # unchanged default
        c = overrides.get(bid, {})
        if c.get("disabled") or c.get("keys"):
            continue  # you've set this one yourself
        hit = yours.get(canonical(b.get("keys")))
        if not hit or hit[1] == bid:
            continue
        report["conflicts"].append({"id": "shortcut:" + bid, "kind": "shortcut", "title": re.sub(r"\$MOD\b", "Super", b.get("keys", "")),
                                    "detail": "The update gives these keys to “" + b.get("label", bid) + "”; you use them for “" + str(hit[2]) + "”.",
                                    "yours": {"kind": hit[0], "id": hit[1]},
                                    "choices": ["mine", "new"], "labels": {"mine": "Keep mine (" + str(hit[2]) + ")", "new": "Use the new one"}})

    # What gets rebuilt after it's applied
    report["rebuild"] = sorted({r for f in incoming for r, p in (("plugin", "hypr/plugin/"), ("native", "shell/native/"), ("shaders", ".frag")) if p in f})
    return report


def three_way(f, new):
    """Your file with the update's changes merged in, or None when they overlap."""
    import tempfile
    base, theirs, mine = show_bytes("HEAD", f), show_bytes(new, f), read_bytes(f)
    if base is None or theirs is None or mine is None or is_binary(base, theirs, mine):
        return None
    try:
        base, theirs, mine = base.decode(), theirs.decode(), mine.decode()
    except UnicodeDecodeError:
        return None
    with tempfile.TemporaryDirectory() as d:
        paths = []
        for name, text in (("mine", mine), ("base", base), ("new", theirs)):
            p = os.path.join(d, name)
            with open(p, "w") as fh:
                fh.write(text)
            paths.append(p)
        r = subprocess.run(["git", "merge-file", "-p", "--quiet", *paths], capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else None


def merges_cleanly(f, new):
    return three_way(f, new) is not None


def code_conflict(f, base, new):
    """A file both you and the update changed: what each did, and the
    choices that make sense for it."""
    b, mine, theirs = show_bytes(base, f), read_bytes(f), show_bytes(new, f)
    c = {"id": "code:" + f, "kind": "code", "title": f}
    if mine is None and theirs is not None:
        c.update(change="deleted-mine", mine="you deleted it", theirs="changed in the update (" + describe_change(numstat(base, new, "--", f)) + ")",
                 detail="You deleted this file; the update changes it.",
                 choices=["mine", "new"], labels={"mine": "Keep it deleted", "new": "Bring back the update's"})
    elif theirs is None and mine is not None:
        c.update(change="deleted-theirs", mine="changed by you (" + describe_change(numstat("HEAD", "--", f)) + ")", theirs="deleted in the update",
                 detail="You changed this file; the update deletes it.",
                 choices=["mine", "new"], labels={"mine": "Keep mine", "new": "Delete it (yours goes to the backup folder)"})
    elif is_binary(b, mine, theirs):
        c.update(change="binary", mine="changed by you", theirs="changed in the update",
                 detail="You changed this file and the update changes it too. It isn't text, so it's one or the other.",
                 choices=["mine", "new"], labels={"mine": "Keep mine", "new": "Use the update's (yours goes to the backup folder)"})
    else:
        both = merges_cleanly(f, new)
        c.update(change="edited", mine="your changes " + describe_change(numstat("HEAD", "--", f)),
                 theirs="the update's " + describe_change(numstat(base, new, "--", f)),
                 detail="You changed this file and the update changes it too." + (" Both sets of changes fit together." if both else " Your changes and the update's overlap, so it's one or the other."),
                 choices=(["both"] if both else []) + ["mine", "new"],
                 labels={"both": "Merge both", "mine": "Keep mine (the update's change is dropped)", "new": "Use the update's (yours goes to the backup folder)"})
    return c


def describe_preset(old, new):
    names = ["glass", "frost", "solid", "tint"]
    parts = [f"{names[i]} {round(o * 100)}% → {round(n * 100)}%" for i, (o, n) in enumerate(zip(old, new)) if o != n]
    return ", ".join(parts).capitalize() if parts else "small changes"


def load_json_text(text, fallback):
    try:
        return json.loads(text) if text else fallback
    except ValueError:
        return fallback


# ── Apply ──────────────────────────────────────────────────────────────────

def rebuild(todo, base=45, span=40):
    """Rebuilds parts (plugin, native, shaders). Failures are reported, not
    fatal: the shell falls back (frosted glass without the plugin)."""
    built, failed = [], []
    steps = {"plugin": ["make", "-C", os.path.join(REPO, "hypr/plugin/irice-glass")],
             "native": [os.path.join(REPO, "shell/native/build.sh")],
             "shaders": [os.path.join(REPO, "tools/build-shaders.sh")]}
    names = {"plugin": "Building Liquid Glass", "native": "Building native helpers", "shaders": "Building shaders"}
    for i, r in enumerate(todo):
        if r not in steps:
            continue
        progress(base + span * i / max(1, len(todo)), names[r])
        res = subprocess.run(steps[r], capture_output=True, text=True)
        (built if res.returncode == 0 else failed).append(r)
    return built, failed


def repo_lock():
    """One update at a time on this repository, from any session: the same
    lock update-run.sh takes (it says so with IRICE_UPDATE_HELD=1, and then
    holds it for us). Kept open (held) until this process ends."""
    if os.environ.get("IRICE_UPDATE_HELD") == "1":
        return None
    import fcntl
    f = open(os.path.join(REPO, ".git", "irice-update.lock"), "w")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        f.close()
        raise SystemExit("Another update is running on this copy of iRice. Wait for it to finish, then try again.")
    return f


def apply(config_dir, choices, extra=()):
    """extra: parts to rebuild whatever the update says (the glass plugin
    after a Hyprland update); with no iRice update, only those are rebuilt."""
    lock = repo_lock()  # noqa: F841 (held while this runs)
    report = check(config_dir, fetch=False)
    if report.get("error"):
        raise SystemExit(report["error"])
    if not report["available"]:
        if not extra:
            return {"applied": False, "message": "Already up to date."}
        built, failed = rebuild(list(extra), 10, 75)
        progress(85, "Finishing up")
        return {"applied": False, "rebuilt": True, "backup": "", "built": built, "failed": failed,
                "now": git("rev-parse", "--short", "HEAD").strip()}
    missing = [c["id"] for c in report["conflicts"] if choices.get(c["id"]) not in c["choices"]]
    if missing:
        raise SystemExit("Decide every conflict first: " + ", ".join(missing))

    progress(5, "Backing up your settings")
    # Back up settings and shortcuts
    stamp = time.strftime("%Y%m%d-%H%M%S") + "-" + str(time.time_ns() % 1000000000)
    backup = os.path.join(config_dir, "backups", "update-" + stamp)
    os.makedirs(backup, exist_ok=True)
    for name in ("config.json", "keybinds.json"):
        if os.path.exists(os.path.join(config_dir, name)):
            shutil.copy2(os.path.join(config_dir, name), backup)

    progress(15, "Installing the new version")
    # Code: set your edits aside, take the update, put back what you keep.
    # If anything after taking the update fails, everything goes back exactly
    # as it was (the old version, your edits, your settings) and it says so.
    old_head = git("rev-parse", "HEAD").strip()
    edited = [l for l in git("diff", "--name-only", "-z", "HEAD").split("\0") if l]
    # Your version of each file you chose the update's for, into the backup
    # folder (as well as the stash), so it's easy to find again
    untracked = [c["title"] for c in report["conflicts"] if c["kind"] == "code" and c.get("change") == "untracked"]
    for c in report["conflicts"]:
        f = c["title"]
        if c["kind"] == "code" and choices.get(c["id"]) == "new" and os.path.isfile(os.path.join(REPO, f)):
            dest = os.path.join(backup, "code", f)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            shutil.copy2(os.path.join(REPO, f), dest)
    # Untracked files the update adds: set aside so taking the update works,
    # put back afterwards if you keep yours (and on any failure)
    aside = os.path.join(backup, "untracked")

    def put_back_untracked():
        for f in untracked:
            src, dst = os.path.join(aside, f), os.path.join(REPO, f)
            if os.path.lexists(src):
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copy2(src, dst)
    merged = {}  # files kept as both: your edits with the update's merged in
    for f in edited:
        if choices.get("code:" + f) == "both":
            text = three_way(f, report["upstream"])
            if text is None:
                raise SystemExit(f + " no longer merges cleanly: choose mine or new for it")
            merged[f] = text
    stash = None
    try:
        for f in untracked:
            dest = os.path.join(aside, f)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            shutil.move(os.path.join(REPO, f), dest)
        if edited:
            git("stash", "push", "-m", "irice-update " + stamp)
            stash = git("rev-parse", "stash@{0}").strip()
        git("merge", "--ff-only", report["upstream"])
    except BaseException:
        try:
            if stash:
                git("stash", "pop")
        finally:
            put_back_untracked()
        raise
    try:
        apply_after_merge(config_dir, choices, report, edited, merged, stash)
        # Untracked files you keep: yours over the update's
        for f in untracked:
            if choices.get("code:" + f) == "mine":
                shutil.copy2(os.path.join(aside, f), os.path.join(REPO, f))
    except BaseException as e:
        # Back to exactly how it was: the old version, then your edits, then
        # your settings (from the backup made above)
        why = str(e) or e.__class__.__name__
        restored = ""
        try:
            git("reset", "--hard", "--quiet", old_head)
            if stash:
                git("stash", "pop")
            put_back_untracked()
            for name in ("config.json", "keybinds.json"):
                if os.path.exists(os.path.join(backup, name)):
                    shutil.copy2(os.path.join(backup, name), os.path.join(config_dir, name))
            restored = " Nothing was changed: iRice, your edits and your settings are as they were."
        except Exception as e2:
            restored = (" It couldn't all be put back (" + str(e2) + ")." +
                        (" Your edits are kept in git stash " + stash + " (git stash pop to get them back)." if stash else "") +
                        " Your settings backup is in " + backup + ".")
        raise SystemExit("The update didn't finish: " + why + "." + restored)
    todo = list(report.get("rebuild", []))
    todo += [r for r in extra if r not in todo]
    built, failed = rebuild(todo)
    progress(85, "Finishing up")
    # (the notes, for the new shell's "What's new")
    return {"applied": True, "backup": backup, "stash": stash, "built": built, "failed": failed,
            "now": git("rev-parse", "--short", "HEAD").strip(),
            "changelog": report.get("changelog", []), "notes": report.get("notes", [])}


def apply_after_merge(config_dir, choices, report, edited, merged, stash):
    """The update is in: put back the edits you keep, write the merged
    files, and update settings and shortcuts. Raises on any failure (apply
    then puts everything back)."""
    if stash:
        keep = [f for f in edited if choices.get("code:" + f, "mine") == "mine"]
        for f in keep:
            # Your version of the file, from the stash. A file you deleted (or
            # renamed away) isn't in it: then yours is "no file"
            if subprocess.run(["git", "-C", REPO, "cat-file", "-e", f"{stash}:{f}"], capture_output=True).returncode == 0:
                git("checkout", stash, "--", f)
            else:
                path = os.path.join(REPO, f)
                if os.path.lexists(path):
                    os.remove(path)
        if keep:
            git("reset", "--quiet")  # (back to unstaged edits, as before)
    for f, text in merged.items():
        with open(os.path.join(REPO, f), "w") as fh:
            fh.write(text)

    progress(35, "Updating settings and shortcuts")
    # Settings and shortcuts
    config_path = os.path.join(config_dir, "config.json")
    config = load_json(config_path, {})
    user_path = os.path.join(config_dir, "keybinds.json")
    user = load_json(user_path, {"version": 1, "keybinds": {}, "shortcuts": [], "gestures": {"changes": {}, "custom": []}})
    user.setdefault("keybinds", {})
    for c in report["conflicts"]:
        pick = choices[c["id"]]
        if c["kind"] == "setting" and pick == "new":
            set_path(config, c["title"], c["new"])
        elif c["kind"] == "setting" and pick == "keep":
            set_path(config, c["title"], c["old"])  # (written down, so it stays put)
        elif c["kind"] == "removed":
            if pick == "move" and c.get("target"):
                set_path(config, c["target"], c["value"])
            del_path(config, c["title"])
        elif c["kind"] == "preset" and pick == "keep":
            lens, frost, solid, tint = (c["old"] + [0.45])[:4]
            set_path(config, "appearance.glass", "custom")
            for k, v in (("glassLens", lens), ("glassFrost", frost), ("glassSolid", solid), ("glassTint", tint)):
                set_path(config, "appearance." + k, v)
        elif c["kind"] == "shortcut":
            bid = c["id"].split(":", 1)[1]
            if pick == "mine":
                user["keybinds"].setdefault(bid, {})["disabled"] = True
            else:
                y = c["yours"]
                if y["kind"] == "override":
                    user["keybinds"].get(y["id"], {}).pop("keys", None)
                    if not user["keybinds"].get(y["id"]):
                        user["keybinds"].pop(y["id"], None)
                else:
                    for s in user.get("shortcuts", []):
                        if s.get("id") == y["id"]:
                            s["disabled"] = True
    if config:
        write_json(config_path, config)
    write_json(user_path, user)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["check", "apply"])
    ap.add_argument("--config", required=True, help="the shell's config folder (config.json, keybinds.json)")
    ap.add_argument("--choices", help="apply: JSON file mapping conflict ids to choices")
    ap.add_argument("--no-fetch", action="store_true")
    ap.add_argument("--rebuild", default="", help="apply: also rebuild these (comma-separated: plugin,native,shaders)")
    a = ap.parse_args()
    try:
        if a.command == "check":
            out = check(a.config, fetch=not a.no_fetch)
        else:
            out = apply(a.config, load_json(a.choices, {}) if a.choices else {}, [r for r in a.rebuild.split(",") if r])
    except (RuntimeError, SystemExit) as e:
        out = {"error": str(e)}
    except Exception as e:  # anything unexpected still answers in JSON
        out = {"error": "The updater stopped: " + (str(e) or e.__class__.__name__)}
    print(json.dumps(out, indent=1))
    return 1 if out.get("error") else 0


if __name__ == "__main__":
    sys.exit(main())
