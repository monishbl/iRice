#!/usr/bin/python3 -I
"""Everything iRice does as root, as one fixed program: installed (by the
next update, or the installer) to /usr/lib/irice/irice-root, owned by root,
with what it runs next to it, so nothing it starts lives in your home.
Started through pkexec; each subcommand is a polkit action of its own
(org.irice.root.<subcommand>, matched on the first argument:
shell/assets/polkit/org.irice.root.policy).

Without a password, for an administrator (wheel) at this computer
(shell/assets/polkit/49-irice.rules):
  wallpaper <path>   copies one image (JPEG, PNG or WebP, up to 64 MB, read
                     as you) to /usr/share/irice/wallpaper.jpg, the login
                     screen's copy
  fix <id>           one of the fixed fixes below (system services, the
                     firewall, Bluetooth's pairing prompts); nothing
                     free-form ever runs
  wifi-helper        Quick Share's Wi-Fi helper (qs-wifi-helper.py, next to
                     this file): JSON on stdin and stdout
With the password, once, at the start:
  update [--snapshot snapper|timeshift] [--system] [--ignore a,b]
         [--install a,b] [--remove a,b] [--setup <shell folder>] [--broker]
      the whole system part of an update (sysupdate.py starts it): snapshot,
      this helper's own files (--setup), pacman -Syu, iRice's packages,
      dropped ones. "@@step" lines say where it is; exit codes say which
      step failed (20 snapshot, 21 system, 22 install, 23 remove). Then
      "@@root-done", and with --broker it stays for the AUR step: the AUR
      helper builds as you and runs `irice-sudo pacman …` for the root
      parts, which sysupdate.py passes here over stdin (a socket only the
      update has). Only pacman runs, from /usr/bin, its arguments checked
      (check_pacman); output and exit code go back the same way. It ends
      when sysupdate.py closes the socket, so nothing outlives the update.
  setup <shell folder>
      the same files as --setup, on their own (install.sh, with sudo)

Files are read as you (the person who started pkexec or sudo), never as
root: a symlink in your home can't make root read anything you can't.
"""
import contextlib
import json
import os
import pwd
import re
import signal
import socket
import subprocess
import sys
import tempfile

LIB = "/usr/lib/irice"
PACMAN = "/usr/bin/pacman"
LOGIN_WALLPAPER = "/usr/share/irice/wallpaper.jpg"
MAX_IMAGE = 64 * 1024 * 1024
MAX_SOURCE = 1024 * 1024
PREFIX = "/"  # where FILES go (tests: a folder)

# What setup installs: from the shell's folder (the repo's shell/, or its
# installed copy) → where, with which mode
FILES = [
    ("assets/scripts/irice-root.py", LIB + "/irice-root", 0o755),
    ("assets/scripts/quickshare/qs-wifi-helper.py", LIB + "/qs-wifi-helper.py", 0o644),
    ("assets/polkit/org.irice.root.policy", "/usr/share/polkit-1/actions/org.irice.root.policy", 0o644),
    ("assets/polkit/49-irice.rules", "/etc/polkit-1/rules.d/49-irice.rules", 0o644),
]


class Refused(Exception):
    """Arguments this helper won't act on; the message says why."""


REFUSED_EXIT = 64  # (an older helper without that subcommand or fix: callers ask as before)


# ── The fixes (System health, Settings → Sharing) ───────────────────────
FIREWALL_SCRIPT = r'''set -e
if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "^Status: active"; then
    ufw allow 53317:53319/tcp comment "iRice sharing"
    ufw allow 53317/udp comment "iRice sharing (LocalSend)"
    ufw allow 5353/udp comment "mDNS"
elif command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port=53317-53319/tcp --add-port=53317/udp --add-service=mdns
    firewall-cmd --reload
else
    echo "No firewall (ufw or firewalld) is turned on" >&2
    exit 1
fi
'''
FIREWALL_SHOWN = ("ufw allow 53317:53319/tcp; ufw allow 53317/udp; ufw allow 5353/udp "
                  "(or firewalld: --add-port=53317-53319/tcp --add-port=53317/udp --add-service=mdns)")

# BlueZ plugins that make phones ask to pair for Quick Share (sharing-check.py)
# rap: Ranging; vcp: volume (with volume offset, audio input); micp: microphone;
# mcp: media control; bass: broadcast audio scan
PLUGINS = ("rap", "vcp", "micp", "mcp", "bass")
DROPIN = "/etc/systemd/system/bluetooth.service.d/irice-no-rap.conf"
PAIRING_SCRIPT = r'''set -e
NOPLUGIN=@PLUGINS@
exe=$(systemctl show -p ExecStart --value bluetooth | sed -n 's/.*path=\([^ ;]*\).*/\1/p')
[ -x "$exe" ] || exe=/usr/lib/bluetooth/bluetoothd
mkdir -p /etc/systemd/system/bluetooth.service.d
printf '# iRice: no Ranging or LE Audio control profiles, which make phones ask to pair for Quick Share\n[Service]\nExecStart=\nExecStart=%s --noplugin=%s\n' "$exe" "$NOPLUGIN" > /etc/systemd/system/bluetooth.service.d/irice-no-rap.conf
systemctl daemon-reload
systemctl restart bluetooth
'''
PAIRING_SCRIPT = PAIRING_SCRIPT.replace("@PLUGINS@", ",".join(PLUGINS))
PAIRING_SHOWN = f"bluetoothd --noplugin={','.join(PLUGINS)} (a drop-in in {os.path.dirname(DROPIN)}), then restart Bluetooth"

FIXES = {
    "networkmanager": ["systemctl", "enable", "--now", "NetworkManager"],
    "bluetooth": ["systemctl", "enable", "--now", "bluetooth"],
    "avahi": ["systemctl", "enable", "--now", "avahi-daemon"],
    "upower": ["systemctl", "enable", "--now", "upower"],
    "profiles": ["systemctl", "enable", "--now", "power-profiles-daemon"],
    "firewall": ["/bin/sh", "-c", FIREWALL_SCRIPT],
    "btpairing": ["/bin/sh", "-c", PAIRING_SCRIPT],
}


# ── Acting as the person who asked ──────────────────────────────────────
def caller_uid():
    """Who started pkexec (or sudo, for install.sh); root if neither."""
    for key in ("PKEXEC_UID", "SUDO_UID"):
        v = os.environ.get(key, "")
        if v.isdigit():
            return int(v)
    return 0


@contextlib.contextmanager
def as_user(uid):
    """Reads files with that person's rights (effective ids only, given back
    afterwards). Unprivileged (tests), it changes nothing."""
    if os.geteuid() != 0 or uid == 0:
        yield
        return
    pw = pwd.getpwuid(uid)
    groups = os.getgroups()
    os.setgroups(os.getgrouplist(pw.pw_name, pw.pw_gid))
    os.setegid(pw.pw_gid)
    os.seteuid(uid)
    try:
        yield
    finally:
        os.seteuid(0)
        os.setegid(0)
        os.setgroups(groups)


def read_as(uid, path, limit):
    """A regular file's bytes, read as uid (never a FIFO or a device)."""
    with as_user(uid):
        fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        st = os.fstat(fd)
        if not (st.st_mode & 0o170000 == 0o100000):
            raise Refused(f"{path} isn't a regular file")
        if st.st_size > limit:
            raise Refused(f"{path} is too big")
        data = b""
        while len(data) <= limit:
            chunk = os.read(fd, 1 << 20)
            if not chunk:
                break
            data += chunk
        if len(data) > limit:
            raise Refused(f"{path} is too big")
        return data
    finally:
        os.close(fd)


def write_root(dest, data, mode):
    """Replaces dest whole (a temporary file next to it, then a rename), so
    nothing ever reads half a file; parents are made 0755 when missing."""
    d = os.path.dirname(dest)
    os.makedirs(d, mode=0o755, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".irice-")
    try:
        os.fchmod(fd, mode)
        os.write(fd, data)
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.rename(tmp, dest)
    except BaseException:
        if fd >= 0:
            os.close(fd)
        with contextlib.suppress(OSError):
            os.unlink(tmp)
        raise


# ── wallpaper ───────────────────────────────────────────────────────────
def image_kind(data):
    if data[:3] == b"\xff\xd8\xff":
        return "jpeg"
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        return "webp"
    return ""


def check_image_path(path):
    if not isinstance(path, str) or not path.startswith("/") or len(path) > 4096 or "\0" in path or "\n" in path:
        raise Refused("give the image's full path")
    return path


def wallpaper(path, uid, dest=LOGIN_WALLPAPER):
    path = check_image_path(path)
    data = read_as(uid, path, MAX_IMAGE)
    if not data or not image_kind(data):
        raise Refused("that isn't a JPEG, PNG or WebP image")
    d = os.path.dirname(dest)
    if not os.path.isdir(d) or os.path.islink(d):
        raise Refused("the iRice login screen isn't installed (./install.sh --greeter)")
    write_root(dest, data, 0o644)


# ── fix ─────────────────────────────────────────────────────────────────
def fix_command(id):
    if id not in FIXES:
        raise Refused("there's no fix called that")
    return FIXES[id]


# ── setup: this helper's own files ──────────────────────────────────────
def check_folder(src):
    if not isinstance(src, str) or not src.startswith("/") or "\0" in src or "\n" in src:
        raise Refused("give the shell's folder as a full path")
    return src


def setup(src, uid, prefix=None):
    """Installs FILES from the shell's folder, only the ones that changed.
    Returns the destinations written."""
    src = check_folder(src)
    prefix = PREFIX if prefix is None else prefix
    written = []
    for rel, dest, mode in FILES:
        data = read_as(uid, os.path.join(src, rel), MAX_SOURCE)
        dest = os.path.join(prefix, dest.lstrip("/"))
        try:
            with open(dest, "rb") as f:
                same = f.read() == data and (os.stat(dest).st_mode & 0o7777) == mode
        except OSError:
            same = False
        if not same:
            write_root(dest, data, mode)
            written.append(dest)
    return written


# ── update: the system part, and the broker for the AUR step ────────────
# The pacman steps as one inline script, as before (sysupdate.py's protocol):
# $1 snapshot tool, $2 held packages, $3 to install, $4 to remove,
# $5 1 = pacman -Syu
UPDATE_SCRIPT = r"""
if [ "$5" = 1 ]; then
    echo "@@step system"
    if [ -n "$2" ]; then pacman -Syu --noconfirm --ignore "$2" || exit 21
    else pacman -Syu --noconfirm || exit 21; fi
fi
if [ -n "$3" ]; then
    echo "@@step packages"
    pacman -S --needed --noconfirm -- $(echo "$3" | tr , ' ') || exit 22
fi
if [ -n "$4" ]; then
    echo "@@step remove"
    pacman -Rns --noconfirm -- $(echo "$4" | tr , ' ') || exit 23
fi
exit 0
"""
SNAPSHOT = {
    "snapper": ["snapper", "-c", "root", "create", "-d", "Before iRice update", "-c", "number"],
    "timeshift": ["timeshift", "--create", "--comments", "Before iRice update", "--scripted"],
}
# A package name: never starting with "-" (it would pass as a pacman option)
NAME = re.compile(r"^[a-z0-9@_+][a-z0-9@._+-]*$")


def parse_update(args):
    """update's arguments, strictly: known flags only, package names checked."""
    p = {"snapshot": "", "system": False, "ignore": [], "install": [], "remove": [], "setup": "", "broker": False}
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("--system", "--broker"):
            p[a[2:]] = True
            i += 1
            continue
        if a not in ("--snapshot", "--ignore", "--install", "--remove", "--setup") or i + 1 >= len(args):
            raise Refused(f"unknown argument: {a}")
        v = args[i + 1]
        i += 2
        if a == "--snapshot":
            if v not in SNAPSHOT:
                raise Refused("unknown snapshot tool")
            p["snapshot"] = v
        elif a == "--setup":
            p["setup"] = check_folder(v)
        else:
            names = [n for n in v.split(",") if n]
            bad = [n for n in names if not NAME.match(n)]
            if bad:
                raise Refused("not a package name: " + ", ".join(bad))
            p[a[2:]] = names
    return p


# Pacman options that point it somewhere else: refused, or only allowed
# with their default value (yay and paru pass --config /etc/pacman.conf)
DEFAULTS = {"--config": ("/etc/pacman.conf",), "--dbpath": ("/var/lib/pacman", "/var/lib/pacman/"), "--root": ("/",)}
REFUSED = {"--cachedir", "--hookdir", "--gpgdir", "--logfile", "--sysroot"}
# Long options that take a value ("--x v" or "--x=v")
VALUED = {"--config", "--dbpath", "--root", "--cachedir", "--hookdir", "--gpgdir", "--logfile", "--sysroot", "--arch",
          "--color", "--assume-installed", "--ignore", "--ignoregroup", "--overwrite", "--print-format", "--ask"}
OPERATIONS = {"--database": "D", "--remove": "R", "--sync": "S", "--upgrade": "U",
              "--query": "Q", "--deptest": "T", "--files": "F", "--version": "V", "--help": "h"}
ALLOWED_OPS = {"D", "R", "S", "U"}


def check_pacman(argv):
    """The AUR helper's command, if it's a pacman we run: pacman (or
    /usr/bin/pacman) installing, upgrading, removing or marking packages,
    not pointed at another config, root, database, hooks, keys, cache or
    log. Returns the arguments after "pacman"."""
    if not isinstance(argv, list) or not argv or not all(isinstance(a, str) for a in argv):
        raise Refused("not a command")
    if argv[0] not in ("pacman", PACMAN):
        raise Refused(f"only pacman runs as root here, not {argv[0]}")
    args = argv[1:]
    if any("\0" in a for a in args):
        raise Refused("a NUL in an argument")
    ops = []
    i = 0
    while i < len(args):
        a = args[i]
        i += 1
        if a == "--":
            break  # the rest are package names or files
        if a.startswith("--"):
            name, eq, value = a.partition("=")
            # pacman takes abbreviations (--logf for --logfile): the risky
            # ones only written out in full
            if name not in VALUED and name not in OPERATIONS and \
                    any(o.startswith(name) and o != name for o in REFUSED | set(DEFAULTS)):
                raise Refused(f"write pacman {name} out in full")
            if name in VALUED and not eq:
                if i >= len(args):
                    raise Refused(f"{name} needs a value")
                value = args[i]
                i += 1
            if name in REFUSED:
                raise Refused(f"pacman {name} isn't allowed")
            if name in DEFAULTS and value not in DEFAULTS[name]:
                raise Refused(f"pacman {name} {value} isn't allowed (only {DEFAULTS[name][0]})")
            if name in OPERATIONS:
                ops.append(OPERATIONS[name])
        elif a.startswith("-") and len(a) > 1:
            for c in a[1:]:
                if c in "br":  # (they take a path: --dbpath, --root)
                    raise Refused(f"pacman -{c} isn't allowed")
                if c in "DRSUQTFV":
                    ops.append(c)
    if len(ops) != 1 or ops[0] not in ALLOWED_OPS:
        raise Refused("only pacman -S, -U, -R or -D runs as root here")
    return args


def _send(sock, obj):
    sock.sendall((json.dumps(obj) + "\n").encode())


def serve(sock, log=None, pacman=None):
    """The broker: one request per line from sysupdate.py ({"argv", "cwd"}),
    answered with {"out": text} lines and one {"exit": code}. Requests run
    one after another. Ends when the socket closes."""
    pacman = pacman or PACMAN
    reader = sock.makefile("rb")
    for line in reader:
        try:
            req = json.loads(line)
            args = check_pacman(req.get("argv"))
            cwd = req.get("cwd")
            if not isinstance(cwd, str) or not cwd.startswith("/") or not os.path.isdir(cwd):
                cwd = "/"
        except (ValueError, AttributeError, Refused) as e:
            msg = f"irice-root: refused: {e}\n"
            if log:
                log.write(msg)
                log.flush()
            try:
                _send(sock, {"out": msg})
                _send(sock, {"exit": 1})
            except OSError:
                return
            continue
        if log:
            log.write("$ pacman " + " ".join(args) + "\n")
            log.flush()
        p = subprocess.Popen([pacman] + args, cwd=cwd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, env=dict(os.environ, LANG="C", LC_ALL="C"))
        gone = False  # sysupdate.py went away: pacman still finishes
        while True:
            chunk = os.read(p.stdout.fileno(), 65536)
            if not chunk:
                break
            text = chunk.decode(errors="replace")
            if log:
                log.write(text)
                log.flush()
            if not gone:
                try:
                    _send(sock, {"out": text})
                except OSError:
                    gone = True
        code = p.wait()
        if gone:
            return
        try:
            _send(sock, {"exit": code})
        except OSError:
            return


def update(args, uid, out=sys.stdout):
    """Returns the exit code (see the top of this file)."""
    p = parse_update(args)
    # Nothing sysupdate.py does (exiting, closing its end) may stop a transaction
    for s in (signal.SIGPIPE, signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(s, signal.SIG_IGN)

    def say(line):
        out.write(line + "\n")
        out.flush()
    say("@@authenticated")
    if p["snapshot"]:
        say("@@step snapshot")
        if subprocess.run(SNAPSHOT[p["snapshot"]], stdout=out, stderr=subprocess.STDOUT).returncode != 0:
            return 20
    if p["setup"]:
        # Never a reason to stop the update: the old behaviour carries on
        say("@@step setup")
        try:
            done = setup(p["setup"], uid)
            say("Installed " + ", ".join(done) if done else "iRice's system helper is up to date")
        except (OSError, Refused) as e:
            say(f"iRice's system helper wasn't installed: {e}")
    code = subprocess.run(["/bin/sh", "-c", UPDATE_SCRIPT, "sh", p["snapshot"], ",".join(p["ignore"]),
                           ",".join(p["install"]), ",".join(p["remove"]), "1" if p["system"] else "0"],
                          stdout=out, stderr=subprocess.STDOUT).returncode
    if code != 0:
        return code
    say("@@root-done")
    if p["broker"]:
        try:
            sock = socket.socket(fileno=os.dup(0))
        except OSError:
            say("irice-root: no channel for the AUR step")
            return 0
        if sock.type != socket.SOCK_STREAM:
            return 0
        serve(sock, out)
    return 0


# ── wifi-helper ─────────────────────────────────────────────────────────
def wifi_helper():
    helper = os.path.join(LIB, "qs-wifi-helper.py")
    os.execv(sys.executable, [sys.executable, "-I", helper])


def main(argv):
    if not argv:
        print(__doc__, file=sys.stderr)
        return 2
    cmd, args = argv[0], argv[1:]
    try:
        if os.geteuid() != 0:
            raise Refused("this runs as root (through pkexec)")
        uid = caller_uid()
        if cmd == "wallpaper" and len(args) == 1:
            wallpaper(args[0], uid)
            return 0
        if cmd == "fix" and len(args) == 1:
            c = fix_command(args[0])
            os.execvp(c[0], c)
        if cmd == "wifi-helper" and not args:
            wifi_helper()
        if cmd == "update":
            return update(args, uid)
        if cmd == "setup" and len(args) == 1:
            for d in setup(args[0], uid):
                print("installed", d)
            return 0
        raise Refused("usage: irice-root wallpaper <path> | fix <id> | wifi-helper | update … | setup <folder>")
    except Refused as e:
        print(f"irice-root: {e}", file=sys.stderr)
        return REFUSED_EXIT
    except OSError as e:
        print(f"irice-root: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
