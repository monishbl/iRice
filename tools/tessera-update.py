#!/usr/bin/env python3
"""Tessera updater: gets the latest Tessera from its git remote without changing
anyone's settings or shortcuts. Anything an update would change for you is a
conflict you decide, and nothing is applied until all of them are decided.

    tessera-update.py check  --config <dir>            -> JSON report on stdout
    tessera-update.py apply  --config <dir> --choices <file.json>
    tessera-update.py info   --config <dir>            -> channel, version, date (no download)
    tessera-update.py peek   --config <dir> [--mark]   -> is there a newer version? (the notice)
    tessera-update.py fetch  --config <dir>            -> download the followed branch (tessera update)
    tessera-update.py stage  --config <dir> --choices-json <json> [--rebuild plugin]
                                                       -> the new version made ready in the background
    tessera-update.py finish --config <dir> [--if-ready] -> install what stage made ready (the restart)
    tessera-update.py staged | discard | cancel        -> what's staged; forget it; stop the stage job

Background updates (Software Update's Update, docs/releases.md): `stage`
runs with the shell up and changes nothing it uses. It takes the repo
lock, checks again, gets tessera-ui and tessera-events for the new version
(only those whose sources change; a signed patch when there is one) into
their folder's .staged/, builds any part whose sources change (the glass
plugin, shaders, native helpers) from a staging checkout of the new version
($XDG_CACHE_HOME/tessera/stage-src, a git worktree), and writes
$XDG_STATE_HOME/tessera/update-staged/staged.json: the commit, your choices,
what's ready. Its progress is $XDG_RUNTIME_DIR/tessera-stage/status.json.
`finish` (the shell stopped: update-run.sh finish, or launch-shell.sh at
the next login) takes exactly that commit, as `apply` does, swapping in what
was staged: renames and copies, a few seconds. A conflict that appeared
since (a setting changed) settles keeping yours; a file changed since stops
it, and nothing changes.

Downloads come from the checkout's remote, else the other sources in
tools/sources.txt (GitHub, its GitLab mirror), fetched by URL: the remote
isn't rewritten, and a mirror that doesn't answer is passed over in seconds.

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
import hashlib
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
MIGRATIONS = "tools/settings-migrations.json"  # {"old.path": "new.path"}


def git(*args, check=True):
    r = subprocess.run(["git", "-C", REPO, *args], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise RuntimeError("git " + " ".join(args) + ": " + (r.stderr.strip() or r.stdout.strip()))
    return r.stdout


def _load_sources():
    # (tools/sources.txt, through tools/tessera_sources.py: loaded by its path,
    # since python -I leaves tools/ off sys.path)
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        "tessera_sources", os.path.join(os.path.dirname(os.path.abspath(__file__)), "tessera_sources.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


SOURCES = _load_sources()


def _load_old_names():
    # (Tessera's old name: shell/assets/scripts/migrate-names.py, the one place
    # that knows it, by its path as above)
    import importlib.util
    spec = importlib.util.spec_from_file_location("migrate_names", os.path.join(REPO, "shell", "assets", "scripts", "migrate-names.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


try:
    OLD_NAMES = _load_old_names()
except Exception:  # (gone after its removal date, or a copy without it)
    OLD_NAMES = None
OLD_KEYMAP = getattr(OLD_NAMES, "OLD_KEYMAP", None)  # (where the shortcuts were in a version from before the rename)


def fetch_branch(remote, branch, target):
    """BRANCH into refs/remotes/TARGET: from REMOTE first, then, when REMOTE
    is one of Tessera's own sources (tools/sources.txt; or TESSERA_MIRRORS is
    set), from each other one in turn, by URL: the remote itself is never
    rewritten. A source that doesn't answer within seconds, or doesn't exist
    yet (a GitLab mirror not created), is passed over quietly.
    Returns {"ok", "source", "missing", "why"}: missing when a source that
    answered has no such branch."""
    spec = f"+refs/heads/{branch}:refs/remotes/{target}"
    url = git("remote", "get-url", remote, check=False).strip()
    official = SOURCES.is_official(url)
    tries = [(remote, url)]
    if official or os.environ.get("TESSERA_MIRRORS", "").strip():
        tries += [(u, u) for u in SOURCES.git_sources() if not SOURCES.same_repo(u, url)]
    env = dict(os.environ, **SOURCES.QUIET_GIT)
    why, missing = [], False
    for i, (src, shown) in enumerate(tries):
        if (i or official) and not SOURCES.reachable(shown):
            why.append(f"{shown}: didn't answer")
            continue
        try:
            r = subprocess.run(["git", "-C", REPO, "fetch", "--quiet", src, spec], capture_output=True, text=True,
                               env=env, timeout=900)
        except subprocess.TimeoutExpired:
            why.append(f"{shown}: took too long")
            continue
        if r.returncode == 0:
            return {"ok": True, "source": shown, "missing": False, "why": []}
        lines = (r.stderr.strip() or r.stdout.strip()).splitlines()
        if any("couldn't find remote ref" in l.lower() for l in lines):
            missing = True
            if i == 0:
                break  # (the branch's own home says it isn't there: no mirror has it either)
        why += lines[-1:] or [f"{shown}: no answer"]
    return {"ok": False, "source": None, "missing": missing, "why": why}


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


def position(config_dir, fetch=True, channel=None, pin=None):
    """Where this copy is against its channel: {"channel", "channelName",
    "branch", "upstream", "local", "current", "date", "latest",
    "latestDate", "behind", "ahead", "own", "kind", "available",
    "downgrade", "target"}, with "error" when it can't tell or can't go there.
    pin: that exact commit instead of the branch's latest (the restart after
    a staged update installs exactly what was staged).
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
        # (from the remote, else Tessera's mirrors: tools/sources.txt)
        f = fetch_branch(remote, branch, target)
        if not f["ok"]:
            if f["missing"]:
                return dict(out, error=f"The {NAMES[ch]} channel isn't published yet. Choose Stable for now.")
            why = f["why"]
            return dict(out, error="Couldn't reach the update server: " + (why[-1] if why else "no answer").split(": ", 1)[-1])
        out["source"] = f["source"]
    ref = pin or target
    if subprocess.run(["git", "-C", REPO, "rev-parse", "--verify", "--quiet", ref + "^{commit}"], capture_output=True).returncode != 0:
        return dict(out, error=f"This copy hasn't downloaded the {NAMES[ch]} channel yet. Check for updates while online.")
    new_id = git("rev-parse", ref + "^{commit}").strip()
    out["target"] = new_id
    behind = int(git("rev-list", "--count", f"HEAD..{new_id}").strip() or 0)
    ahead = int(git("rev-list", "--count", f"{new_id}..HEAD").strip() or 0)
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


NOT_A_CHECKOUT = "This copy of Tessera isn't a git checkout, so it can't update itself. Get it with the install command from tessera.monishbl.in."


def check(config_dir, fetch=True, channel=None, pin=None):
    """The full report: position() on this copy's channel, plus what's new
    (or, going back, what goes away), the conflicts to decide, and how
    tessera-ui comes with it."""
    if not is_checkout():
        return {"error": NOT_A_CHECKOUT}
    report = position(config_dir, fetch, channel, pin)
    report.update(notes=[], conflicts=[], keeps=[], leaving=[])
    if "latest" not in report:
        return report
    base, new = "HEAD", report["target"]
    new_id = new
    # How tessera-ui comes with it ("tessera-ui: ready-made (v3)"; the line
    # Software Update shows): tools/native-install.py plan
    if fetch:
        report["ui"] = native_plan(new_id, report["channel"])
        wait = preparing(report["ui"], new_id) if report["available"] else None
        if wait:
            report["available"] = False
            report["preparing"] = wait
    # A version whose shell didn't start here (launch-shell.sh went back to
    # the build before it) isn't offered again; a newer one is
    if report["available"] and not pin and is_bad(new_id):
        report["available"] = False
        report["bad"] = True
        report["error"] = "This version of Tessera didn't start on this computer, so it went back to the one before. A newer one is offered when it's out."
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
    old_k, new_k = keymap(show(base, KEYMAP) or (OLD_KEYMAP and show(base, OLD_KEYMAP))), keymap(show(new, KEYMAP) or (OLD_KEYMAP and show(new, OLD_KEYMAP)))
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

    # What gets rebuilt after it's applied: each part whose own sources
    # change (part_key), or whose build isn't there
    report["rebuild"] = parts_needed(new)
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


# A version only minutes old has no ready-made tessera-ui yet: server1 builds
# and signs it after every push (about 45 minutes). Offered meanwhile, the
# update compiled the shell here instead (19 Nov: 50 minutes on a laptop in
# Power Saver, from an update started 25 minutes before the download was
# out). So such a version waits; after this long it's offered anyway, built
# here if it must be.
PREPARE_WAIT = 3 * 3600
PREPARE_TYPICAL = 45 * 60


def is_bad(rev):
    """Is REV's build one that didn't start here (native-install.py's
    bad-builds.json, by its native/ tree)?"""
    tree = git("rev-parse", "--verify", "--quiet", rev + ":native", check=False).strip()
    d = load_json(os.path.join(state_dir(), "bad-builds.json"), {})
    return bool(tree) and isinstance(d, dict) and tree in (d.get("trees") or [])


def preparing(ui, rev, now=None):
    """Minutes until the ready-made build is likely out (at least 5), when
    the version REV has none yet only because it's new; else None."""
    if not isinstance(ui, dict) or ui.get("available") is not False:
        return None
    if "no ready-made build for it yet" not in str(ui.get("label", "")):
        return None
    age = (now or time.time()) - commit_time(rev)
    if not commit_time(rev) or age < 0 or age >= PREPARE_WAIT:
        return None
    return max(5, int((PREPARE_TYPICAL - age) // 60) + 1)


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
    # (the plugin through build-progress.py: its phase and log for the update screen)
    # (the plugin through build-plugin.sh: built and put where Hyprland loads
    # it at login, beside the loaded one, never over it; it went only into
    # the checkout before, so an installed session kept the stale one)
    steps = {"plugin": ["bash", os.path.join(REPO, "tools", "build-plugin.sh")],
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


def repo_lock(wait=False):
    """One update at a time on this repository, from any session: the same
    lock update-run.sh takes (it says so with TESSERA_UPDATE_HELD=1, and then
    holds it for us). Kept open (held) until this process ends."""
    global _HELD
    if os.environ.get("TESSERA_UPDATE_HELD") == "1" or (_HELD is not None and _HELD() is not None):
        return None  # (held already: by update-run.sh, or by our caller: finish → apply)
    import fcntl
    import weakref
    f = open(os.path.join(REPO, ".git", "tessera-update.lock"), "w")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | (0 if wait else fcntl.LOCK_NB))
    except BlockingIOError:
        f.close()
        raise SystemExit("Another update is running on this copy of Tessera. Wait for it to finish, then try again.")
    held = _Held(f)
    _HELD = weakref.ref(held)
    return held


_HELD = None  # (a weak reference to the lock this process holds)


class _Held:
    """The repo lock, held while its owner keeps this (released when it's
    dropped: the end of the function that took it)"""

    def __init__(self, f):
        self.f = f

    def __del__(self):
        try:
            self.f.close()
        except Exception:
            pass


def take(report):
    """Moves this copy to its channel's version, on the channel's branch
    (following it from now on). A newer version on the branch checked out
    is a fast-forward; anything else (another channel, going back to an
    older version) checks the branch out at that version: the commits it
    leaves stay on the other channel's branch. Returns how to undo it."""
    target = report.get("target") or report["upstream"]
    upstream = report["upstream"]
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
    remote = upstream.split("/", 1)[0]
    spec = f"+refs/heads/{branch}:refs/remotes/{upstream}"
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


def apply(config_dir, choices, extra=(), channel=None, staged=None):
    """extra: parts to rebuild whatever the update says (the glass plugin
    after a Hyprland update); with no Tessera update, only those are rebuilt.
    staged: a staged update (stage()): exactly its version is taken, its
    programs and parts swapped in (nothing is downloaded or built: the
    shell's restart takes seconds)."""
    lock = repo_lock()  # noqa: F841 (held while this runs)
    report = check(config_dir, fetch=False, channel=channel, pin=staged["to"] if staged else None)
    if report.get("error"):
        raise SystemExit(report["error"])
    if staged:
        if not report["available"]:
            raise SystemExit("The prepared update no longer applies to this copy. Check for updates again.")
        choices = settled_choices(report, staged.get("choices") or {})
    # (a system update started meanwhile: Tessera's part waits for its
    # ready-made build rather than compiling here)
    elif report["available"] and not report.get("downgrade"):
        upstream = git("rev-parse", report["upstream"]).strip()
        if preparing(native_plan(upstream, report.get("channel")), upstream):
            report["available"] = False
            report["preparing"] = True
    if not report["available"]:
        if not extra:
            return {"applied": False, "message": "Tessera's new version is still being prepared; it's offered once it's ready." if report.get("preparing") else "Already up to date."}
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
    # (written before anything changes, and as each step is done: an update
    # cut off anywhere, power off, a crash, is put back at the next start:
    # recover)
    journal = Journal(config_dir, backup)
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
    journal.write(old_head=old_head, here=local_branch(), untracked=untracked, aside=aside)
    checkpoint("backup")

    def put_back_untracked():
        for f in untracked:
            src, dst = os.path.join(aside, f), os.path.join(REPO, f)
            if os.path.lexists(src):
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copy2(src, dst)
    merged = {}  # files kept as both: your edits with the update's merged in
    for f in edited:
        if choices.get("code:" + f) == "both":
            text = three_way(f, report.get("target") or report["upstream"])
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
        journal.write(step="stashed", stash=stash)
        checkpoint("stashed")
        journal.write(step="taking")
        undo = take(report)
        journal.write(step="taken", undo=undo)
        checkpoint("taken")
    except BaseException as e0:
        if isinstance(e0, Cut):
            raise  # (a power cut: nothing runs after it; recover() puts it back)
        try:
            if stash:
                git("stash", "pop")
        finally:
            put_back_untracked()
        raise
    try:
        apply_after_merge(config_dir, choices, report, edited, merged, stash)
        journal.write(step="settings")
        checkpoint("settings")
        # Untracked files you keep: yours over the update's
        for f in untracked:
            if choices.get("code:" + f) == "mine":
                shutil.copy2(os.path.join(aside, f), os.path.join(REPO, f))
        # tessera-ui for the new version, with it: the one it replaces is
        # kept as tessera-ui.prev (going back below swaps it back)
        if staged:
            # (the programs staged for exactly this version: renames only)
            progress(60, "Installing the new programs")
            ui = native_install(60, "--staged")
            if not ui.get("ok"):
                raise RuntimeError("the prepared programs couldn't be installed (" + (ui.get("why") or "no answer") + ")")
        else:
            ui = native_install(40)
        journal.write(step="programs", ui_changed=bool(ui and ui.get("changed")))
        checkpoint("programs")
    except BaseException as e:
        if isinstance(e, Cut):
            raise
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
        journal.done()
        raise SystemExit("The update didn't finish: " + why + "." + restored)
    if staged:
        built, failed = install_parts(staged)
        checkpoint("parts")
    else:
        todo = list(report.get("rebuild", []))
        todo += [r for r in extra if r not in todo]
        built, failed = rebuild(todo)
        remember_parts(built, "HEAD")
    progress(85, "Finishing up")
    # What this copy still keeps under Tessera's old name moves over now (its
    # folder too, to ~/tessera, the old name left as a link; never fatal):
    # the shell's restart after the update does it too, for an update run by
    # an older copy (migrate-names.py)
    try:
        subprocess.run([sys.executable, "-I", os.path.join(REPO, "shell", "assets", "scripts", "migrate-names.py"), "--checkout", REPO],
                       capture_output=True, timeout=120)
    except (OSError, subprocess.SubprocessError):
        pass
    journal.done()
    # (a new shell from this update that doesn't stay up goes back to the
    # build before it: launch-shell.sh's rollback_ui; tessera-ui removes the
    # mark once it has been up a minute)
    if ui and ui.get("changed"):
        try:
            write_json(fresh_path(), {"to": git("rev-parse", "HEAD").strip(), "from": old_head, "at": int(time.time())})
        except OSError:
            pass
    # (the notes, for the new shell's "What's new")
    return {"applied": True, "backup": backup, "stash": stash, "built": built, "failed": failed, "ui": ui,
            "now": git("rev-parse", "--short", "HEAD").strip(),
            "changelog": report.get("changelog", []), "notes": report.get("notes", [])}


def journal_path():
    return os.path.join(state_dir(), "update-journal.json")


def fresh_path():
    return os.path.join(state_dir(), "fresh-update.json")


class Journal:
    """What an update in progress has done so far, on disk: recover() puts
    it back if the update never finished (the computer went off, the
    process was killed). Each write replaces the file whole."""

    def __init__(self, config_dir, backup):
        self.d = {"step": "started", "config": config_dir, "backup": backup, "at": int(time.time()), "pid": os.getpid(),
                  "repo": os.path.realpath(REPO)}
        self.write()

    def write(self, **kw):
        self.d.update(kw)
        os.makedirs(state_dir(), exist_ok=True)
        write_json(journal_path(), self.d)

    def done(self):
        try:
            os.remove(journal_path())
        except OSError:
            pass


class Cut(BaseException):
    """Tests: an update cut off here, as by a power cut (no cleanup runs)"""


def checkpoint(name):
    """TESSERA_UPDATE_CRASH_AT=<name> ends the process right here, as a power
    cut would (nothing after it runs, not even cleanup); in-process tests
    replace this with one that raises Cut"""
    if os.environ.get("TESSERA_UPDATE_CRASH_AT") == name:
        sys.stderr.flush()
        os._exit(9)


STEPS = ("started", "stashed", "taking", "taken", "settings", "programs")


def _alive(pid):
    return bool(pid) and os.path.exists("/proc/" + str(pid))


def recover():
    """At the next start after an update that was cut off (launch-shell.sh,
    before any shell; and the shell, if one starts anyway): everything back
    exactly as it was before it, from its journal: the version, your edits
    (stashed), files set aside, your settings, and the programs. Never a
    half state: the checkout and tessera-ui are of the same version again.
    A staged update stays staged (it's finished again right after, at a
    login). Returns {"recovered": bool, ...}."""
    j = load_json(journal_path(), None)
    # (another checkout's journal: never ours to put back)
    if isinstance(j, dict) and j.get("repo") and j["repo"] != os.path.realpath(REPO):
        return {"recovered": False, "other": j["repo"]}
    if not isinstance(j, dict) or not j.get("old_head"):
        if isinstance(j, dict) and not _alive(j.get("pid")):  # (cut off before anything changed)
            try:
                os.remove(journal_path())
            except OSError:
                pass
        return {"recovered": False}
    try:
        lock = repo_lock(wait=False)  # noqa: F841 (an update still running holds it: not ours to undo)
    except SystemExit:
        return {"recovered": False, "busy": True}
    step = j.get("step", "started")
    at = STEPS.index(step) if step in STEPS else 0
    old_head = j["old_head"]
    if at >= STEPS.index("taking"):
        undo = j.get("undo") or {"here": j.get("here", ""), "branch": j.get("here", ""), "moved": False}
        if not j.get("undo") and j.get("here") and local_branch() != j.get("here"):
            undo = {"here": j["here"], "branch": local_branch(), "had": None, "moved": True}
        untake(undo, old_head)
    # (the programs of the old version back, when the new ones went in)
    if at >= STEPS.index("taken"):
        subprocess.run([sys.executable, "-I", NATIVE, "install", "--offline", "--only-prev", "--json"], capture_output=True, timeout=600)
    if j.get("stash") and at >= STEPS.index("stashed"):
        r = subprocess.run(["git", "-C", REPO, "stash", "apply", "--index", j["stash"]], capture_output=True, text=True)
        if r.returncode == 0:
            ref = next((l.split()[0] for l in git("stash", "list", "--format=%H %gd", check=False).splitlines()
                        if l.split()[0] == j["stash"]), None)
            idx = next((l.split()[1] for l in git("stash", "list", "--format=%H %gd", check=False).splitlines()
                        if l.split()[0] == j["stash"]), None)
            if ref and idx:
                git("stash", "drop", "--quiet", idx, check=False)
    for f in j.get("untracked") or []:
        src, dst = os.path.join(j.get("aside", ""), f), os.path.join(REPO, f)
        if j.get("aside") and os.path.lexists(src):
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copy2(src, dst)
    if at >= STEPS.index("taken") and j.get("config") and j.get("backup"):
        for name in ("config.json", "keybinds.json"):
            if os.path.exists(os.path.join(j["backup"], name)):
                shutil.copy2(os.path.join(j["backup"], name), os.path.join(j["config"], name))
    try:
        os.remove(journal_path())
    except OSError:
        pass
    return {"recovered": True, "step": step, "now": git("rev-parse", "--short", "HEAD").strip(),
            "error": "The last update was cut off, so Tessera went back to how it was before it. Update again when you're ready."}


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


# ── Parts: what each of Tessera's other builds is made from ─────────────────
# The glass plugin (hypr/plugin/tessera-glass, against Hyprland's headers),
# the shaders (*.frag, *.vert to .qsb) and the native helpers (shell/native,
# against Qt): each is built again only when its own sources (or what it's
# built against) change between the installed version and the new one, or
# its build isn't there. What each was last built from is kept in
# $XDG_STATE_HOME/tessera/parts.json.

PARTS = ("plugin", "shaders", "native")
PART_NAMES = {"plugin": "the glass plugin", "shaders": "the shaders", "native": "the native helpers"}


def _xdg(var, default):
    return os.environ.get(var) or os.path.expanduser(default)


def state_dir():
    return os.path.join(_xdg("XDG_STATE_HOME", "~/.local/state"), "tessera")


def data_dir():
    return os.path.join(_xdg("XDG_DATA_HOME", "~/.local/share"), "tessera")


def runtime_dir():
    return os.environ.get("XDG_RUNTIME_DIR") or "/tmp"


def staged_dir():
    """The staged update (persists across a reboot: it's finished at the next login)"""
    return os.path.join(state_dir(), "update-staged")


def job_dir():
    """The background job's status, log, pid and what it was asked
    (request.json): kept across a reboot, so a job cut off is resumed"""
    return os.path.join(state_dir(), "update-job")


def stage_source_dir():
    """The staging checkout of the new version: built from when something
    must be built here. Kept between updates (one folder, at one place), so
    only what changed is rebuilt"""
    return os.path.join(_xdg("XDG_CACHE_HOME", "~/.cache"), "tessera", "stage-src")


def _out(cmd, timeout=30):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return r.stdout.strip() if r.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def hyprland_headers():
    return _out(["pkg-config", "--modversion", "hyprland"])


def qt_version():
    return _out(["/usr/lib/qt6/bin/qtpaths", "--qt-version"]) or _out(["qmake6", "-query", "QT_VERSION"])


SHADER = re.compile(r"\.(frag|vert)$")


def part_key(part, rev, headers=None, qt=None):
    """What PART at REV is built from (None: it isn't in that version)"""
    if part == "plugin":
        t = git("rev-parse", "--verify", "--quiet", rev + ":hypr/plugin/tessera-glass", check=False).strip()
        return t and t + ":" + (hyprland_headers() if headers is None else headers)
    if part == "native":
        t = git("rev-parse", "--verify", "--quiet", rev + ":shell/native", check=False).strip()
        return t and t + ":" + (qt_version() if qt is None else qt)
    if part == "shaders":
        lines = [l for l in git("ls-tree", "-r", rev, "--", "shell", "material", check=False).splitlines() if SHADER.search(l)]
        return hashlib.sha1("\n".join(sorted(lines)).encode()).hexdigest() if lines else None
    return None


def parts_path():
    return os.path.join(state_dir(), "parts.json")


def parts_record():
    r = load_json(parts_path(), {})
    return r if isinstance(r, dict) else {}


def remember_parts(parts, rev, keys=None):
    """PARTS were built from REV (or KEYS, when given)"""
    if not parts:
        return
    rec = parts_record()
    for p in parts:
        k = (keys or {}).get(p) or part_key(p, rev)
        if k:
            rec[p] = k
    try:
        os.makedirs(state_dir(), exist_ok=True)
        write_json(parts_path(), rec)
    except OSError:
        pass


def part_built(part):
    """Is PART's build there (as the session uses it)?"""
    if part == "plugin":
        return os.path.isfile(os.path.join(data_dir(), "tessera-glass.so"))
    if part == "native":
        return os.path.isdir(os.path.join(REPO, "shell", "native", "build", "qml", "Tessera", "Native"))
    if part == "shaders":
        for top in ("shell", "material"):
            for d, _, files in os.walk(os.path.join(REPO, top)):
                for f in files:
                    if SHADER.search(f) and not os.path.exists(os.path.join(d, f + ".qsb")):
                        return False
        return True
    return False


def part_buildable(part):
    """Can PART be built here at all (its compiler and headers are there)?
    One that can't is left alone instead of failing every update"""
    if part == "plugin":
        return bool(hyprland_headers())
    if part == "shaders":
        return os.path.exists("/usr/lib/qt6/bin/qsb") or bool(shutil.which("distrobox"))
    if part == "native":
        return bool(shutil.which("cmake")) and bool(qt_version())
    return False


def parts_needed(rev, record=None, built=None, buildable=None, keys_now=None):
    """The parts to build for REV: those whose source key changes from what
    the installed build was made from, or whose build isn't there"""
    record = parts_record() if record is None else record
    built = built or part_built
    buildable = buildable or part_buildable
    out = []
    for p in PARTS:
        new = part_key(p, rev)
        if not new or not buildable(p):
            continue
        if not built(p):
            out.append(p)
            continue
        old = record.get(p)
        if old is None:
            # (built before records were kept: the version installed now is
            # what it was built from; the plugin says its Hyprland itself)
            if p == "plugin":
                headers = _read_text(os.path.join(data_dir(), "tessera-glass.built-for")).strip()
                old = part_key(p, "HEAD", headers=headers or None)
            else:
                old = part_key(p, "HEAD") if keys_now is None else keys_now.get(p)
        if old != new:
            out.append(p)
    return out


def _read_text(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return ""


# ── The background job ─────────────────────────────────────────────────────
# Software Update's Update starts `stage` on its own (setsid): with the
# shell running, it gets the new version ready beside the installed one.
# Its status, for the shell, in $XDG_RUNTIME_DIR/tessera-stage/status.json:
#   {"state": running | ready | done | error | cancelled | none, "phase":
#    prepare | download | build | parts | ready, "percent", "text",
#    "detail", "how" (patch | full | build | prev | none), "bytes",
#    "latest", "current", "error", "started", "finished", "pid"}

JOB = None


class Job:
    def __init__(self, directory=None):
        self.dir = directory or job_dir()
        os.makedirs(self.dir, exist_ok=True)
        self.state = {"state": "running", "phase": "prepare", "percent": 0, "text": "Preparing", "detail": "", "how": "",
                      "bytes": 0, "error": "", "started": time.time(), "finished": 0, "pid": os.getpid()}
        with open(os.path.join(self.dir, "pid"), "w") as f:
            f.write(str(os.getpid()))
        self.write()

    def write(self, **kw):
        self.state.update(kw)
        tmp = os.path.join(self.dir, "status.json.tmp")
        try:
            with open(tmp, "w") as f:
                json.dump(self.state, f)
            os.replace(tmp, os.path.join(self.dir, "status.json"))
        except OSError:
            pass

    def log(self, line):
        try:
            with open(os.path.join(self.dir, "log"), "a") as f:
                f.write(line.rstrip("\n") + "\n")
        except OSError:
            pass

    def end(self, state, **kw):
        self.write(state=state, finished=time.time(), **kw)
        try:
            os.remove(os.path.join(self.dir, "pid"))
        except OSError:
            pass


def job_progress(percent, text, phase=None, **kw):
    """A step of the background job (and the PROGRESS line, as ever)"""
    progress(percent, text)
    if JOB:
        JOB.write(percent=int(percent), text=text, **({"phase": phase} if phase else {}), **kw)


def step_phase(text):
    """The job's phase for one of native-install.py's steps"""
    t = text.lower()
    if t.startswith("downloading"):
        return "download"
    if t.startswith("building"):
        return "build"
    return None


MB = re.compile(r"([0-9.]+) MB")


def stage_source(rev):
    """The staging checkout at REV (a git worktree of this repository, only
    the folders builds need), made or moved there"""
    wt = stage_source_dir()
    if not os.path.exists(os.path.join(wt, ".git")):
        shutil.rmtree(wt, ignore_errors=True)
        git("worktree", "prune", check=False)
        os.makedirs(os.path.dirname(wt), exist_ok=True)
        git("worktree", "add", "--detach", "--no-checkout", wt, rev)
        subprocess.run(["git", "-C", wt, "sparse-checkout", "set", "--cone", "native", "hypr", "shell", "material", "tools"],
                       capture_output=True, text=True)
    r = subprocess.run(["git", "-C", wt, "checkout", "--quiet", "--detach", "--force", rev], capture_output=True, text=True)
    if r.returncode != 0:
        # (a staging checkout git no longer knows: made again)
        shutil.rmtree(wt, ignore_errors=True)
        git("worktree", "prune", check=False)
        git("worktree", "add", "--detach", "--no-checkout", wt, rev)
        subprocess.run(["git", "-C", wt, "sparse-checkout", "set", "--cone", "native", "hypr", "shell", "material", "tools"],
                       capture_output=True, text=True)
        git_wt = subprocess.run(["git", "-C", wt, "checkout", "--quiet", "--detach", "--force", rev], capture_output=True, text=True)
        if git_wt.returncode != 0:
            raise RuntimeError("the staging checkout couldn't be made: " + (git_wt.stderr.strip() or "git checkout failed"))
    return wt


def native_stage(rev, source, lo=8, hi=78):
    """tools/native-install.py stage: tessera-ui and tessera-events for REV
    ready beside the installed ones (only the ones whose sources change; a
    patch when there is one). Returns its JSON."""
    try:
        p = subprocess.Popen([sys.executable, "-I", NATIVE, "stage", "--ref", rev, "--source", source, "--json", "--progress"],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        for line in p.stderr:
            line = line.rstrip("\n")
            if JOB:
                JOB.log(line)
            if line.startswith("STEP "):
                text = line[5:].strip()
                phase = step_phase(text)
                m = MB.search(text)
                extra = {}
                if phase == "download":
                    extra = {"how": "patch" if "patch" in text else "full", "bytes": int(float(m.group(1)) * 1e6) if m else 0}
                elif phase == "build":
                    extra = {"how": "build"}
                job_progress(lo + (5 if phase == "download" else 10 if phase == "build" else 0), text, phase, **extra)
            else:
                print(line, file=sys.stderr)
        out = p.stdout.read()
        p.wait()
        r = json.loads(out)
        job_progress(hi, "Programs ready")
        return r
    except (OSError, ValueError) as e:
        return {"ok": False, "why": "tools/native-install.py didn't answer (" + str(e) + ")"}


def build_parts_staged(parts, source, into, lo=80, hi=95):
    """Each part built from the staging checkout, its build kept in INTO
    (swapped in at the restart: install_parts). Failures are reported, not
    fatal (the shell falls back: frosted glass without the plugin)."""
    built, failed = [], []
    for i, part in enumerate(parts):
        job_progress(lo + (hi - lo) * i / max(1, len(parts)), "Building " + PART_NAMES[part], "parts")
        dest = os.path.join(into, part)
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest, exist_ok=True)
        if part == "plugin":
            cmd, env = ["bash", os.path.join(source, "tools", "build-plugin.sh")], dict(os.environ, TESSERA_PLUGIN_DEST=dest)
        elif part == "shaders":
            cmd, env = ["bash", os.path.join(source, "tools", "build-shaders.sh")], None
        else:
            cmd, env = [os.path.join(source, "shell", "native", "build.sh")], None
        res = subprocess.run(cmd, capture_output=True, text=True, env=env)
        if JOB:
            JOB.log((res.stdout or "") + (res.stderr or ""))
        ok = res.returncode == 0
        if ok and part == "shaders":
            for top in ("shell", "material"):
                for d, _, files in os.walk(os.path.join(source, top)):
                    for f in files:
                        if f.endswith(".qsb"):
                            rel = os.path.relpath(os.path.join(d, f), source)
                            os.makedirs(os.path.join(dest, os.path.dirname(rel)), exist_ok=True)
                            shutil.copy2(os.path.join(d, f), os.path.join(dest, rel))
        elif ok and part == "native":
            b = os.path.join(source, "shell", "native", "build")
            for name in ("qml", "glass-ipc", "built-for-qt"):
                src = os.path.join(b, name)
                if os.path.isdir(src):
                    shutil.copytree(src, os.path.join(dest, name), symlinks=True)
                elif os.path.isfile(src):
                    shutil.copy2(src, os.path.join(dest, name))
            ok = os.path.isdir(os.path.join(dest, "qml"))
        elif ok and part == "plugin":
            ok = os.path.isfile(os.path.join(dest, "tessera-glass.so"))
        (built if ok else failed).append(part)
        if not ok:
            shutil.rmtree(dest, ignore_errors=True)
    return built, failed


def _swap_file(src, dst):
    """DST becomes a copy of SRC in one rename (a loaded plugin keeps its copy)"""
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    tmp = dst + ".new"
    shutil.copy2(src, tmp)
    os.replace(tmp, dst)


def _swap_dir(src, dst):
    tmp, old = dst + ".new", dst + ".old"
    shutil.rmtree(tmp, ignore_errors=True)
    shutil.copytree(src, tmp, symlinks=True)
    shutil.rmtree(old, ignore_errors=True)
    if os.path.exists(dst):
        os.replace(dst, old)
    os.replace(tmp, dst)
    shutil.rmtree(old, ignore_errors=True)


def install_parts(staged, into=None):
    """The restart: the staged parts' builds swapped in (the shell is
    stopped meanwhile). The glass plugin is loaded again now when it was
    built for the Hyprland that's running; after a Hyprland update it loads
    at the next login. Returns (built, failed)."""
    into = into or staged_dir()
    built, failed = [], list(staged.get("failed") or [])
    for part in staged.get("parts") or []:
        src = os.path.join(into, part)
        try:
            if part == "plugin":
                so = os.path.join(src, "tessera-glass.so")
                _swap_file(so, os.path.join(data_dir(), "tessera-glass.so"))
                if os.path.exists(os.path.join(src, "tessera-glass.built-for")):
                    _swap_file(os.path.join(src, "tessera-glass.built-for"), os.path.join(data_dir(), "tessera-glass.built-for"))
                # (a console session loads the checkout's own build)
                if os.environ.get("TESSERA_TTY") == "1":
                    _swap_file(so, os.path.join(REPO, "hypr", "plugin", "tessera-glass", "tessera-glass.so"))
                reload_plugin(_read_text(os.path.join(src, "tessera-glass.built-for")).strip())
            elif part == "shaders":
                for d, _, files in os.walk(src):
                    for f in files:
                        rel = os.path.relpath(os.path.join(d, f), src)
                        _swap_file(os.path.join(d, f), os.path.join(REPO, rel))
            elif part == "native":
                b = os.path.join(REPO, "shell", "native", "build")
                os.makedirs(b, exist_ok=True)
                for name in os.listdir(src):
                    s, d = os.path.join(src, name), os.path.join(b, name)
                    (_swap_dir if os.path.isdir(s) else _swap_file)(s, d)
            built.append(part)
        except OSError as e:
            print("installing " + part + " failed: " + str(e), file=sys.stderr)
            failed.append(part)
    remember_parts(built, "HEAD", staged.get("keys"))
    return built, failed


def reload_plugin(built_for):
    """The new glass plugin into the running Hyprland, when it's built for it"""
    if not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE") or not shutil.which("hyprctl"):
        return False
    m = re.search(r"[0-9]+\.[0-9]+(\.[0-9]+)?", _out(["hyprctl", "version"]))
    if not m or not built_for or m.group(0) != built_for:
        return False
    so = os.path.join(data_dir(), "tessera-glass.so")
    if os.environ.get("TESSERA_TTY") == "1":
        so = os.path.join(REPO, "hypr", "plugin", "tessera-glass", "tessera-glass.so")
    for old in (os.path.join(data_dir(), "tessera-glass.so"), os.path.join(REPO, "hypr", "plugin", "tessera-glass", "tessera-glass.so")):
        _out(["hyprctl", "plugin", "unload", old])
    return _out(["hyprctl", "plugin", "load", so]).strip() == "ok"


def read_staged():
    s = load_json(os.path.join(staged_dir(), "staged.json"), {})
    return s if isinstance(s, dict) else {}


def discard():
    """Forget a staged update (Cancel once it's ready, or a stale one)"""
    shutil.rmtree(staged_dir(), ignore_errors=True)
    subprocess.run([sys.executable, "-I", NATIVE, "stage", "--discard"], capture_output=True, timeout=60)
    # (the job's "ready" is over with it)
    st = os.path.join(job_dir(), "status.json")
    if load_json(st, {}).get("state") == "ready":
        try:
            os.remove(st)
        except OSError:
            pass


def staged_info():
    """{"ready", "latest", "current", "valid", "at", "parts", "ui"}: the
    staged update, for the shell (Settings, the notice)"""
    s = read_staged()
    if not s.get("to"):
        return {"ready": False}
    head = git("rev-parse", "HEAD", check=False).strip()
    return {"ready": True, "valid": head == s.get("from") and s.get("repo", os.path.realpath(REPO)) == os.path.realpath(REPO), "latest": s.get("latest", ""), "current": s.get("current", ""),
            "at": s.get("at", 0), "parts": s.get("parts", []), "ui": s.get("ui", {}), "channel": s.get("channel", "")}


# Conflicts that weren't there when the update was prepared (a setting
# changed meanwhile) settle the way that changes nothing of yours; a file
# you changed meanwhile needs a decision, so the update waits for one
SETTLED = {"setting": "keep", "preset": "keep", "shortcut": "mine"}


def settled_choices(report, choices):
    out = dict(choices)
    undecided = []
    for c in report.get("conflicts", []):
        if out.get(c["id"]) in c["choices"]:
            continue
        if c["kind"] in SETTLED and SETTLED[c["kind"]] in c["choices"]:
            out[c["id"]] = SETTLED[c["kind"]]
        else:
            undecided.append(c["title"])
    if undecided:
        raise SystemExit("Something changed since the update was prepared (" + ", ".join(undecided[:3]) +
                         "). Check for updates to decide again.")
    return out


def stage(config_dir, choices, extra=(), channel=None, job=None):
    """Software Update's Update, in the background with the shell running:
    the new version made ready beside the installed one (its programs,
    patched or downloaded or built; its parts built from a staging
    checkout), the checkout itself untouched. The shell's restart (finish)
    then takes seconds. extra: parts to rebuild anyway; with no Tessera
    update, only those, in place (they load at the next login)."""
    global JOB
    JOB = job
    lock = repo_lock()  # noqa: F841 (held while this runs)
    report = check(config_dir, fetch=False, channel=channel)
    if report.get("error"):
        raise SystemExit(report["error"])
    if report["available"] and not report.get("downgrade"):
        if preparing(native_plan(report["target"], report.get("channel")), report["target"]):
            report["available"] = False
            report["preparing"] = True
    if not report["available"]:
        if not extra:
            return {"staged": False, "state": "none", "message": "Tessera's new version is still being prepared; it's offered once it's ready."
                    if report.get("preparing") else "Already up to date."}
        # (after a system update: the parts it needs, in place; Hyprland
        # loads the new plugin at the next login, keeping the one it has)
        built, failed = rebuild(list(extra), 10, 80)
        remember_parts(built, "HEAD")
        return {"staged": False, "state": "done", "rebuilt": True, "built": built, "failed": failed}
    missing = [c["id"] for c in report["conflicts"] if choices.get(c["id"]) not in c["choices"]]
    if missing:
        raise SystemExit("Decide every conflict first: " + ", ".join(missing))
    target = report["target"]
    discard()
    job_progress(3, "Preparing", "prepare", latest=report.get("latest", ""), current=report.get("current", ""))
    source = stage_source(target)
    ui = native_stage(target, source)
    if not ui.get("ok"):
        raise SystemExit("Tessera's programs couldn't be made ready: " + (ui.get("why") or "no answer"))
    parts = list(report.get("rebuild", []))
    parts += [p for p in extra if p not in parts and p in PARTS]
    into = staged_dir()
    os.makedirs(into, exist_ok=True)
    built, failed = build_parts_staged(parts, source, into)
    staged = {"format": 1, "repo": os.path.realpath(REPO), "from": git("rev-parse", "HEAD").strip(), "to": target, "latest": report.get("latest", ""),
              "current": report.get("current", ""), "channel": report.get("channel", ""), "choices": choices,
              "parts": built, "failed": failed, "keys": {p: part_key(p, target) for p in built},
              "ui": {k: ui.get(k) for k in ("staged", "kept", "how", "bytes", "label")}, "at": int(time.time()),
              "changelog": report.get("changelog", []), "notes": report.get("notes", [])}
    write_json(os.path.join(into, "staged.json"), staged)
    job_progress(100, "Ready to restart", "ready")
    return {"staged": True, "state": "ready", "latest": staged["latest"], "parts": built, "failed": failed, "ui": staged["ui"]}


def finish(config_dir, if_ready=False):
    """The shell's restart (update-run.sh finish, or launch-shell.sh at the
    next login, with no shell running): the staged update installed, in
    seconds. Whatever happens, the staged update is used up: a failure puts
    everything back as it was (apply) and says why."""
    rec = recover()
    if rec.get("recovered") and if_ready:
        return rec  # (at login: back as it was; the staged update waits for Restart)
    s = read_staged()
    if not s.get("to"):
        return {"applied": False, "message": "" if if_ready else "There's no prepared update to install."}
    if s.get("repo") and s["repo"] != os.path.realpath(REPO):
        return {"applied": False, "message": "" if if_ready else "The prepared update is for another copy of Tessera."}
    lock = repo_lock()  # noqa: F841 (before anything: a stage still running keeps its work)
    try:
        if git("rev-parse", "HEAD").strip() != s.get("from"):
            raise SystemExit("Tessera changed since the update was prepared. Check for updates again.")
        progress(20, "Installing the update")
        return apply(config_dir, {}, (), s.get("channel") if s.get("channel") in CHANNELS else None, staged=s)
    finally:
        discard()


def _remove_request(d):
    try:
        os.remove(os.path.join(d, "request.json"))
    except OSError:
        pass


def job_interrupted(status=None, alive=None):
    """Was the background job cut off (its status says running, its
    process is gone: a reboot, a logout, a crash)?"""
    st = status if status is not None else load_json(os.path.join(job_dir(), "status.json"), {})
    if not isinstance(st, dict) or st.get("state") != "running":
        return False
    pid = st.get("pid")
    alive = alive or (lambda p: os.path.exists("/proc/" + str(p)) and "tessera-update" in _read_text("/proc/" + str(p) + "/cmdline"))
    return not (pid and alive(pid))


def resume(a):
    """A job cut off (job_interrupted): started again from what it was asked
    (request.json), in the background; what it had done is redone quickly
    (downloads checked again, builds from the cache)"""
    req = load_json(os.path.join(job_dir(), "request.json"), None)
    if not isinstance(req, dict) or not job_interrupted():
        return {"resumed": False}
    a.rebuild = req.get("rebuild") or ""
    a.channel = req.get("channel") if req.get("channel") in CHANNELS else None
    out = run_stage(a, req.get("config") or a.config, req.get("choices") or {})
    return dict(out, resumed=True)


def unblock_signals():
    """The shell (tessera-ui) blocks SIGTERM and friends for its signalfd, and
    a blocked signal stays blocked across fork and exec: a job it started,
    and the cargo and rustc under it, never heard Cancel (19 Nov, the arena).
    Unblocked here, first thing, for this process and all it starts."""
    import signal
    sigs = {signal.SIGTERM, signal.SIGINT, signal.SIGHUP}
    try:
        signal.pthread_sigmask(signal.SIG_UNBLOCK, sigs)
    except (AttributeError, OSError, ValueError):
        pass
    for sg in sigs - {signal.SIGTERM}:
        try:
            if signal.getsignal(sg) == signal.SIG_IGN:
                signal.signal(sg, signal.SIG_DFL)
        except (OSError, ValueError):
            pass


def cancel_job(grace=10.0, sleep=time.sleep, killpg=None, alive=None):
    """Stops the background job, its whole process group (cargo, rustc and
    all): SIGTERM, then SIGKILL after GRACE seconds if any of it is still
    there. However it ended, its status says cancelled afterwards (a job
    that died with "running" would be resumed at the next start) and what
    it was asked is forgotten. True when there was a job."""
    import signal
    killpg = killpg or os.killpg
    alive = alive or _group_alive
    try:
        with open(os.path.join(job_dir(), "pid")) as f:
            pid = int(f.read().strip())
    except (OSError, ValueError):
        return False
    try:
        killpg(pid, signal.SIGTERM)
    except OSError:
        pass
    waited = 0.0
    while alive(pid) and waited < grace:
        sleep(0.25)
        waited += 0.25
    if alive(pid):
        try:
            killpg(pid, signal.SIGKILL)
        except OSError:
            pass
        for _ in range(20):
            if not alive(pid):
                break
            sleep(0.1)
    st_path = os.path.join(job_dir(), "status.json")
    st = load_json(st_path, {})
    if isinstance(st, dict) and st.get("state") == "running":
        st.update(state="cancelled", text="Update cancelled", finished=time.time())
        write_json(st_path, st)
        discard()
    _remove_request(job_dir())
    try:
        os.remove(os.path.join(job_dir(), "pid"))
    except OSError:
        pass
    return True


def _group_alive(pgid):
    """Is any process of the group PGID still running (not a zombie)?"""
    try:
        for d in os.listdir("/proc"):
            if not d.isdigit():
                continue
            try:
                with open(f"/proc/{d}/stat") as f:
                    fields = f.read().rsplit(")", 1)[1].split()
                if int(fields[2]) == pgid and fields[0] != "Z":
                    return True
            except (OSError, IndexError, ValueError):
                continue
    except OSError:
        pass
    return False



def hold_awake(why):
    """The screen stays on and the computer awake while an update runs:
    a systemd idle and sleep inhibitor (hypridle honours it), let go when
    this process ends. 19 Nov: the screen went off in the middle of one.
    None when systemd-inhibit isn't there."""
    import shutil as _sh
    if not _sh.which("systemd-inhibit") or os.environ.get("TESSERA_NO_INHIBIT") == "1":
        return None
    try:
        p = subprocess.Popen(["systemd-inhibit", "--what=idle:sleep", "--who=Tessera", "--why=" + why, "--mode=block",
                              "sh", "-c", "while kill -0 \"$1\" 2>/dev/null; do sleep 5; done", "sh", str(os.getpid())],
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    except OSError:
        return None
    import atexit
    atexit.register(lambda: p.poll() is None and p.terminate())
    return p


def run_stage(a, config_dir, choices):
    """The `stage` command: the job's status kept as it goes; cancelled by
    SIGTERM (cancel_job), it leaves nothing staged"""
    import signal
    job = Job()
    awake = hold_awake("Updating Tessera")  # noqa: F841 (held while this runs)
    write_json(os.path.join(job.dir, "request.json"), {"config": config_dir, "choices": choices, "rebuild": a.rebuild, "channel": a.channel})

    def stop(*_):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, stop)
    try:
        out = stage(config_dir, choices, [r for r in a.rebuild.split(",") if r], a.channel, job=job)
        _remove_request(job.dir)
        job.end(out.get("state", "done"), percent=100, text={"ready": "Ready to restart", "none": out.get("message", ""),
                                                            "done": "Done"}.get(out.get("state"), "Done"))
        return out
    except KeyboardInterrupt:
        discard()
        job.end("cancelled", text="Update cancelled")
        _remove_request(job.dir)
        return {"error": "", "cancelled": True}
    except (RuntimeError, SystemExit) as e:
        discard()
        job.end("error", error=str(e), text="The update didn't finish")
        return {"error": str(e)}
    except Exception as e:
        discard()
        job.end("error", error="The updater stopped: " + (str(e) or e.__class__.__name__), text="The update didn't finish")
        return {"error": "The updater stopped: " + (str(e) or e.__class__.__name__)}


def main():
    unblock_signals()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["check", "apply", "info", "peek", "fetch", "stage", "finish", "discard", "staged", "cancel",
                                        "recover", "resume"])
    ap.add_argument("--choices-json", help="stage: the conflicts' choices as JSON text")
    ap.add_argument("--if-ready", action="store_true", help="finish: quietly nothing when nothing is staged (launch-shell.sh at login)")
    ap.add_argument("--channel", choices=sorted(CHANNELS), help="this channel instead of config.json's update.channel")
    ap.add_argument("--mark", action="store_true", help="peek: write down that a new version was mentioned")
    ap.add_argument("--config", default="", help="the shell's config folder (config.json, keybinds.json)")
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
        elif a.command == "fetch":
            # (tessera update: the branch this checkout follows, from wherever answers)
            up = tracked() if is_checkout() else ""
            remote, branch = up.split("/", 1) if up else follow(a.config, a.channel)[1:]
            f = fetch_branch(remote, branch, remote + "/" + branch) if is_checkout() else {"ok": False, "why": [NOT_A_CHECKOUT]}
            out = {"upstream": remote + "/" + branch, "source": f.get("source")} if f["ok"] else \
                {"error": "Couldn't download the update: " + ((f["why"] or ["no answer"])[-1])}
        elif a.command == "stage":
            choices = load_json(a.choices, {}) if a.choices else {}
            if a.choices_json:
                try:
                    choices = json.loads(a.choices_json)
                except ValueError:
                    choices = {}
            out = run_stage(a, a.config, choices if isinstance(choices, dict) else {})
        elif a.command == "finish":
            out = finish(a.config, a.if_ready)
        elif a.command == "discard":
            discard()
            out = {"discarded": True}
        elif a.command == "staged":
            out = staged_info()
        elif a.command == "cancel":
            out = {"cancelled": cancel_job()}
        elif a.command == "recover":
            out = recover()
        elif a.command == "resume":
            out = resume(a)
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
