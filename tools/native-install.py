#!/usr/bin/env python3
"""Tessera's Rust programs, ready-made or built here: tessera-ui (the
interface in Rust, native/tessera-ui) and tessera-events (the system watcher,
native/tessera-events), in $XDG_DATA_HOME/tessera/bin, matching this checkout.

    native-install.py install [--mode auto|build|buildTuned] [--ui] [--offline] [--only-prev] [--json]
    native-install.py fetch   [--ref REF] [--json]   download and check only (installs nothing)
    native-install.py plan    [--ref REF] [--online] [--json]   what install would do
    native-install.py status  [--json]                what's installed, from where
    native-install.py restore                         the previous build back (undo)
    native-install.py cpu                             x86-64-v3, generic (or nothing)
    native-install.py can-build [--packages] [--json] whether building here can work

How (rust.uiSource in Settings' config.json, or --mode):
  auto        (the default) the ready-made build for this exact version,
              from the project's GitHub releases: the one for this processor
              (x86-64-v3, or generic), its minisign signature checked against
              tools/tessera-ui.pub and its sha256 against the signed manifest.
              Without one (a checkout with your own edits, a version not
              built yet, no network, no release key yet), built here instead.
  build       always built here (tools/build-native.sh), for any x86-64.
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
ROLLING = "tessera-ui-latest"  # main's builds (a rolling prerelease); tags v* get their own
MAX_DOWNLOAD = 200 * 1024 * 1024
MAX_MANIFEST = 256 * 1024
GIB = 1024 ** 3
NEED_RAM = 4 * GIB  # (a release build of tessera-ui: link-time optimisation, one codegen unit)
NEED_DISK = 4 * GIB

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


def ui_in_use(config=None):
    rust = (config if config is not None else load_config()).get("rust")
    return isinstance(rust, dict) and bool(rust.get("surfaces"))


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


def release_tags(commit, repo=None):
    """Where a build for COMMIT can be: its own release when a v* tag points
    at it (stable), then main's rolling one (latest)."""
    tags = (git("tag", "--points-at", commit, "--list", "v*", repo=repo) or "").split() if commit else []
    return sorted(tags, reverse=True) + [ROLLING]


GITHUB = re.compile(r"^(?:https://github\.com/|git@github\.com:|ssh://git@github\.com/)([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+?)(?:\.git)?/?$")


def releases_base(repo=None):
    """The download URL with {tag} in it, from the origin remote (GitHub
    only), or None. TESSERA_RELEASES sets it (tests: a file:// folder)."""
    if os.environ.get("TESSERA_RELEASES"):
        return os.environ["TESSERA_RELEASES"]
    m = GITHUB.match(git("remote", "get-url", "origin", repo=repo) or "")
    return f"https://github.com/{m[1]}/{m[2]}/releases/download/{{tag}}" if m else None


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

def download(url, path, max_bytes, timeout=300):
    """curl, with timeouts and retries. Raises Failed(kind=missing) for a
    file that isn't there, Failed(kind=network) for anything else."""
    local = url.startswith("file://")
    if not local and not url.startswith("https://"):
        raise Failed("only https downloads are allowed", "network")
    cmd = ["curl", "--fail", "--silent", "--show-error", "--location",
           "--proto", "=file" if local else "=https", "--proto-redir", "=https",
           "--connect-timeout", "10", "--max-time", str(timeout),
           "--retry", "3", "--retry-delay", "2", "--max-filesize", str(max_bytes),
           "--user-agent", "Tessera", "--write-out", "%{http_code}", "--output", path, url]
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

def fetch(tree, commit, variant, work, *, base=None, tags=None, pubkey=None, repo=None):
    """Downloads and checks the build for TREE into WORK (a new folder in
    the destination's file system): signature, manifest (tree, commit,
    variant), checksums, contents and --version. Returns
    {"files": {name: path}, "info": {...}}; raises Failed."""
    if variant is None:
        raise Failed("there are no ready-made builds for this processor (" + platform.machine() + ")", "missing")
    pubkey = pubkey if pubkey is not None else _read(pubkey_path())
    if not pubkey:
        raise Failed("this copy of Tessera has no release key yet (tools/tessera-ui.pub)", "missing")
    base = base if base is not None else releases_base(repo)
    if not base:
        raise Failed("this checkout doesn't come from GitHub, where the ready-made builds are", "missing")
    tags = tags if tags is not None else release_tags(commit, repo)
    os.makedirs(work, exist_ok=True)
    mpath = os.path.join(work, "manifest.json")
    found = None
    last = None
    for tag in tags:
        url = base.replace("{tag}", tag) + "/" + manifest_name(tree)
        try:
            download(url, mpath, MAX_MANIFEST, timeout=60)
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
    tpath = os.path.join(work, entry["file"])
    download(base.replace("{tag}", tag) + "/" + entry["file"], tpath, MAX_DOWNLOAD)
    if os.path.getsize(tpath) != entry.get("size", -1) or sha256_of(tpath) != entry.get("sha256"):
        raise Failed("the download doesn't match its signed checksum (damaged, or not what was published)")
    files = unpack(tpath, work, PROGRAMS)
    progs = entry.get("programs") or {}
    for name in PROGRAMS:
        if name not in files:
            raise Failed(f"the download has no {name}")
        want = (progs.get(name) or {}).get("sha256")
        if want and sha256_of(files[name]) != want:
            raise Failed(f"{name} in the download doesn't match its signed checksum")
        said = program_version(files[name])
        if not said.startswith(name + " "):
            raise Failed(f"{name} in the download doesn't run on this computer")
        want_v = (progs.get(name) or {}).get("version")
        if want_v and said != want_v:
            raise Failed(f"{name} in the download says it's {said}, not {want_v}")
    info = {"source": "ready-made", "variant": variant, "tree": tree, "commit": built_from,
            "version": manifest.get("version", ""), "channel": manifest.get("channel", ""), "tag": tag}
    return {"files": files, "info": info}


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


def swap_in(dest, files, info):
    """Installs FILES ({name: path}, already in DEST's file system) over the
    programs in DEST, one rename each; the replaced ones become <name>.prev.
    The installed program is always whole: the old one or the new one."""
    for name, src in files.items():
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
        _write_info(dest, name, dict(info, at=int(time.time())))
    _fsync_dir(dest)


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


def run_build(stage, tuned, ui, say):
    """tools/build-native.sh --stage: builds and copies the programs into
    STAGE (TESSERA_NATIVE_BUILDER: another command, for tests). Returns
    {name: path}; raises Failed."""
    cmd = os.environ.get("TESSERA_NATIVE_BUILDER")
    cmd = [cmd] if cmd else ["bash", os.path.join(REPO, "tools", "build-native.sh")]
    cmd += ["--stage", stage] + (["--tuned"] if tuned else []) + (["--ui"] if ui else [])
    say("STEP Building " + ("tessera-ui and tessera-events" if ui else "tessera-events") + " here" + (", tuned" if tuned else "")
        + (" (several minutes)" if ui else ""))
    r = subprocess.run(cmd, stdin=subprocess.DEVNULL, capture_output=True, text=True)
    sys.stderr.write(r.stdout + r.stderr)
    files = {n: os.path.join(stage, n) for n in PROGRAMS if os.path.isfile(os.path.join(stage, n))}
    if r.returncode != 0 or "tessera-events" not in files:
        tail = (r.stderr.strip() or r.stdout.strip()).splitlines()[-1:] or ["exit " + str(r.returncode)]
        raise Failed("it didn't build (" + tail[0] + ")", "build")
    return files


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


def _install(out, mode, tree, commit, edited, build_ui, names, offline, only_prev, dest, repo, say, builder, fetcher):
    cur = {n: read_info(dest, n) for n in names}
    if tree and not edited and all(cur[n].get("tree") == tree and accepts(mode, cur[n]) and os.path.exists(os.path.join(dest, n)) for n in names):
        info = cur[names[0]]
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
                got = fetcher(tree, commit, variant, work, repo=repo)
                swap_in(dest, got["files"], got["info"])
                info = got["info"]
                return dict(out, ok=True, changed=True, source="ready-made", variant=info["variant"], label=label(info),
                            message="Installed the ready-made build (" + label(info) + ")")
            except Failed as e:
                why = e.why
                if e.kind == "refused":
                    say("! The ready-made build was refused: " + e.why)
            finally:
                shutil.rmtree(work, ignore_errors=True)
        fallback = why
        say("No ready-made build: " + why + ". Building here instead.")
    tuned = mode == "buildTuned"
    stage = os.path.join(dest, ".built-" + secrets.token_hex(4))
    os.makedirs(stage)
    try:
        try:
            files = builder(stage, tuned, build_ui, say)
        except Failed as e:
            raise Failed(e.why + (" (and no ready-made build: " + fallback + ")" if fallback else ""), e.kind) from None
        info = {"source": "built-tuned" if tuned else "built", "variant": "native" if tuned else "generic",
                "tree": tree if not edited else None, "commit": commit, "edited": edited}
        swap_in(dest, files, info)
        res = dict(out, ok=True, changed=True, source=info["source"], variant=info["variant"], label=label(info),
                   fallback=fallback, message="Built here (" + label(info) + ")")
        if build_ui and "tessera-ui" not in files:
            # (tessera-events is in; the installed tessera-ui, if any, stays)
            res.update(ok=False, kind="build", why="tessera-ui didn't build",
                       message="Built tessera-events here; tessera-ui didn't build, so the shell draws everything in QML meanwhile"
                       if not os.path.exists(os.path.join(dest, "tessera-ui")) else
                       "Built tessera-events here; tessera-ui didn't build, so the installed one stays")
        return res
    finally:
        shutil.rmtree(stage, ignore_errors=True)


def plan(ref="HEAD", online=False, mode=None, dest=None, repo=None, config=None):
    """What install would do for REF (Software Update's check): {"mode",
    "variant", "available", "changes", "label"}. online: ask the release
    server whether the ready-made build is there (a HEAD request)."""
    config = load_config() if config is None else config
    mode = mode if mode in MODES else configured_mode(config)
    dest = dest or dest_dir()
    tree, commit, edited = native_tree(ref, repo)
    variant = detect_cpu()
    cur = read_info(dest, "tessera-ui") or read_info(dest, "tessera-events")
    changes = not (tree and cur.get("tree") == tree and accepts(mode, cur))
    out = {"mode": mode, "variant": variant, "tree": tree, "commit": commit, "changes": changes, "available": None}
    if mode == "build":
        return dict(out, label="tessera-ui: built here")
    if mode == "buildTuned":
        return dict(out, label="tessera-ui: built here, tuned")
    if not changes:
        return dict(out, available=True, label="tessera-ui: " + label(cur))
    why = ("your edits in native/" if edited and ref == "HEAD" else "not a git checkout" if not tree else
           "no ready-made build for this processor" if variant is None else
           "no release key yet" if not _read(pubkey_path()) else "")
    base = releases_base(repo)
    if not why and not base:
        why = "not from GitHub"
    if not why and online:
        tags = release_tags(commit, repo)
        seen = [exists_online(base.replace("{tag}", t) + "/" + manifest_name(tree)) for t in tags]
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
    return out


# ── Command line ─────────────────────────────────────────────────────────

def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["install", "fetch", "plan", "status", "restore", "cpu", "can-build"])
    ap.add_argument("--mode", choices=MODES)
    ap.add_argument("--ui", action="store_true", help="install: tessera-ui too when built here, even before Settings uses it")
    ap.add_argument("--offline", action="store_true", help="install: no download (a .prev build or building here)")
    ap.add_argument("--only-prev", action="store_true", help="install: only the .prev build, when it's this version's (going back)")
    ap.add_argument("--online", action="store_true", help="plan: ask the release server")
    ap.add_argument("--ref", default="HEAD")
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
        emit(s, "\n".join(f"{n}: {i['label']}" + (f" ({i.get('tree', '')[:12]})" if i.get("tree") else "") for n, i in s.items()) or "Nothing installed")
        return 0
    if a.command == "plan":
        p = plan(a.ref, online=a.online, mode=a.mode)
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
            got = fetch(tree, commit, detect_cpu(), work) if tree else None
            if not got:
                raise Failed("not a git checkout", "missing")
            emit(dict(got["info"], ok=True), "Checked: " + label(got["info"]))
            return 0
        except Failed as e:
            emit({"ok": False, "why": e.why, "kind": e.kind}, "No: " + e.why)
            return 1
        finally:
            shutil.rmtree(work, ignore_errors=True)
    say = (lambda s: print(s, file=sys.stderr, flush=True)) if a.progress else None
    r = install(a.mode, ui=a.ui, offline=a.offline, only_prev=a.only_prev, say=say)
    emit(r, r.get("message") or r.get("why"))
    return 0 if r["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
