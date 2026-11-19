#!/usr/bin/env python3
"""Tessera's Rust programs, ready-made or built here: tessera-ui (the
interface in Rust, native/tessera-ui) and tessera-events (the system watcher,
native/tessera-events), in $XDG_DATA_HOME/tessera/bin, matching this checkout.

    native-install.py install [--mode auto|build|buildTuned] [--ui] [--offline] [--only-prev] [--json]
    native-install.py fetch   [--ref REF] [--json]   download and check only (installs nothing)
    native-install.py plan    [--ref REF] [--online] [--channel stable|beta] [--json]   what install would do
    native-install.py status  [--json]                what's installed, from where
    native-install.py restore                         the previous build back (undo)
    native-install.py cpu                             x86-64-v3, generic (or nothing)
    native-install.py can-build [--packages] [--json] whether building here can work
    native-install.py stage   --ref REF [--source DIR] [--json] [--progress]
                              the programs for REF made ready beside the installed ones
                              (fetched, or built from DIR, a checkout of REF); nothing installed
    native-install.py install --staged [--json]       the staged programs swapped in (HEAD must be REF)
    native-install.py cache   [--delete] [--json]     the build cache: its size, or delete it

Each program has its own source key (part_keys): tessera-events its folder's
git tree, tessera-ui its folder's tree and the few files outside it that it
compiles in (UI_EXTRA). A program whose key is the installed one's is kept
as it is (never downloaded or built again), even when the other changed.

Patches: a release's manifest may list, per variant and program, zstd
--patch-from patches from the builds before it (by their native/ tree). When
the installed program is exactly the one a patch starts from (its sha256),
the patch is downloaded instead of the archive: its own signature is
checked, its size and sha256 against the signed manifest, and the rebuilt
program's sha256 against the manifest's. Any of that failing falls back to
the whole archive, quietly.

Building here keeps cargo's work between updates in one target folder per
variant ($XDG_CACHE_HOME/tessera/cargo/generic, …/tuned), with the "local"
profile (thin LTO, 16 codegen units: minutes, not the release profile's
three-quarters of an hour), so an update rebuilds only what changed. The
folder is wiped when the Rust toolchain changes (everything in it is stale
then) and pruned after each build to CACHE_CAP: units the last build didn't
use go first, then the other variant's folder.

How (rust.uiSource in Settings' config.json, or --mode):
  auto        (the default) the ready-made build for this exact version,
              from the project's GitHub releases (else its GitLab mirror's
              packages, tools/sources.txt): the one for this processor
              (x86-64-v3, or generic), its minisign signature checked against
              tools/tessera-ui.pub and its sha256 against the signed manifest.
              Without one (a checkout with your own edits, a version not
              built yet, no network, no release key yet), built here instead.
  build       always built here (tools/build-native.sh), for this processor's level
              (x86-64-v3 or generic, as the ready-made build would be).
  buildTuned  built here for this processor only (-C target-cpu=native).

The build is chosen by the git tree of native/ (git rev-parse HEAD:native):
a commit that only changes QML uses the same build as the one before it. The
signed manifest names the tree and the commit it was built from; both are
checked against this checkout.

Installing never leaves a half-done program: each one is written beside the
old one and renamed over it (the running copy keeps its file), and the one it
replaces is kept as <name>.prev (with <name>.json saying where each came from).
`restore`, or install for a version whose build is the .prev one (the code went
back), swaps them back without a download. On any failure the installed
programs stay exactly as they were, and it says why.

Programs are never started here except with --version, and then without a
display (WAYLAND_DISPLAY, WAYLAND_SOCKET and DISPLAY unset).
"""
import argparse
import base64
import fcntl
import hashlib
import json
import os
import platform
import re
import secrets
import shutil
import subprocess
import sys
import tarfile
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROGRAMS = ("tessera-ui", "tessera-events")
VARIANTS = ("x86-64-v3", "generic")
MODES = ("auto", "build", "buildTuned")
SHORT = {"x86-64-v3": "v3", "generic": "generic"}
ROLLING = "tessera-ui-latest"  # main's builds, the Stable channel (a rolling prerelease); tags v* get their own
BETA = "tessera-ui-beta"  # the beta branch's builds, the Beta channel (rolling too)
ROLLINGS = {"stable": [ROLLING, BETA], "beta": [BETA, ROLLING]}  # where to look first, per channel
MAX_DOWNLOAD = 200 * 1024 * 1024
MAX_MANIFEST = 256 * 1024
GIB = 1024 ** 3
NEED_RAM = 4 * GIB  # (a release build of tessera-ui: link-time optimisation, one codegen unit)
NEED_DISK = 4 * GIB
CACHE_CAP = int(4.5 * GIB)  # the build cache, after a build
PATCH_WINDOW = 28  # zstd --long: the patch window (2^28: room for a program up to 256 MB)
# Files outside native/tessera-ui that tessera-ui compiles in (include_str!):
# a change to one is a change to tessera-ui (tests/test_native_install.py checks
# the list against the sources)
UI_EXTRA = ("shell/assets/polkit/org.tessera.root.policy", "shell/common/Config.qml", "shell/common/material-symbols.js",
            "shell/common/phosphor.js", "shell/modules/cheatsheet/elements.js")

# x86-64-v3 (AVX2 and friends): every processor's flags in /proc/cpuinfo must
# have these. LZCNT shows as "abm" on both Intel and AMD; XSAVE is the
# kernel's (it hides avx when the OS doesn't save its registers)
V2 = {"cx16", "lahf_lm", "popcnt", "sse4_1", "sse4_2", "ssse3"}
V3 = {"avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "movbe", "xsave"}


class Failed(Exception):
    """Nothing installed; `why` says why, `kind` what kind of failure:
    missing (no ready-made build), network, refused (signature, checksum,
    contents), build, busy."""

    def __init__(self, why, kind="refused"):
        super().__init__(why)
        self.why = why
        self.kind = kind


# ── Where things are ─────────────────────────────────────────────────────

def data_home():
    return os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share")


def dest_dir():
    return os.environ.get("TESSERA_NATIVE_DEST") or os.path.join(data_home(), "tessera", "bin")


def config_path():
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    return os.path.join(base, "tessera", "config.json")


def pubkey_path():
    return os.environ.get("TESSERA_UI_PUBKEY") or os.path.join(REPO, "tools", "tessera-ui.pub")


def load_config():
    try:
        with open(config_path()) as f:
            c = json.load(f)
        return c if isinstance(c, dict) else {}
    except (OSError, ValueError):
        return {}


def configured_mode(config=None):
    rust = (config if config is not None else load_config()).get("rust")
    mode = rust.get("uiSource") if isinstance(rust, dict) else None
    return mode if mode in MODES else "auto"


def configured_channel(config=None, repo=None):
    """Software Update's channel (update.channel): stable | beta. Until one
    is chosen, the branch the checkout follows (beta: beta)."""
    up = (config if config is not None else load_config()).get("update")
    ch = up.get("channel") if isinstance(up, dict) else None
    if ch in ROLLINGS:
        return ch
    return "beta" if (git("rev-parse", "--abbrev-ref", "@{u}", repo=repo) or "").endswith("/beta") else "stable"


def ui_in_use(config=None):
    """Is tessera-ui the shell (or drawing part of it)? As launch-shell.sh
    decides: the shell itself unless rust.standalone is false, which is the
    default since the Rust shell became Tessera (the QML one is its
    fallback). It read only rust.surfaces, from when tessera-ui drew a few
    panels for the QML shell: an install and every update left tessera-ui
    unbuilt, and the computer started the QML shell."""
    rust = (config if config is not None else load_config()).get("rust")
    if not isinstance(rust, dict):
        return True
    return rust.get("standalone") is not False or bool(rust.get("surfaces"))


# ── The processor ────────────────────────────────────────────────────────

def cpu_level(cpuinfo, machine=None):
    """"x86-64-v3", "generic" (any other x86-64), or None (not x86-64: no
    ready-made build). Every processor's flags count (all must have them)."""
    machine = machine or platform.machine()
    if machine not in ("x86_64", "amd64", "AMD64"):
        return None
    sets = [set(line.split(":", 1)[1].split()) for line in cpuinfo.splitlines()
            if line.split(":", 1)[0].strip() == "flags" and ":" in line]
    if not sets:
        return "generic"
    flags = set.intersection(*sets)
    if V2 <= flags and V3 <= flags and ({"abm", "lzcnt"} & flags):
        return "x86-64-v3"
    return "generic"


def detect_cpu():
    forced = os.environ.get("TESSERA_UI_VARIANT")
    if forced in VARIANTS:
        return forced
    try:
        with open(os.environ.get("TESSERA_CPUINFO") or "/proc/cpuinfo") as f:
            return cpu_level(f.read())
    except OSError:
        return "generic" if platform.machine() in ("x86_64", "amd64") else None


# ── The checkout ─────────────────────────────────────────────────────────

def git(*args, repo=None):
    r = subprocess.run(["git", "-C", repo or REPO, *args], capture_output=True, text=True, timeout=60)
    return r.stdout.strip() if r.returncode == 0 else None


def native_tree(ref="HEAD", repo=None):
    """(tree, commit, edited): the git tree of native/ at REF, the commit, and
    whether native/ has edits of yours that aren't committed (then no
    ready-made build matches). (None, None, False) outside a git checkout."""
    commit = git("rev-parse", "--verify", "--quiet", ref + "^{commit}", repo=repo)
    tree = git("rev-parse", "--verify", "--quiet", (commit or ref) + ":native", repo=repo) if commit else None
    edited = False
    if tree and ref == "HEAD":
        edited = bool(git("status", "--porcelain", "--untracked-files=normal", "--", "native", repo=repo))
    return tree, commit, edited


def part_keys(ref="HEAD", repo=None):
    """{program: its source key at REF}, None for one that isn't there (or
    outside a checkout). tessera-events: the git tree of native/tessera-events;
    tessera-ui: a hash of native/tessera-ui's tree and UI_EXTRA's blobs."""
    out = {n: None for n in PROGRAMS}
    commit = git("rev-parse", "--verify", "--quiet", ref + "^{commit}", repo=repo)
    if not commit:
        return out
    ev = git("rev-parse", "--verify", "--quiet", commit + ":native/tessera-events", repo=repo)
    out["tessera-events"] = ev or None
    ui = git("rev-parse", "--verify", "--quiet", commit + ":native/tessera-ui", repo=repo)
    if ui:
        extra = git("ls-tree", commit, "--", *UI_EXTRA, repo=repo) or ""
        out["tessera-ui"] = hashlib.sha1(("tessera-ui " + ui + "\n" + extra).encode()).hexdigest()
    return out


def release_tags(commit, repo=None, channel="stable"):
    """Where a build for COMMIT can be: its own release when a v* tag points
    at it, then the rolling releases, the channel's first (main's for
    Stable, beta's for Beta). A build is named by its native/ tree, so a
    version promoted from beta to main finds its build in either."""
    tags = (git("tag", "--points-at", commit, "--list", "v*", repo=repo) or "").split() if commit else []
    return sorted(tags, reverse=True) + ROLLINGS.get(channel, ROLLINGS["stable"])


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


def releases_bases(repo=None):
    """Where the ready-made builds are, in order, each a download URL with
    {tag} in it: the origin remote's (GitHub releases, or a GitLab project's
    generic packages), then, when origin is one of Tessera's own sources
    (tools/sources.txt), each other source's (the GitLab mirror). Every one
    is checked the same way: signature, manifest, checksums. TESSERA_RELEASES
    sets the list (space-separated; tests: file:// folders)."""
    if os.environ.get("TESSERA_RELEASES", "").strip():
        return os.environ["TESSERA_RELEASES"].split()
    origin = git("remote", "get-url", "origin", repo=repo) or ""
    out = [SOURCES.release_base(origin)]
    if SOURCES.is_official(origin):
        out += [SOURCES.release_base(u) for u in SOURCES.git_sources()]
    seen = []
    for b in out:
        if b and b not in seen:
            seen.append(b)
    return seen


def releases_base(repo=None):
    """The first of releases_bases(), or None."""
    return (releases_bases(repo) or [None])[0]


def manifest_name(tree):
    return f"tessera-ui-{tree}.manifest.json"


def tarball_name(tree, variant):
    return f"tessera-ui-{tree}-{variant}.tar.zst"


# ── minisign ─────────────────────────────────────────────────────────────
# The format (https://jedisct1.github.io/minisign/): a public key is "Ed",
# an 8-byte key id and a 32-byte Ed25519 key; a signature file has the
# signature ("ED": over the file's BLAKE2b-512, "Ed": over the file), a
# trusted comment, and a global signature over signature + trusted comment.
# Checked with python-cryptography (one of Tessera's packages), or the
# minisign program when that's missing.

def _b64(line):
    try:
        return base64.b64decode(line.strip(), validate=True)
    except ValueError as e:
        raise Failed("the signature file isn't minisign's format") from e


def read_pubkey(text):
    lines = [l for l in text.splitlines() if l.strip() and not l.startswith("untrusted comment:")]
    if not lines:
        raise Failed("the release key (tools/tessera-ui.pub) is empty")
    raw = _b64(lines[0])
    if len(raw) != 42 or raw[:2] != b"Ed":
        raise Failed("the release key (tools/tessera-ui.pub) isn't a minisign public key")
    return raw[2:10], raw[10:]


def verify_minisign(pubkey_text, data, sig_text):
    """Raises Failed unless SIG_TEXT is a good signature of DATA by the key;
    returns its trusted comment."""
    keyid, key = read_pubkey(pubkey_text)
    lines = sig_text.splitlines()
    if len(lines) < 4 or not lines[0].startswith("untrusted comment:") or not lines[2].startswith("trusted comment: "):
        raise Failed("the signature file isn't minisign's format")
    sig = _b64(lines[1])
    if len(sig) != 74 or sig[:2] not in (b"Ed", b"ED"):
        raise Failed("the signature file isn't minisign's format")
    if sig[2:10] != keyid:
        raise Failed("it's signed with another key than this copy of Tessera trusts")
    trusted = lines[2][len("trusted comment: "):]
    global_sig = _b64(lines[3])
    message = hashlib.blake2b(data, digest_size=64).digest() if sig[:2] == b"ED" else data
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    except ImportError:
        return _verify_with_program(pubkey_text, data, sig_text, trusted)
    pk = Ed25519PublicKey.from_public_bytes(key)
    try:
        pk.verify(sig[10:], message)
        pk.verify(global_sig, sig[10:] + trusted.encode())
    except InvalidSignature:
        raise Failed("its signature doesn't match: it isn't what the project published") from None
    return trusted


def _verify_with_program(pubkey_text, data, sig_text, trusted):
    if not shutil.which("minisign"):
        raise Failed("nothing here can check signatures (python-cryptography or minisign)")
    import tempfile
    with tempfile.TemporaryDirectory(prefix="tessera-verify-") as d:
        for name, content in (("key.pub", pubkey_text.encode()), ("file", data), ("file.minisig", sig_text.encode())):
            with open(os.path.join(d, name), "wb") as f:
                f.write(content)
        r = subprocess.run(["minisign", "-V", "-q", "-p", os.path.join(d, "key.pub"), "-m", os.path.join(d, "file")],
                           capture_output=True, timeout=30)
    if r.returncode != 0:
        raise Failed("its signature doesn't match: it isn't what the project published")
    return trusted


# ── Downloading ──────────────────────────────────────────────────────────

def fetch_resumable(url, work, name, size, sha256):
    """A big file (an archive, a patch) into WORK/NAME, by way of
    <dest>/.downloads/NAME: a download cut off (the computer went off, the
    job was stopped) carries on from where it got to next time. Checked
    against its signed size and sha256 before it's used; a bad one is
    thrown away whole. Returns its path; raises Failed."""
    keep = os.path.join(os.path.dirname(os.path.abspath(work)), ".downloads")
    os.makedirs(keep, exist_ok=True)
    part = os.path.join(keep, name)
    # (only files of the current build are kept going)
    for old in os.listdir(keep):
        if old != name and old.split("-")[:3] != name.split("-")[:3]:
            _remove(os.path.join(keep, old))
    if os.path.exists(part) and os.path.getsize(part) > (size or 0):
        _remove(part)
    if not (os.path.exists(part) and os.path.getsize(part) == size):
        download(url, part, MAX_DOWNLOAD, resume=True)
    if os.path.getsize(part) != size or sha256_of(part) != sha256:
        _remove(part)
        raise Failed("the download doesn't match its signed checksum (damaged, or not what was published)")
    dst = os.path.join(work, name)
    os.replace(part, dst)
    return dst


def download(url, path, max_bytes, timeout=300, retries=3, connect=10, resume=False):
    """curl, with timeouts and retries. Raises Failed(kind=missing) for a
    file that isn't there, Failed(kind=network) for anything else."""
    local = url.startswith("file://")
    if not local and not url.startswith("https://"):
        raise Failed("only https downloads are allowed", "network")
    cmd = ["curl", "--fail", "--silent", "--show-error", "--location",
           "--proto", "=file" if local else "=https", "--proto-redir", "=https",
           "--connect-timeout", str(connect), "--max-time", str(timeout),
           "--retry", str(retries), "--retry-delay", "2", "--max-filesize", str(max_bytes),
           "--user-agent", "Tessera", "--write-out", "%{http_code}", "--output", path, url]
    if resume and not local:
        cmd[1:1] = ["--continue-at", "-", "--retry-all-errors"]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout + 60)
    except FileNotFoundError:
        raise Failed("curl isn't installed", "network") from None
    except subprocess.TimeoutExpired:
        raise Failed("the download took too long", "network") from None
    if r.returncode == 0:
        return
    code = r.stdout.strip()[-3:]
    if code in ("404", "410") or (local and r.returncode == 37):
        raise Failed("there's no ready-made build for this version", "missing")
    if r.returncode == 63:
        raise Failed("the download is larger than any build should be", "refused")
    raise Failed("it couldn't be downloaded (" + (r.stderr.strip().splitlines() or ["curl " + str(r.returncode)])[-1].replace("curl: ", "") + ")", "network")


def exists_online(url, timeout=15):
    """True / False (not there) / None (couldn't tell: offline)."""
    if url.startswith("file://"):
        return os.path.exists(url[len("file://"):])
    try:
        r = subprocess.run(["curl", "--silent", "--location", "--head", "--proto", "=https", "--proto-redir", "=https",
                            "--connect-timeout", "5", "--max-time", str(timeout), "--user-agent", "Tessera",
                            "--output", "/dev/null", "--write-out", "%{http_code}", url],
                           capture_output=True, text=True, timeout=timeout + 10)
    except (OSError, subprocess.SubprocessError):
        return None
    code = r.stdout.strip()[-3:]
    return True if code == "200" else False if code in ("404", "410") else None


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def program_version(path):
    """What `<program> --version` says, run without any display, or ""."""
    env = {k: v for k, v in os.environ.items()
           if k not in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY", "HYPRLAND_INSTANCE_SIGNATURE")}
    try:
        r = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=10, env=env,
                           stdin=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError):
        return ""
    return r.stdout.strip() if r.returncode == 0 else ""


def unpack(tarball, into, expect):
    """The programs in a release tarball (flat, or under bin/), written by
    name into INTO: nothing else in it is ever written."""
    import tempfile
    with tempfile.TemporaryDirectory(dir=into, prefix=".unpack-") as tmp:
        plain = os.path.join(tmp, "tar")
        r = subprocess.run(["zstd", "-q", "-d", "-f", "-o", plain, tarball], capture_output=True, text=True, timeout=300)
        if r.returncode != 0:
            raise Failed("the download isn't a zstd archive")
        out = {}
        try:
            with tarfile.open(plain, "r:") as tar:
                for m in tar.getmembers():
                    name = os.path.basename(m.name.rstrip("/"))
                    if name not in expect or m.name.strip("./") not in (name, "bin/" + name):
                        continue
                    if not m.isfile() or m.size > MAX_DOWNLOAD:
                        raise Failed(f"{name} in the download isn't a plain file")
                    if name in out:
                        raise Failed(f"the download has {name} twice")
                    src = tar.extractfile(m)
                    path = os.path.join(into, name)
                    with open(path, "wb") as f:
                        shutil.copyfileobj(src, f)
                    os.chmod(path, 0o755)
                    out[name] = path
        except tarfile.TarError as e:
            raise Failed("the download isn't a tar archive (" + str(e) + ")") from None
    return out


# ── Ready-made: find, check, keep ready ──────────────────────────────────

def fetch(tree, commit, variant, work, *, base=None, tags=None, pubkey=None, repo=None, channel=None, want=None, have=None, say=None):
    """Downloads and checks the build for TREE into WORK (a new folder in
    the destination's file system): signature, manifest (tree, commit,
    variant), checksums, contents and --version. WANT: the programs wanted
    (all by default). HAVE: the installed build ({"tree", "variant",
    "programs": {name: path}}): when the manifest has a patch from it, the
    patch is downloaded instead, and checked as strictly. Returns
    {"files": {name: path}, "info": {...}} (info's "how": patch | full,
    "bytes": what was downloaded); raises Failed."""
    if variant is None:
        raise Failed("there are no ready-made builds for this processor (" + platform.machine() + ")", "missing")
    pubkey = pubkey if pubkey is not None else _read(pubkey_path())
    if not pubkey:
        raise Failed("this copy of Tessera has no release key yet (tools/tessera-ui.pub)", "missing")
    bases = base if base is not None else releases_bases(repo)
    bases = [bases] if isinstance(bases, str) else [b for b in bases if b]
    if not bases:
        raise Failed("this checkout doesn't come from GitHub or GitLab, where the ready-made builds are", "missing")
    tags = tags if tags is not None else release_tags(commit, repo, channel or configured_channel(repo=repo))
    # Each source in turn (GitHub, then its GitLab mirror): one that's down,
    # doesn't exist yet or hasn't this build is passed over; every one is
    # checked exactly the same way, and a refused download is never used
    errors = []
    for b in bases:
        shutil.rmtree(work, ignore_errors=True)
        try:
            return _fetch_from(b, tree, commit, variant, work, tags, pubkey, repo, want, have, say)
        except Failed as e:
            errors.append(e)
    shutil.rmtree(work, ignore_errors=True)
    refused = [e for e in errors if e.kind == "refused"]
    raise refused[0] if refused else errors[0]


def mb(n):
    """"8.0 MB" """
    return "{:.1f} MB".format((n or 0) / 1e6)


def patch_name(tree, variant, name, from_tree):
    """A patch's file: tessera-ui-<tree>-<variant>.<program>.from-<from tree, 12>.zst
    (named for its build, so a rolling release's pruning takes it with it)"""
    return f"tessera-ui-{tree}-{variant}.{name}.from-{from_tree[:12]}.zst"


def _fetch_from(base, tree, commit, variant, work, tags, pubkey, repo, want=None, have=None, say=None):
    say = say or (lambda s: None)
    os.makedirs(work, exist_ok=True)
    mpath = os.path.join(work, "manifest.json")
    found = None
    last = None
    for tag in tags:
        url = base.replace("{tag}", tag) + "/" + manifest_name(tree)
        try:
            # (a source that doesn't answer is given up on in seconds)
            download(url, mpath, MAX_MANIFEST, timeout=60, retries=1, connect=5)
            found = (tag, url)
            break
        except Failed as e:
            if e.kind != "missing":
                raise
            last = e
    if not found:
        raise last or Failed("there's no ready-made build for this version", "missing")
    tag, murl = found
    try:
        download(murl + ".minisig", mpath + ".minisig", MAX_MANIFEST, timeout=60)
    except Failed as e:
        if e.kind == "missing":
            raise Failed("the ready-made build isn't signed", "missing") from None
        raise
    with open(mpath, "rb") as f:
        mdata = f.read()
    trusted = verify_minisign(pubkey, mdata, _read(mpath + ".minisig"))
    if "tree=" + tree not in trusted.split():
        raise Failed("the signed manifest is for another version")
    try:
        manifest = json.loads(mdata)
    except ValueError:
        raise Failed("the signed manifest isn't JSON") from None
    if manifest.get("tree") != tree:
        raise Failed("the signed manifest is for another version")
    built_from = manifest.get("commit") or ""
    if not re.fullmatch(r"[0-9a-f]{40}", built_from):
        raise Failed("the signed manifest doesn't say which commit it was built from")
    # (the commit it was built from, when this checkout has it: the same sources)
    known_tree = git("rev-parse", "--verify", "--quiet", built_from + ":native", repo=repo)
    if known_tree is not None and known_tree != tree:
        raise Failed("the signed manifest's commit has other sources than this checkout")
    variants = manifest.get("variants") or {}
    if variant not in variants and "generic" in variants:
        variant = "generic"  # (a tuned one missing: the generic one runs anywhere)
    entry = variants.get(variant)
    if not isinstance(entry, dict) or entry.get("file") != tarball_name(tree, variant):
        raise Failed("there's no ready-made build of this version for this processor", "missing")
    want = [n for n in PROGRAMS if n in (want or PROGRAMS)]
    progs = entry.get("programs") or {}
    rel = base.replace("{tag}", tag)
    how, got = "full", 0
    patched = _try_patches(rel, tree, variant, entry, want, have, work, pubkey, say)
    if patched is not None:
        files, got = patched
        how = "patch"
    else:
        say("STEP Downloading " + mb(entry.get("size")))
        tpath = fetch_resumable(rel + "/" + entry["file"], work, entry["file"], entry.get("size", -1), entry.get("sha256"))
        got = os.path.getsize(tpath)
        files = unpack(tpath, work, PROGRAMS)
        for name in PROGRAMS:
            if name not in files:
                raise Failed(f"the download has no {name}")
    for name in want:
        want_sha = (progs.get(name) or {}).get("sha256")
        if want_sha and sha256_of(files[name]) != want_sha:
            raise Failed(f"{name} in the download doesn't match its signed checksum")
        said = program_version(files[name])
        if not said.startswith(name + " "):
            raise Failed(f"{name} in the download doesn't run on this computer")
        want_v = (progs.get(name) or {}).get("version")
        if want_v and said != want_v:
            raise Failed(f"{name} in the download says it's {said}, not {want_v}")
    info = {"source": "ready-made", "variant": variant, "tree": tree, "commit": built_from,
            "version": manifest.get("version", ""), "channel": manifest.get("channel", ""), "tag": tag,
            "how": how, "bytes": got}
    return {"files": {n: files[n] for n in want}, "info": info}


def _try_patches(rel, tree, variant, entry, want, have, work, pubkey, say):
    """The wanted programs rebuilt from the installed ones with the
    manifest's patches: ({name: path}, bytes downloaded), or None (no patch
    from the installed build, or one didn't check out: the archive then)."""
    if not have or have.get("variant") != variant or not have.get("tree"):
        return None
    patches = (entry.get("patches") or {}).get(have["tree"]) or {}
    progs = entry.get("programs") or {}
    plan = {}
    for name in want:
        old = (have.get("programs") or {}).get(name)
        target = (progs.get(name) or {}).get("sha256")
        if not old or not os.path.isfile(old) or not target:
            return None
        old_sha = sha256_of(old)
        p = patches.get(name)
        if old_sha == target:
            plan[name] = None  # (the very same program: copied)
        elif isinstance(p, dict) and p.get("from_sha256") == old_sha and p.get("file") == patch_name(tree, variant, name, have["tree"]):
            plan[name] = p
        else:
            return None
    total = sum(p.get("size", 0) for p in plan.values() if p)
    out, got = {}, 0
    try:
        if total:
            say("STEP Downloading a patch, " + mb(total))
        for name, p in plan.items():
            dst = os.path.join(work, name)
            if p is None:
                shutil.copy2(have["programs"][name], dst)
            else:
                ppath = fetch_resumable(rel + "/" + p["file"], work, p["file"], p.get("size", -1), p.get("sha256"))
                download(rel + "/" + p["file"] + ".minisig", ppath + ".minisig", MAX_MANIFEST, timeout=60)
                with open(ppath, "rb") as f:
                    data = f.read()
                trusted = verify_minisign(pubkey, data, _read(ppath + ".minisig")).split()
                if p["file"] not in trusted or "tree=" + tree not in trusted:
                    raise Failed("the patch's signature is for another file")
                if len(data) != p.get("size") or hashlib.sha256(data).hexdigest() != p.get("sha256"):
                    raise Failed("the patch doesn't match its signed checksum")
                got += len(data)
                del data
                apply_patch(have["programs"][name], ppath, dst)
                _remove(ppath)
            # (the program it makes is the published one, byte for byte)
            if sha256_of(dst) != progs[name]["sha256"]:
                raise Failed(f"{name} rebuilt from the patch isn't the published one")
            os.chmod(dst, 0o755)
            out[name] = dst
    except Failed as e:
        say("The patch didn't work (" + e.why + "): downloading the whole update")
        for n in plan:
            _remove(os.path.join(work, n))
        return None
    return out, got


def apply_patch(old, patch, out):
    """zstd --patch-from: OUT is OLD with PATCH applied; raises Failed."""
    try:
        r = subprocess.run(["zstd", "-q", "-d", "-f", f"--long={PATCH_WINDOW}", f"--patch-from={old}", patch, "-o", out],
                           capture_output=True, text=True, timeout=300)
    except (OSError, subprocess.SubprocessError) as e:
        raise Failed("zstd couldn't apply the patch (" + str(e) + ")") from None
    if r.returncode != 0:
        raise Failed("zstd couldn't apply the patch (" + (r.stderr.strip().splitlines() or ["exit " + str(r.returncode)])[-1] + ")")


def make_patch(old, new, out, level=19):
    """The release builder's side: a zstd --patch-from patch turning OLD into NEW."""
    r = subprocess.run(["zstd", "-q", "-f", f"-{level}", f"--long={PATCH_WINDOW}", f"--patch-from={old}", new, "-o", out],
                       capture_output=True, text=True, timeout=1800)
    if r.returncode != 0:
        raise Failed("zstd couldn't make the patch (" + (r.stderr.strip().splitlines() or ["exit " + str(r.returncode)])[-1] + ")")


def _read(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return ""


# ── Installing: atomic, keeping .prev ────────────────────────────────────

class Lock:
    def __init__(self, dest):
        os.makedirs(dest, exist_ok=True)
        self.f = open(os.path.join(dest, ".native-install.lock"), "w")

    def __enter__(self):
        try:
            fcntl.flock(self.f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            self.f.close()
            raise Failed("another install of tessera-ui is running", "busy") from None
        return self

    def __exit__(self, *exc):
        self.f.close()


def _fsync_dir(d):
    try:
        fd = os.open(d, os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    except OSError:
        pass


def _keep(src, dst):
    """DST becomes the file SRC is now, in one rename (a hard link: the same
    file, even after SRC is replaced; a copy where links don't work)."""
    tmp = dst + ".tmp-" + secrets.token_hex(4)
    try:
        os.link(src, tmp)
    except OSError:
        shutil.copy2(src, tmp)
    os.replace(tmp, dst)


def read_info(dest, name, prev=False):
    try:
        with open(os.path.join(dest, name + (".prev" if prev else "") + ".json")) as f:
            d = json.load(f)
            return d if isinstance(d, dict) else {}
    except (OSError, ValueError):
        return {}


def _write_info(dest, name, info):
    path = os.path.join(dest, name + ".json")
    tmp = path + ".tmp-" + secrets.token_hex(4)
    with open(tmp, "w") as f:
        json.dump(info, f, indent=1)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)


def swap_in(dest, files, info, parts=None):
    """Installs FILES ({name: path}, already in DEST's file system) over the
    programs in DEST, one rename each; the replaced ones become <name>.prev.
    The installed program is always whole: the old one or the new one.
    PARTS: each program's source key (part_keys), recorded with it."""
    for name, src in files.items():
        info_n = dict(info, part=(parts or {}).get(name)) if parts is not None else info
        final = os.path.join(dest, name)
        os.chmod(src, 0o755)
        with open(src, "rb") as f:
            os.fsync(f.fileno())
        new = final + ".new"
        os.replace(src, new)
        if os.path.exists(final):
            _keep(final, final + ".prev")
            if os.path.exists(os.path.join(dest, name + ".json")):
                _keep(os.path.join(dest, name + ".json"), os.path.join(dest, name + ".prev.json"))
            else:
                _remove(os.path.join(dest, name + ".prev.json"))
        os.replace(new, final)
        extra = {}
        if name == "tessera-ui":
            # (what it says it is: channel, commit, date, variant, kind;
            # Settings shows it when it isn't the build that's running)
            said = program_version(final)
            if said.startswith(name + " "):
                extra["build"] = said
        _write_info(dest, name, dict(info_n, at=int(time.time()), **extra))
    _fsync_dir(dest)


def keep_as_is(dest, names, tree, commit):
    """Programs whose sources didn't change stay as they are, now recorded
    as the build for TREE. Their .prev becomes this same program (with the
    record it had), so going back with `restore` leaves them as they are
    instead of bringing back an older one."""
    for name in names:
        final, ji = os.path.join(dest, name), os.path.join(dest, name + ".json")
        if not os.path.exists(final):
            continue
        info = read_info(dest, name)
        if info.get("tree") == tree:
            continue
        _keep(final, final + ".prev")
        if os.path.exists(ji):
            _keep(ji, os.path.join(dest, name + ".prev.json"))
        _write_info(dest, name, dict(info, tree=tree, commit=commit, kept=True))
    _fsync_dir(dest)


def is_current(mode, info, path, tree, key, edited=False):
    """Is the installed program (INFO, at PATH) the right one for the
    sources (native/ TREE, its part KEY)? A ready-made one for the same
    tree; one recorded with its part key when that key is the same (only
    the other program changed); else, an older record, by the tree."""
    if edited or not os.path.exists(path) or not accepts(mode, info):
        return False
    if info.get("source") == "ready-made" and tree and info.get("tree") == tree:
        return True
    if info.get("part"):
        return bool(key) and info["part"] == key
    return bool(tree) and info.get("tree") == tree


def _remove(path):
    try:
        os.remove(path)
    except FileNotFoundError:
        pass


def restore(dest=None, names=PROGRAMS):
    """The .prev programs back (and the current ones become .prev: restoring
    again goes forward). Returns the names swapped."""
    dest = dest or dest_dir()
    with Lock(dest):
        return _restore(dest, names)


def _restore(dest, names):
    """(restore, with the lock held) Each swap is renames: the program is
    always whole, the .prev one or the current one."""
    done = []
    for name in names:
        final, prev = os.path.join(dest, name), os.path.join(dest, name + ".prev")
        if not os.path.exists(prev):
            continue
        ji, jp = os.path.join(dest, name + ".json"), os.path.join(dest, name + ".prev.json")
        hold = final + ".swap"
        if os.path.exists(final):
            _keep(final, hold)
        os.replace(prev, final)
        if os.path.exists(hold):
            os.replace(hold, prev)
        jhold = ji + ".swap"
        if os.path.exists(ji):
            _keep(ji, jhold)
        if os.path.exists(jp):
            os.replace(jp, ji)
        else:
            _remove(ji)
        if os.path.exists(jhold):
            os.replace(jhold, jp)
        done.append(name)
    _fsync_dir(dest)
    return done


# ── Labels ───────────────────────────────────────────────────────────────

def label(info):
    """"ready-made (v3)", "built here", "built here, tuned"."""
    s = info.get("source")
    if s == "ready-made":
        return "ready-made (" + SHORT.get(info.get("variant"), info.get("variant") or "?") + ")"
    if s == "built-tuned":
        return "built here, tuned"
    if s == "built":
        return "built here"
    return "not installed" if not info else "built here"


def accepts(mode, info):
    """Whether an installed build is what MODE asks for."""
    if mode == "build":
        return info.get("source") == "built"
    if mode == "buildTuned":
        return info.get("source") == "built-tuned"
    return info.get("source") in ("ready-made", "built", "built-tuned")


# ── Building here ────────────────────────────────────────────────────────

def can_build(packages_coming=False, meminfo=None, free_disk=None, cargo=None):
    """{"build": {"ok", "why"}, "buildTuned": {...}}: cargo, about 4 GB of
    memory free and 4 GB of disk. packages_coming: rust is about to be
    installed with Tessera's packages."""
    reasons = []
    cargo = shutil.which("cargo") if cargo is None else cargo
    if not cargo and not packages_coming:
        reasons.append("needs Rust (cargo): sudo pacman -S rust")
    if meminfo is None:
        meminfo = _read("/proc/meminfo")
    m = re.search(r"^MemAvailable:\s+(\d+)\s*kB", meminfo, re.M)
    if m and int(m[1]) * 1024 < NEED_RAM * 0.95:
        reasons.append("needs about 4 GB of free memory ({:.1f} GB now)".format(int(m[1]) * 1024 / GIB))
    if free_disk is None:
        try:
            free_disk = shutil.disk_usage(os.path.join(REPO, "native")).free
        except OSError:
            free_disk = 0
    if free_disk < NEED_DISK * 0.95:
        reasons.append("needs about 4 GB of free disk ({:.1f} GB now)".format(free_disk / GIB))
    why = "; ".join(reasons)
    return {"build": {"ok": not reasons, "why": why}, "buildTuned": {"ok": not reasons, "why": why}}


def run_build(stage, tuned, ui, say, root=None):
    """tools/build-native.sh --stage: builds and copies the programs into
    STAGE (TESSERA_NATIVE_BUILDER: another command, for tests). ROOT: the
    checkout to build from (this one by default; the updater's staging
    checkout of the new version). cargo's work is kept in the build cache
    (cache_dir), with the same flags every time, so the next build only
    compiles what changed. Returns {name: path}; raises Failed."""
    root = root or REPO
    cmd = os.environ.get("TESSERA_NATIVE_BUILDER")
    cmd = [cmd] if cmd else ["bash", os.path.join(root, "tools", "build-native.sh")]
    variant = local_variant(tuned)
    target = cache_dir(variant)
    if not os.environ.get("TESSERA_NATIVE_BUILDER"):
        cache_prepare(target)
    env = dict(build_flags(os.environ, variant), **build_env(tuned, repo=root, variant=variant))
    env.update(TESSERA_CARGO_TARGET=target, TESSERA_CARGO_PROFILE="local")
    cmd += ["--stage", stage] + (["--tuned"] if tuned else []) + (["--ui"] if ui else [])
    say("STEP Building " + ("tessera-ui and tessera-events" if ui else "tessera-events") + " here" + (", tuned" if tuned else ""))
    # Its output as it comes, on stderr (stdout is this program's JSON): the
    # build's progress (tools/build-progress.py) live on a terminal, line by
    # line for the installer and the updater; the last lines kept for why
    sys.stderr.flush()
    last = []
    tty = sys.stderr.isatty()
    # (a terminal: straight through, so the progress line updates in place)
    with subprocess.Popen(cmd, env=env, stdin=subprocess.DEVNULL, stdout=sys.stderr.fileno() if tty else subprocess.PIPE,
                          stderr=sys.stderr.fileno() if tty else subprocess.STDOUT) as p:
        if not tty:
            for raw in p.stdout:
                line = raw.decode("utf-8", "replace").rstrip("\n")
                sys.stderr.write(line + "\n")
                sys.stderr.flush()
                last = (last + [line])[-20:]
        code = p.wait()
    files = {n: os.path.join(stage, n) for n in PROGRAMS if os.path.isfile(os.path.join(stage, n))}
    if code != 0 or "tessera-events" not in files:
        raise Failed("it didn't build (" + build_error(last, code) + ")", "build")
    if not os.environ.get("TESSERA_NATIVE_BUILDER"):
        try:
            cache_prune(target)
        except OSError as e:
            say("The build cache couldn't be pruned (" + str(e) + ")")
    return files


# Environment that changes what cargo builds (and so throws away its cache
# when it differs from one run to the next: a build started from a terminal
# with RUSTFLAGS set, the next one from the shell without)
FLAG_VARS = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR",
             "CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "CARGO_BUILD_TARGET", "CARGO_INCREMENTAL")


def local_variant(tuned):
    """What a build here is for: "native" (tuned: this processor only), else
    this processor's ready-made level, x86-64-v3 or generic, so it's the
    same program the ready-made one is (19 Nov: a v3 laptop built generic)"""
    return "native" if tuned else (detect_cpu() or "generic")


def build_flags(environ, variant):
    """ENVIRON with the same build flags every time: RUSTFLAGS only the
    variant's target-cpu (x86-64-v3; the tuned build's native is added by
    build-native.sh), no other flag or target-folder variables, incremental
    compilation off (the profile's dependencies are what's worth keeping).
    The same variant gives the same flags, run after run, so cargo's cache
    stays good."""
    env = {k: v for k, v in environ.items() if k not in FLAG_VARS and not k.startswith("CARGO_PROFILE_")}
    env.update(RUSTFLAGS="-C target-cpu=x86-64-v3" if variant == "x86-64-v3" else "", CARGO_INCREMENTAL="0")
    return env


def build_env(tuned, repo=None, variant=None):
    """What a build here tells tessera-ui about itself (its build.rs, for
    `tessera-ui --version` and Settings → About): built locally, generic or
    tuned, and the channel when the checkout follows one (origin/main:
    stable, origin/beta: beta; any other branch: build.rs says "local build")."""
    env = {"TESSERA_BUILD_KIND": "local", "TESSERA_BUILD_VARIANT": "native" if tuned else (variant or "generic")}
    up = git("rev-parse", "--abbrev-ref", "@{u}", repo=repo) or git("rev-parse", "--abbrev-ref", "@{u}") or ""
    channel = {"main": "stable", "beta": "beta"}.get(up.split("/", 1)[1] if "/" in up else "")
    if channel:
        env["TESSERA_BUILD_CHANNEL"] = channel
    return env


def build_error(last, code):
    """Why a build failed, in one line: the compiler's first error (from
    build-progress.json, when the build wrote one), else the last line said"""
    path = os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "tessera", "build-progress.json")
    try:
        with open(path) as f:
            snap = json.load(f)
        if snap.get("phase") == "failed" and snap.get("error") and time.time() - snap.get("updated_at", 0) < 600:
            return snap["error"]
    except (OSError, ValueError):
        pass
    lines = [x.strip() for x in last if x.strip() and not x.lstrip().startswith(("The whole log", "BUILD {"))]
    return lines[-1] if lines else "exit " + str(code)


# ── The build cache ──────────────────────────────────────────────────────

HASH16 = re.compile(r"[0-9a-f]{16}")


def cache_root():
    base = os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache")
    return os.path.join(base, "tessera", "cargo")


def cache_dir(variant):
    """One cargo target folder per variant (native, x86-64-v3, generic),
    outside the checkout (it survives the checkout moving, a fresh clone
    and the updater's staging checkout)"""
    return os.path.join(cache_root(), "tuned" if variant in (True, "native") else variant if variant in VARIANTS else "generic")


def dir_size(path):
    total = 0
    stack = [path]
    while stack:
        p = stack.pop()
        try:
            with os.scandir(p) as it:
                for e in it:
                    try:
                        if e.is_dir(follow_symlinks=False):
                            stack.append(e.path)
                        else:
                            total += e.stat(follow_symlinks=False).st_size
                    except OSError:
                        pass
        except (NotADirectoryError, FileNotFoundError, PermissionError):
            pass
    return total


def legacy_targets(repo=None):
    """Where builds here went before the cache: native/<crate>/target/release
    and /tuned in the checkout, when Tessera's own build wrote them (its
    build-progress stats are there)"""
    out = []
    for crate in ("tessera-ui", "tessera-events"):
        t = os.path.join(repo or REPO, "native", crate, "target")
        for sub, stats in (("release", os.path.join(t, "tessera-build-stats.json")), ("tuned", os.path.join(t, "tuned", "tessera-build-stats.json"))):
            if os.path.isdir(os.path.join(t, sub)) and os.path.isfile(stats):
                out.append(os.path.join(t, sub))
    return out


def cache_info(repo=None):
    """{"path", "bytes"}: the build cache and the older builds' leftovers"""
    dirs = [cache_root()] + legacy_targets(repo)
    return {"path": cache_root(), "bytes": sum(dir_size(d) for d in dirs if os.path.isdir(d)), "dirs": [d for d in dirs if os.path.isdir(d)]}


def cache_delete(dest=None, repo=None):
    """Deletes the build cache (never while a build or install runs)"""
    with Lock(dest or dest_dir()):
        info = cache_info(repo)
        for d in info["dirs"]:
            shutil.rmtree(d, ignore_errors=True)
        return info["bytes"]


def toolchain():
    """rustc -vV: the toolchain the cache was made with"""
    try:
        r = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, timeout=30)
        return r.stdout.strip() if r.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def cache_prepare(target, now_toolchain=None):
    """Before a build: a cache made by another Rust toolchain is stale
    through and through (cargo rebuilds all of it), so it goes rather than
    sit beside the new one. 19 Nov: rust 1.99 → 1.100 the evening before an
    update made it compile every crate again."""
    tc = toolchain() if now_toolchain is None else now_toolchain
    stamp = os.path.join(target, "tessera-toolchain")
    if os.path.isdir(target) and tc and _read(stamp).strip() != tc:
        shutil.rmtree(target, ignore_errors=True)
    os.makedirs(target, exist_ok=True)
    if tc:
        with open(stamp, "w") as f:
            f.write(tc + "\n")


def used_hashes(target):
    """The unit hashes the last build used (build-progress.py writes every
    artifact and build-script output folder cargo named, fresh or rebuilt,
    to tessera-build-used-<label>.json)"""
    used, found = set(), False
    try:
        names = os.listdir(target)
    except OSError:
        return used, False
    for n in names:
        if n.startswith("tessera-build-used-") and n.endswith(".json"):
            try:
                with open(os.path.join(target, n)) as f:
                    paths = json.load(f)
            except (OSError, ValueError):
                continue
            found = True
            for p in paths if isinstance(paths, list) else []:
                used.update(HASH16.findall(str(p)))
    return used, found


def _units(profile_dir):
    """(path, hashes) of each unit's files and folders: deps/<x>-<hash>.*,
    .fingerprint/<pkg>-<hash>, build/<pkg>-<hash> and, in cargo's newer
    layout, build/<pkg>/<hash>"""
    for top in ("deps", ".fingerprint", "build"):
        d = os.path.join(profile_dir, top)
        try:
            entries = list(os.scandir(d))
        except OSError:
            continue
        for e in entries:
            hs = HASH16.findall(e.name)
            if hs:
                yield e.path, hs
            elif e.is_dir(follow_symlinks=False):
                try:
                    for c in os.scandir(e.path):
                        ch = HASH16.findall(c.name)
                        if ch:
                            yield c.path, ch
                except OSError:
                    pass


def cache_prune(target, cap=None):
    """After a build, the cache back under CAP: first the units of TARGET
    that the build didn't use (old versions of crates, other features),
    then the other variants' folders. Returns the bytes freed."""
    cap = CACHE_CAP if cap is None else cap
    root = os.path.dirname(target)
    total = dir_size(root)
    if total <= cap:
        return 0
    freed = 0
    used, known = used_hashes(target)
    if known:
        for prof in os.listdir(target):
            pd = os.path.join(target, prof)
            if not os.path.isdir(pd):
                continue
            for path, hs in list(_units(pd)):
                if not any(h in used for h in hs):
                    n = dir_size(path) if os.path.isdir(path) else os.path.getsize(path)
                    shutil.rmtree(path, ignore_errors=True) if os.path.isdir(path) else _remove(path)
                    freed += n
    if total - freed > cap:
        for other in sorted(os.listdir(root)):
            p = os.path.join(root, other)
            if p != target and os.path.isdir(p):
                n = dir_size(p)
                shutil.rmtree(p, ignore_errors=True)
                freed += n
                if total - freed <= cap:
                    break
    return freed


# ── A new build that doesn't start: back to the one before ───────────────

def state_home():
    return os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "tessera")


def bad_path():
    return os.path.join(state_home(), "bad-builds.json")


def bad_trees():
    """native/ trees whose build didn't start here (not offered again)"""
    try:
        with open(bad_path()) as f:
            d = json.load(f)
        return [t for t in d.get("trees", []) if isinstance(t, str)] if isinstance(d, dict) else []
    except (OSError, ValueError):
        return []


def mark_bad(tree):
    if not tree:
        return
    trees = [t for t in bad_trees() if t != tree][-19:] + [tree]
    os.makedirs(state_home(), exist_ok=True)
    _write_info(state_home(), "bad-builds", {"trees": trees})


def rollback(dest=None):
    """launch-shell.sh, when the shell an update just installed didn't stay
    up: the previous build back (restore), the one it replaces marked bad,
    and a note for the shell to say so ("Back on …"). Raises Failed."""
    dest = dest or dest_dir()
    with Lock(dest):
        if not os.path.exists(os.path.join(dest, "tessera-ui.prev")):
            raise Failed("there's no previous build to go back to", "missing")
        gone = read_info(dest, "tessera-ui")
        done = _restore(dest, PROGRAMS)
        back = read_info(dest, "tessera-ui")
        mark_bad(gone.get("tree"))
        os.makedirs(state_home(), exist_ok=True)
        _write_info(state_home(), "rolled-back", {"from": gone.get("build") or gone.get("tree", ""), "tree": gone.get("tree"),
                                                  "to": back.get("build") or back.get("tree", ""), "at": int(time.time())})
        return {"restored": done, "bad": gone.get("tree"), "back": back.get("build", "")}


# ── install ──────────────────────────────────────────────────────────────

def install(mode=None, ui=False, offline=False, only_prev=False, dest=None, repo=None, say=None, builder=None, fetcher=None, config=None):
    """Makes the programs in DEST match the checkout, as MODE asks. Returns
    {"ok", "changed", "source", "label", "why", ...}; never raises for a
    failure (the installed programs stay as they were)."""
    say = say or (lambda s: print(s[5:] if s.startswith("STEP ") else s, file=sys.stderr, flush=True))
    config = load_config() if config is None else config
    mode = mode if mode in MODES else configured_mode(config)
    dest = dest or dest_dir()
    builder = builder or run_build
    fetcher = fetcher or fetch
    tree, commit, edited = native_tree("HEAD", repo)
    # (tessera-ui is built here only when it's used or asked for: it takes
    # minutes; a ready-made download always has both)
    build_ui = ui or ui_in_use(config)
    names = [n for n in PROGRAMS if n == "tessera-events" or build_ui or (mode == "auto" and os.path.exists(os.path.join(dest, n)))]
    out = {"ok": False, "changed": False, "mode": mode, "tree": tree, "commit": commit, "why": ""}
    try:
        # (one install at a time, from the check to the swap)
        with Lock(dest):
            return _install(out, mode, tree, commit, edited, build_ui, names, offline, only_prev, dest, repo, say, builder, fetcher)
    except Failed as e:
        cur = read_info(dest, "tessera-ui") or read_info(dest, "tessera-events")
        return dict(out, why=e.why, kind=e.kind, label=label(cur) if cur else "",
                    message="Kept the installed " + ("build" if cur else "programs (none yet)") + ": " + e.why)


def _accepts_kw(fn, name):
    import inspect
    try:
        ps = inspect.signature(fn).parameters
    except (TypeError, ValueError):
        return False
    return name in ps or any(p.kind == p.VAR_KEYWORD for p in ps.values())


def _todo(mode, names, tree, keys, edited, dest):
    """The programs that need another build for these sources"""
    return [n for n in names if not is_current(mode, read_info(dest, n), os.path.join(dest, n), tree, keys.get(n), edited)]


def _have(dest, names):
    """The installed build, for patches: its tree, variant and programs"""
    info = read_info(dest, "tessera-ui") or read_info(dest, "tessera-events")
    return {"tree": info.get("tree"), "variant": info.get("variant"),
            "programs": {n: os.path.join(dest, n) for n in names if os.path.exists(os.path.join(dest, n))}}


def _produce(mode, tree, commit, edited, build_ui, todo, offline, dest, repo, say, builder, fetcher, into, source=None):
    """The programs TODO for these sources, into INTO (in DEST's file
    system): the ready-made build (by a patch when there is one), else built
    here. Returns (files, info, fallback); raises Failed."""
    fallback = ""
    if mode == "auto":
        why = ("this checkout has edits of its own in native/" if edited else
               "this copy of Tessera isn't a git checkout" if not tree else
               "offline" if offline else "")
        if not why:
            variant = detect_cpu()
            say("STEP Getting tessera-ui (ready-made" + (", " + SHORT[variant] if variant else "") + ")")
            work = os.path.join(dest, ".incoming-" + secrets.token_hex(4))
            try:
                kw = {"repo": repo}
                if _accepts_kw(fetcher, "want"):
                    kw.update(want=todo, have=_have(dest, todo), say=say)
                got = fetcher(tree, commit, variant, work, **kw)
                os.makedirs(into, exist_ok=True)
                files = {}
                for n in todo:
                    if n in got["files"]:
                        os.replace(got["files"][n], os.path.join(into, n))
                        files[n] = os.path.join(into, n)
                return files, got["info"], ""
            except Failed as e:
                why = e.why
                if e.kind == "refused":
                    say("! The ready-made build was refused: " + e.why)
            finally:
                shutil.rmtree(work, ignore_errors=True)
        fallback = why
        say("No ready-made build: " + why + ". Building here instead.")
    tuned = mode == "buildTuned"
    os.makedirs(into, exist_ok=True)
    build = builder
    if source and builder is run_build:
        build = lambda s, t, u, sy: run_build(s, t, u, sy, root=source)  # noqa: E731
    try:
        files = build(into, tuned, build_ui, say)
    except Failed as e:
        raise Failed(e.why + (" (and no ready-made build: " + fallback + ")" if fallback else ""), e.kind) from None
    info = {"source": "built-tuned" if tuned else "built", "variant": local_variant(tuned),
            "tree": tree if not edited else None, "commit": commit, "edited": edited}
    # (a program whose sources didn't change stays as it is, built or not)
    files = {n: p for n, p in files.items() if n in todo}
    return files, info, fallback


def _install(out, mode, tree, commit, edited, build_ui, names, offline, only_prev, dest, repo, say, builder, fetcher):
    keys = part_keys("HEAD", repo) if tree and not edited else {n: None for n in PROGRAMS}
    todo = _todo(mode, names, tree, keys, edited, dest)
    if tree and not edited and not todo:
        keep_as_is(dest, names, tree, commit)
        info = read_info(dest, names[0])
        return dict(out, ok=True, source=info.get("source"), variant=info.get("variant"), label=label(info),
                    message="Already installed (" + label(info) + ")")
    # The code went back to the version of the .prev build: it comes back
    prev = {n: read_info(dest, n, prev=True) for n in names}
    if tree and not edited and all(prev[n].get("tree") == tree and accepts(mode, prev[n]) and os.path.exists(os.path.join(dest, n + ".prev")) for n in names):
        _restore(dest, names)
        info = read_info(dest, names[0])
        return dict(out, ok=True, changed=True, source=info.get("source"), variant=info.get("variant"), label=label(info),
                    message="The previous build is back (" + label(info) + ")")
    if only_prev:
        raise Failed("the previous build isn't for this version", "missing")
    into = os.path.join(dest, ".built-" + secrets.token_hex(4))
    try:
        files, info, fallback = _produce(mode, tree, commit, edited, build_ui, todo, offline, dest, repo, say, builder, fetcher, into)
        swap_in(dest, files, info, parts=keys)
        if tree and not edited:
            keep_as_is(dest, [n for n in names if n not in files], tree, commit)
        how = info.get("how")
        if info.get("source") == "ready-made":
            msg = "Installed the ready-made build (" + label(info) + ")" + (", from a patch" if how == "patch" else "")
        else:
            msg = "Built here (" + label(info) + ")"
        res = dict(out, ok=True, changed=True, source=info["source"], variant=info["variant"], label=label(info),
                   fallback=fallback, message=msg, how=how, bytes=info.get("bytes", 0),
                   installed=sorted(files), kept=[n for n in names if n not in files])
        if build_ui and "tessera-ui" in todo and "tessera-ui" not in files and info.get("source") != "ready-made":
            # (tessera-events is in; the installed tessera-ui, if any, stays)
            res.update(ok=False, kind="build", why="tessera-ui didn't build",
                       message="Built tessera-events here; tessera-ui didn't build, so the shell draws everything in QML meanwhile"
                       if not os.path.exists(os.path.join(dest, "tessera-ui")) else
                       "Built tessera-events here; tessera-ui didn't build, so the installed one stays")
        return res
    finally:
        shutil.rmtree(into, ignore_errors=True)


# ── Staging: the next version's programs, ready beside the installed ones ─

def staged_dir(dest=None):
    return os.path.join(dest or dest_dir(), ".staged")


def read_staged(dest=None):
    try:
        with open(os.path.join(staged_dir(dest), "staged.json")) as f:
            s = json.load(f)
            return s if isinstance(s, dict) else {}
    except (OSError, ValueError):
        return {}


def discard_staged(dest=None):
    shutil.rmtree(staged_dir(dest), ignore_errors=True)


def stage(ref, mode=None, ui=False, source=None, dest=None, repo=None, say=None, builder=None, fetcher=None, config=None):
    """The programs for REF made ready in DEST/.staged (installs nothing:
    the running shell keeps its programs), for `install --staged` when the
    checkout is at REF. Only the programs whose sources change are fetched
    (by a patch when there is one) or built (from SOURCE, a checkout of
    REF). Returns {"ok", "staged", "kept", "how", "bytes", "label", "why"}."""
    say = say or (lambda s: print(s[5:] if s.startswith("STEP ") else s, file=sys.stderr, flush=True))
    config = load_config() if config is None else config
    mode = mode if mode in MODES else configured_mode(config)
    dest = dest or dest_dir()
    builder = builder or run_build
    fetcher = fetcher or fetch
    tree, commit, _ = native_tree(ref, repo)
    build_ui = ui or ui_in_use(config)
    names = [n for n in PROGRAMS if n == "tessera-events" or build_ui or (mode == "auto" and os.path.exists(os.path.join(dest, n)))]
    out = {"ok": False, "mode": mode, "tree": tree, "commit": commit, "staged": [], "kept": [], "why": ""}
    if not tree:
        return dict(out, why="this copy of Tessera isn't a git checkout", kind="missing")
    keys = part_keys(ref, repo)
    try:
        with Lock(dest):
            sdir = staged_dir(dest)
            shutil.rmtree(sdir, ignore_errors=True)
            todo = _todo(mode, names, tree, keys, False, dest)
            record = {"format": 1, "tree": tree, "commit": commit, "keys": keys, "names": names, "mode": mode,
                      "files": {}, "keep": [n for n in names if n not in todo], "at": int(time.time())}
            prev = {n: read_info(dest, n, prev=True) for n in names}
            if todo and all(prev[n].get("tree") == tree and accepts(mode, prev[n]) and os.path.exists(os.path.join(dest, n + ".prev")) for n in names):
                record.update(prev=True, keep=[])  # (going back: the .prev build is it)
                todo = []
                info = prev[names[0]]
                how, got = "prev", 0
            elif todo:
                files, info, _ = _produce(mode, tree, commit, False, build_ui, todo, False, dest, repo, say, builder, fetcher,
                                          os.path.join(sdir, "bin"), source=source)
                record["files"] = {n: os.path.relpath(p, sdir) for n, p in files.items()}
                record["info"] = info
                record["keep"] = [n for n in names if n not in files]
                todo = sorted(files)
                how, got = info.get("how") or "build", info.get("bytes", 0)
            else:
                info = read_info(dest, names[0])
                how, got = "none", 0
            os.makedirs(sdir, exist_ok=True)
            _write_info(sdir, "staged", record)
            return dict(out, ok=True, staged=todo, kept=record["keep"], how=how, bytes=got, label=label(info) if info else "",
                        message=("Nothing to change: " + ", ".join(names) + " stay as they are") if not todo and not record.get("prev") else
                        "Ready: " + ", ".join(todo or names))
    except Failed as e:
        discard_staged(dest)
        return dict(out, why=e.why, kind=e.kind, message="Couldn't get the new programs ready: " + e.why)


def install_staged(dest=None, repo=None):
    """`install --staged`: the staged programs swapped in, when the checkout
    is now at the version they were staged for. Never leaves a half-done
    program; on a mismatch nothing changes."""
    dest = dest or dest_dir()
    tree, commit, edited = native_tree("HEAD", repo)
    out = {"ok": False, "changed": False, "tree": tree, "commit": commit, "why": ""}
    try:
        with Lock(dest):
            s = read_staged(dest)
            if not s:
                raise Failed("nothing is staged", "missing")
            if s.get("tree") != tree or edited:
                raise Failed("the staged programs are for another version", "refused")
            names = s.get("names") or list(PROGRAMS)
            if s.get("prev"):
                _restore(dest, names)
                info = read_info(dest, names[0])
                return dict(out, ok=True, changed=True, label=label(info), message="The previous build is back (" + label(info) + ")")
            sdir = staged_dir(dest)
            files = {n: os.path.join(sdir, rel) for n, rel in (s.get("files") or {}).items()}
            for n, p in files.items():
                if not os.path.isfile(p):
                    raise Failed(f"the staged {n} is missing", "missing")
            swap_in(dest, files, s.get("info") or {}, parts=s.get("keys") or {})
            keep_as_is(dest, s.get("keep") or [], tree, commit)
            info = read_info(dest, names[0])
            return dict(out, ok=True, changed=bool(files), installed=sorted(files), kept=s.get("keep") or [], label=label(info),
                        message="Installed " + (", ".join(sorted(files)) or "nothing new"))
    except Failed as e:
        return dict(out, why=e.why, kind=e.kind, message="Kept the installed programs: " + e.why)
    finally:
        s = read_staged(dest)
        if not s or s.get("tree") == tree:
            discard_staged(dest)


def plan(ref="HEAD", online=False, mode=None, dest=None, repo=None, config=None, channel=None):
    """What install would do for REF (Software Update's check): {"mode",
    "variant", "available", "changes", "label"}. online: ask the release
    server whether the ready-made build is there (a HEAD request)."""
    config = load_config() if config is None else config
    mode = mode if mode in MODES else configured_mode(config)
    dest = dest or dest_dir()
    tree, commit, edited = native_tree(ref, repo)
    variant = detect_cpu()
    cur = read_info(dest, "tessera-ui") or read_info(dest, "tessera-events")
    keys = part_keys(ref, repo) if tree else {n: None for n in PROGRAMS}
    names = [n for n in PROGRAMS if os.path.exists(os.path.join(dest, n))] or list(PROGRAMS)
    todo = _todo(mode, names, tree, keys, edited and ref == "HEAD", dest)
    changes = bool(todo) or not tree
    out = {"mode": mode, "variant": variant, "tree": tree, "commit": commit, "changes": changes, "available": None,
           "parts": todo, "kept": [n for n in names if n not in todo]}
    if mode == "build":
        return dict(out, label="tessera-ui: built here")
    if mode == "buildTuned":
        return dict(out, label="tessera-ui: built here, tuned")
    if not changes:
        return dict(out, available=True, label="tessera-ui: " + label(cur))
    why = ("your edits in native/" if edited and ref == "HEAD" else "not a git checkout" if not tree else
           "no ready-made build for this processor" if variant is None else
           "no release key yet" if not _read(pubkey_path()) else "")
    bases = releases_bases(repo)
    if not why and not bases:
        why = "not from GitHub or GitLab"
    if not why and online:
        tags = release_tags(commit, repo, channel or configured_channel(config, repo))
        seen = []
        for b in bases:  # (GitHub, then its mirror: the first that has it will do)
            seen += [exists_online(b.replace("{tag}", t) + "/" + manifest_name(tree)) for t in tags]
            if True in seen:
                break
        out["available"] = True if True in seen else None if None in seen else False
        if out["available"] is False:
            why = "no ready-made build for it yet"
    if why:
        return dict(out, available=False, label="tessera-ui: built here (" + why + ")")
    return dict(out, label="tessera-ui: ready-made (" + SHORT[variant] + ")")


def status(dest=None):
    dest = dest or dest_dir()
    out = {}
    for n in PROGRAMS:
        if os.path.exists(os.path.join(dest, n)):
            info = read_info(dest, n)
            out[n] = dict(info, label=label(info), prev=os.path.exists(os.path.join(dest, n + ".prev")))
            if n == "tessera-ui" and not info.get("build"):
                said = program_version(os.path.join(dest, n))  # (installed before records kept it)
                if said.startswith(n + " "):
                    out[n]["build"] = said
    return out


# ── Command line ─────────────────────────────────────────────────────────

def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["install", "fetch", "plan", "status", "restore", "cpu", "can-build", "stage", "cache", "rollback", "bad"])
    ap.add_argument("--staged", action="store_true", help="install: the staged programs (native-install.py stage)")
    ap.add_argument("--source", help="stage: a checkout of REF to build from when it must be built here")
    ap.add_argument("--delete", action="store_true", help="cache: delete the build cache")
    ap.add_argument("--discard", action="store_true", help="stage: forget what's staged")
    ap.add_argument("--mode", choices=MODES)
    ap.add_argument("--ui", action="store_true", help="install: tessera-ui too when built here, even before Settings uses it")
    ap.add_argument("--offline", action="store_true", help="install: no download (a .prev build or building here)")
    ap.add_argument("--only-prev", action="store_true", help="install: only the .prev build, when it's this version's (going back)")
    ap.add_argument("--online", action="store_true", help="plan: ask the release server")
    ap.add_argument("--ref", default="HEAD")
    ap.add_argument("--channel", choices=sorted(ROLLINGS), help="plan, fetch: whose rolling release to look in first (default: Software Update's channel)")
    ap.add_argument("--packages", action="store_true", help="can-build: rust comes with Tessera's packages")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--progress", action="store_true", help="install: each step as a STEP line on stderr (the updater)")
    a = ap.parse_args(argv)

    def emit(obj, text):
        print(json.dumps(obj, indent=1) if a.json else text)

    if a.command == "cpu":
        print(detect_cpu() or "")
        return 0
    if a.command == "can-build":
        r = can_build(packages_coming=a.packages)
        emit(r, "yes" if r["build"]["ok"] else "no: " + r["build"]["why"])
        return 0 if r["build"]["ok"] else 1
    if a.command == "status":
        s = status()
        emit(s, "\n".join(f"{n}: {i['label']}" + (f" ({i.get('tree', '')[:12]})" if i.get("tree") else "")
                          + (f"\n  {i['build']}" if i.get("build") else "") for n, i in s.items()) or "Nothing installed")
        return 0
    if a.command == "plan":
        p = plan(a.ref, online=a.online, mode=a.mode, channel=a.channel)
        emit(p, p["label"])
        return 0
    if a.command == "restore":
        done = restore()
        emit({"restored": done}, ("Back to the previous build: " + ", ".join(done)) if done else "There's no previous build to go back to")
        return 0 if done else 1
    if a.command == "fetch":
        tree, commit, _ = native_tree(a.ref)
        dest = dest_dir()
        work = os.path.join(dest, ".incoming-" + secrets.token_hex(4))
        try:
            got = fetch(tree, commit, detect_cpu(), work, channel=a.channel) if tree else None
            if not got:
                raise Failed("not a git checkout", "missing")
            emit(dict(got["info"], ok=True), "Checked: " + label(got["info"]))
            return 0
        except Failed as e:
            emit({"ok": False, "why": e.why, "kind": e.kind}, "No: " + e.why)
            return 1
        finally:
            shutil.rmtree(work, ignore_errors=True)
    if a.command == "rollback":
        try:
            r = rollback()
        except Failed as e:
            emit({"ok": False, "why": e.why}, "No: " + e.why)
            return 1
        emit(dict(r, ok=True), "Back to the previous build")
        return 0
    if a.command == "bad":
        tree = native_tree(a.ref)[0]
        emit({"tree": tree, "bad": tree in bad_trees()}, "bad" if tree in bad_trees() else "ok")
        return 0
    if a.command == "cache":
        if a.delete:
            try:
                freed = cache_delete()
            except Failed as e:
                emit({"ok": False, "why": e.why, "kind": e.kind}, "No: " + e.why)
                return 1
            emit({"ok": True, "freed": freed}, "Deleted the build cache (" + mb(freed) + ")")
            return 0
        c = cache_info()
        emit({"path": c["path"], "bytes": c["bytes"]}, c["path"] + ": " + mb(c["bytes"]))
        return 0
    say = (lambda s: print(s, file=sys.stderr, flush=True)) if a.progress else None
    if a.command == "stage":
        if a.discard:
            discard_staged()
            emit({"ok": True}, "Nothing staged")
            return 0
        r = stage(a.ref, a.mode, ui=a.ui, source=a.source, say=say)
        emit(r, r.get("message") or r.get("why"))
        return 0 if r["ok"] else 1
    if a.staged:
        r = install_staged()
        emit(r, r.get("message") or r.get("why"))
        return 0 if r["ok"] else 1
    r = install(a.mode, ui=a.ui, offline=a.offline, only_prev=a.only_prev, say=say)
    emit(r, r.get("message") or r.get("why"))
    return 0 if r["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
