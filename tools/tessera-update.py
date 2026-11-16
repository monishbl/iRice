#!/usr/bin/env python3
"""Tessera updater: gets the latest Tessera from its git remote without changing
anyone's settings or shortcuts. Anything an update would change for you is a
conflict you decide, and nothing is applied until all of them are decided.

    tessera-update.py check  --config <dir>            -> JSON report on stdout
    tessera-update.py apply  --config <dir> --choices <file.json>
    tessera-update.py info   --config <dir>            -> channel, version, date (no download)
    tessera-update.py peek   --config <dir> [--mark]   -> is there a newer version? (the notice)

Channels (config.json's update.channel, or --channel): stable follows the
branch main, beta the branch beta. Until one is chosen the checkout's own
branch decides (main: stable, beta: beta, any other: that branch). Going to
another channel whose version lacks some of this copy's commits is going back
to an older version: the report says so ("downgrade", "leaving").

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
KEYMAP = "hypr/tessera/keymap.json"
OLD_KEYMAP = "hypr/irice/keymap.json"  # (where it was in a version from before iRice became Tessera)
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


# ── Channels ───────────────────────────────────────────────────────────────
# Which branch this copy follows (Settings → Software Update → Channel,
# config.json's update.channel). Every branch carries both shells (QML and
# Rust): the channel only decides which one to follow. Both always ask
# before installing (docs/releases.md).
CHANNELS = {"stable": "main", "beta": "beta"}
NAMES = {"stable": "Stable", "beta": "Beta", "custom": "Your branch"}
NOTICE_EVERY = 24 * 3600  # an update not installed is mentioned again a day later


def tracked():
    """The remote branch this checkout follows ("origin/main"), or ""."""
    r = subprocess.run(["git", "-C", REPO, "rev-parse", "--abbrev-ref", "@{u}"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else ""


def upstream():
    return tracked() or "origin/main"


def local_branch():
    """The branch checked out ("" when HEAD is detached)."""
    r = subprocess.run(["git", "-C", REPO, "symbolic-ref", "--quiet", "--short", "HEAD"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else ""


def configured_channel(config_dir):
    c = load_json(os.path.join(config_dir, "config.json"), {}) if config_dir else {}
    u = c.get("update") if isinstance(c, dict) else None
    return u.get("channel", "") if isinstance(u, dict) else ""


def follow(config_dir, channel=None):
    """(channel, remote, branch) this copy follows. A channel chosen in
    Settings wins; until one is chosen, the branch the checkout follows says
    (main: Stable, beta: Beta, anything else: that branch, "custom", so a
    developer's checkout keeps following its own branch)."""
    ch = channel if channel is not None else configured_channel(config_dir)
    up = tracked()
    remote, _, ub = up.partition("/") if up else ("origin", "", "")
    if ch in CHANNELS:
        return ch, remote, CHANNELS[ch]
    for k, b in CHANNELS.items():
        if ub == b:
            return k, remote, b
    if ub:
        return "custom", remote, ub
    return "stable", remote, CHANNELS["stable"]


def commit_time(rev):
    try:
        return int(git("log", "-1", "--format=%ct", rev).strip() or 0)
    except RuntimeError:
        return 0


def position(config_dir, fetch=True, channel=None):
    """Where this copy is against its channel: {"channel", "channelName",
    "branch", "upstream", "local", "current", "date", "latest",
    "latestDate", "behind", "ahead", "own", "kind", "available",
    "downgrade"}, with "error" when it can't tell or can't go there.
    kind: none | update (newer: taken as it is) | switch (the channel's
    version lacks some of this copy's commits, another channel's: going
    there is going back to an older version, "downgrade")."""
    ch, remote, branch = follow(config_dir, channel)
    target = remote + "/" + branch
    out = {"channel": ch, "channelName": NAMES[ch], "branch": branch, "upstream": target, "local": local_branch(),
           "current": git("rev-parse", "--short=8", "HEAD").strip(), "date": commit_time("HEAD"), "available": False}
    if fetch:
        # (only this branch: a copy cloned with one branch gets the other
        # channel's when it's chosen)
        r = subprocess.run(["git", "-C", REPO, "fetch", "--quiet", remote, f"+refs/heads/{branch}:refs/remotes/{target}"],
                           capture_output=True, text=True)
        if r.returncode != 0:
            why = (r.stderr.strip() or r.stdout.strip()).splitlines()
            if any("couldn't find remote ref" in l.lower() for l in why):
                return dict(out, error=f"The {NAMES[ch]} channel isn't published yet. Choose Stable for now.")
            return dict(out, error="Couldn't reach the update server: " + (why[-1] if why else "no answer").split(": ", 1)[-1])
    if subprocess.run(["git", "-C", REPO, "rev-parse", "--verify", "--quiet", target + "^{commit}"], capture_output=True).returncode != 0:
        return dict(out, error=f"This copy hasn't downloaded the {NAMES[ch]} channel yet. Check for updates while online.")
    new_id = git("rev-parse", target).strip()
    behind = int(git("rev-list", "--count", f"HEAD..{target}").strip() or 0)
    ahead = int(git("rev-list", "--count", f"{target}..HEAD").strip() or 0)
    # Commits here that no branch of the remote has: your own work
    own = int(git("rev-list", "--count", "HEAD", "--not", f"--remotes={remote}").strip() or 0) if ahead else 0
    kind = "none" if behind == 0 and ahead == 0 else "update" if ahead == 0 else "switch"
    out.update(latest=new_id[:8], latestDate=commit_time(target), behind=behind, ahead=ahead, own=own, kind=kind,
               available=kind != "none", downgrade=ahead > 0)
    if ahead and (own or ch == "custom"):
        out.update(available=False, error=f"This copy has {own or ahead} commit(s) of its own, so the update can't be applied on top automatically. Ask for help, or move your changes out first.")
    return out


def notice_path():
    base = os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state")
    return os.path.join(base, "tessera", "update-notice.json")


def peek(config_dir, channel=None, mark=False, now=None):
    """The background check (both shells, every few hours, while Software
    Update's "Check automatically" is on): is there a newer version on this
    copy's channel? It only fetches; nothing is installed. "notify" is true
    once per new version, and again a day later while it isn't installed
    (only for a newer version: going back to another channel is asked in
    Settings, never pushed). mark: write down that it was mentioned now."""
    now = time.time() if now is None else now
    if not is_checkout():
        return {"error": NOT_A_CHECKOUT, "notify": False}
    p = position(config_dir, True, channel)
    seen = load_json(notice_path(), {})
    seen = seen if isinstance(seen, dict) else {}
    try:
        due = float(seen.get("next", 0) or 0)
    except (TypeError, ValueError):
        due = 0
    fresh = p.get("latest") != seen.get("commit") or now >= due
    p["notify"] = bool(not p.get("error") and p.get("kind") == "update" and fresh)
    if p["notify"] and mark:
        try:
            os.makedirs(os.path.dirname(notice_path()), exist_ok=True)
            write_json(notice_path(), {"commit": p["latest"], "next": now + NOTICE_EVERY})
        except OSError:
            pass
    return p


# ── Update notes (CHANGELOG.md) ────────────────────────────────────────────
# "# Tessera changelog", then entries newest first: "## <Title> · <YYYY-MM-DD>"
# and markdown under it. An entry may end with <!-- packages: +a -b --> (the
# Tessera packages it adds / drops).
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


def is_checkout():
    return os.path.exists(os.path.join(REPO, ".git"))


NOT_A_CHECKOUT = "This copy of Tessera isn't a git checkout, so it can't update itself. Get it with the install command from irice.monishbl.in."


def check(config_dir, fetch=True, channel=None):
    """The full report: position() on this copy's channel, plus what's new
    (or, going back, what goes away), the conflicts to decide, and how
    tessera-ui comes with it."""
    if not is_checkout():
        return {"error": NOT_A_CHECKOUT}
    report = position(config_dir, fetch, channel)
    report.update(notes=[], conflicts=[], keeps=[], leaving=[])
    if "latest" not in report:
        return report
    base, new = "HEAD", report["upstream"]
    new_id = git("rev-parse", new).strip()
    # How tessera-ui comes with it ("tessera-ui: ready-made (v3)"; the line
    # Software Update shows): tools/native-install.py plan
    if fetch:
        report["ui"] = native_plan(new_id, report["channel"])
    if not report["available"]:
        return report
    report["notes"] = [l for l in git("log", "--format=%s", f"{base}..{new}").splitlines() if l][:30]
    # Going back to another channel's version: what goes away with it
    if report["downgrade"]:
        report["leaving"] = [l for l in git("log", "--format=%s", f"{new}..{base}").splitlines() if l][:30]
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
                                    "detail": "You have a file here that Tessera doesn't track, and the update adds one with the same name.",
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
    old_k, new_k = keymap(show(base, KEYMAP) or show(base, OLD_KEYMAP)), keymap(show(new, KEYMAP) or show(new, OLD_KEYMAP))
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

NATIVE = os.path.join(REPO, "tools", "native-install.py")


def native_plan(ref, channel=None):
    """{"label", "changes", …} for the version REF, or None. channel: which
    rolling release to look in first."""
    try:
        r = subprocess.run([sys.executable, "-I", NATIVE, "plan", "--ref", ref, "--online", "--json"] +
                           (["--channel", channel] if channel in CHANNELS else []),
                           capture_output=True, text=True, timeout=60)
        return json.loads(r.stdout) if r.returncode == 0 else None
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


def native_install(percent=40, *extra):
    """tessera-ui and tessera-events for the checkout as it is now: the
    ready-made build, the previous one when the code went back to it, or
    built here (rust.uiSource). Never fatal: on failure the installed ones
    stay, and the result says why."""
    try:
        p = subprocess.Popen([sys.executable, "-I", NATIVE, "install", "--json", "--progress", *extra],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        for line in p.stderr:
            if line.startswith("STEP "):
                progress(percent, line[5:].strip())
            else:
                print(line.rstrip(), file=sys.stderr)
        out = p.stdout.read()
        p.wait()
        return json.loads(out)
    except (OSError, ValueError) as e:
        return {"ok": False, "why": "tools/native-install.py didn't answer (" + str(e) + ")"}


def rebuild(todo, base=45, span=40):
    """Rebuilds parts (plugin, native, shaders). Failures are reported, not
    fatal: the shell falls back (frosted glass without the plugin)."""
    built, failed = [], []
    steps = {"plugin": ["make", "-C", os.path.join(REPO, "hypr/plugin/tessera-glass")],
             "native": [os.path.join(REPO, "shell/native/build.sh")],
             "shaders": [os.path.join(REPO, "tools/build-shaders.sh")]}
    names = {"plugin": "Building the glass plugin", "native": "Building native helpers", "shaders": "Building shaders"}
    for i, r in enumerate(todo):
        if r not in steps:
            continue
        progress(base + span * i / max(1, len(todo)), names[r])
        res = subprocess.run(steps[r], capture_output=True, text=True)
        (built if res.returncode == 0 else failed).append(r)
    return built, failed


def repo_lock():
    """One update at a time on this repository, from any session: the same
    lock update-run.sh takes (it says so with TESSERA_UPDATE_HELD=1, and then
    holds it for us). Kept open (held) until this process ends."""
    if os.environ.get("TESSERA_UPDATE_HELD") == "1":
        return None
    import fcntl
    f = open(os.path.join(REPO, ".git", "tessera-update.lock"), "w")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        f.close()
        raise SystemExit("Another update is running on this copy of Tessera. Wait for it to finish, then try again.")
    return f


def take(report):
    """Moves this copy to its channel's version, on the channel's branch
    (following it from now on). A newer version on the branch checked out
    is a fast-forward; anything else (another channel, going back to an
    older version) checks the branch out at that version: the commits it
    leaves stay on the other channel's branch. Returns how to undo it."""
    target = report["upstream"]
    branch = report.get("branch") or ""
    here = local_branch()
    undo = {"here": here, "branch": branch, "had": None, "upstream": None}
    if report.get("channel") == "custom" or not branch or (branch == here and report.get("kind", "update") == "update"):
        git("merge", "--ff-only", target)
        return undo
    undo["had"] = git("rev-parse", "--verify", "--quiet", "refs/heads/" + branch, check=False).strip() or None
    undo["upstream"] = git("rev-parse", "--abbrev-ref", branch + "@{u}", check=False).strip() or None if undo["had"] else None
    undo["moved"] = True
    git("checkout", "--quiet", "-B", branch, target)
    # Following it: the branch's remote and name (set directly: a copy
    # cloned with one branch doesn't count the other as a remote branch),
    # and the remote's fetch list gets it, so a plain git pull works too
    remote = target.split("/", 1)[0]
    spec = f"+refs/heads/{branch}:refs/remotes/{target}"
    specs = git("config", "--get-all", f"remote.{remote}.fetch", check=False).split()
    if spec not in specs and f"+refs/heads/*:refs/remotes/{remote}/*" not in specs:
        git("config", "--add", f"remote.{remote}.fetch", spec)
    git("config", f"branch.{branch}.remote", remote)
    git("config", f"branch.{branch}.merge", "refs/heads/" + branch)
    return undo


def untake(undo, old_head):
    """take() undone: the branch that was checked out, at OLD_HEAD."""
    # (on another branch now: back to the one before, and this one as it was)
    if undo and undo.get("moved") and undo["branch"] != undo.get("here"):
        git("checkout", "--quiet", "--force", undo["here"] or old_head)
        if undo.get("had"):
            git("update-ref", "refs/heads/" + undo["branch"], undo["had"])
            r, _, b = (undo.get("upstream") or "").partition("/")
            if b:
                git("config", f"branch.{undo['branch']}.remote", r, check=False)
                git("config", f"branch.{undo['branch']}.merge", "refs/heads/" + b, check=False)
        else:
            git("branch", "--quiet", "-D", undo["branch"], check=False)
    git("reset", "--hard", "--quiet", old_head)


def apply(config_dir, choices, extra=(), channel=None):
    """extra: parts to rebuild whatever the update says (the glass plugin
    after a Hyprland update); with no Tessera update, only those are rebuilt."""
    lock = repo_lock()  # noqa: F841 (held while this runs)
    report = check(config_dir, fetch=False, channel=channel)
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
    ui = None
    undo = None
    try:
        for f in untracked:
            dest = os.path.join(aside, f)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            shutil.move(os.path.join(REPO, f), dest)
        if edited:
            git("stash", "push", "-m", "tessera-update " + stamp)
            stash = git("rev-parse", "stash@{0}").strip()
        undo = take(report)
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
        # tessera-ui for the new version, with it: the one it replaces is
        # kept as tessera-ui.prev (going back below swaps it back)
        ui = native_install(40)
    except BaseException as e:
        # Back to exactly how it was: the old version, then your edits, then
        # your settings (from the backup made above)
        why = str(e) or e.__class__.__name__
        restored = ""
        try:
            untake(undo, old_head)
            # (the build for the old version back too, when it was replaced:
            # install --offline finds it as .prev, no download or build)
            if ui is not None and ui.get("changed"):
                native_install(85, "--offline", "--only-prev")
            if stash:
                git("stash", "pop")
            put_back_untracked()
            for name in ("config.json", "keybinds.json"):
                if os.path.exists(os.path.join(backup, name)):
                    shutil.copy2(os.path.join(backup, name), os.path.join(config_dir, name))
            restored = " Nothing was changed: Tessera, your edits and your settings are as they were."
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
    return {"applied": True, "backup": backup, "stash": stash, "built": built, "failed": failed, "ui": ui,
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
    ap.add_argument("command", choices=["check", "apply", "info", "peek"])
    ap.add_argument("--channel", choices=sorted(CHANNELS), help="this channel instead of config.json's update.channel")
    ap.add_argument("--mark", action="store_true", help="peek: write down that a new version was mentioned")
    ap.add_argument("--config", required=True, help="the shell's config folder (config.json, keybinds.json)")
    ap.add_argument("--choices", help="apply: JSON file mapping conflict ids to choices")
    ap.add_argument("--no-fetch", action="store_true")
    ap.add_argument("--rebuild", default="", help="apply: also rebuild these (comma-separated: plugin,native,shaders)")
    a = ap.parse_args()
    try:
        if a.command == "check":
            out = check(a.config, fetch=not a.no_fetch, channel=a.channel)
        elif a.command == "info":
            out = position(a.config, False, a.channel) if is_checkout() else {"error": NOT_A_CHECKOUT}
        elif a.command == "peek":
            out = peek(a.config, a.channel, a.mark)
        else:
            out = apply(a.config, load_json(a.choices, {}) if a.choices else {}, [r for r in a.rebuild.split(",") if r], a.channel)
    except (RuntimeError, SystemExit) as e:
        out = {"error": str(e)}
    except Exception as e:  # anything unexpected still answers in JSON
        out = {"error": "The updater stopped: " + (str(e) or e.__class__.__name__)}
    print(json.dumps(out, indent=1))
    return 1 if out.get("error") else 0


if __name__ == "__main__":
    sys.exit(main())
