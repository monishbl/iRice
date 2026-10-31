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
With the password:
  fingerprint on|off polkit|sudo|login
      a finger is (or isn't) accepted for password prompts (polkit), sudo, or
      the login screen (greetd, ly, SDDM): one marked line, `auth sufficient
      pam_fprintd.so`, first in that PAM file (greetd, ly, SDDM, LightDM or
      LXDM for the login screen; a vendor file in /usr/lib/pam.d
      is copied to /etc/pam.d first). "sufficient": no finger in time, or no
      pam_fprintd, and the password is asked as before. iRice's own lock
      screen needs none of this (its PAM file is in the shell).
  face setup <camera> | add | clear | list
      face unlock for iRice's lock screen, with Howdy (howdy-next): setup
      downloads Howdy's face models (once) and sets the infrared camera
      (a /dev/v4l/by-path, /dev/v4l/by-id or /dev/videoN path that exists) in
      /etc/howdy/config.ini; add, clear and list the faces of the person who
      asked (never anyone else's). iRice's lock screen uses its own PAM file;
      no system PAM file is touched.
  phonecam on|off
      a virtual camera for an Android phone's camera (Settings → Phone): the
      v4l2loopback module with one device, /dev/video42 "Phone camera",
      loaded now and at every start (modprobe.d and modules-load.d drop-ins
      named irice-phonecam.conf); off removes them and unloads it when no app
      is using it.
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
import glob
import json
import os
import pwd
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

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
    ufw allow 53317:53320/tcp comment "iRice sharing"
    ufw allow 53317/udp comment "iRice sharing (LocalSend)"
    ufw allow 5353/udp comment "mDNS"
    ufw allow 67/udp comment "iRice Quick Share hotspot (DHCP)"
    echo "@@dhcp"
elif command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port=53317-53320/tcp --add-port=53317/udp --add-service=mdns --add-service=dhcp
    firewall-cmd --reload
    echo "@@dhcp"
elif command -v nft >/dev/null 2>&1 && systemctl is-active --quiet nftables; then
    # An accept in a table of our own wouldn't do: nftables runs every base
    # chain on the input hook, and a drop in any of them is final. So the
    # rules go first in each input chain there is (iptables' own INPUT chains
    # are left to iptables), replacing any earlier ones of ours.
    chains=$(nft list chains | awk '$1 == "table" { f = $2; t = $3 } $1 == "chain" { c = $2 }
        /hook input/ && (f == "inet" || f == "ip" || f == "ip6") && !((f == "ip" || f == "ip6") && t == "filter" && c == "INPUT") { print f, t, c }')
    [ -n "$chains" ] || { echo "nftables has no input chain: nothing blocks sharing"; exit 0; }
    # Kept as a drop-in only where nftables.conf reads one (Arch's doesn't)
    persist=""
    if [ -d /etc/nftables.d ]; then
        if grep -qE '^[[:space:]]*include "/etc/nftables\.d/\*(\.nft)?"' /etc/nftables.conf 2>/dev/null; then
            persist=/etc/nftables.d/irice.nft
        elif grep -qE '^[[:space:]]*include "/etc/nftables\.d/\*\.conf"' /etc/nftables.conf 2>/dev/null; then
            persist=/etc/nftables.d/irice.conf
        fi
    fi
    echo "$chains" | while read -r f t c; do
        for h in $(nft -a list chain "$f" "$t" "$c" | sed -n 's/.*comment "irice sharing".*# handle \([0-9]*\).*/\1/p'); do
            nft delete rule "$f" "$t" "$c" handle "$h"
        done
        nft insert rule "$f" "$t" "$c" udp dport '{ 53317, 5353, 67 }' accept comment '"irice sharing"'
        nft insert rule "$f" "$t" "$c" tcp dport 53317-53320 accept comment '"irice sharing"'
    done
    if [ -n "$persist" ]; then
        {
            echo "# iRice: lets LocalSend and Quick Share through (Settings → Sharing)"
            echo "$chains" | while read -r f t c; do
                echo "insert rule $f $t $c udp dport { 53317, 5353, 67 } accept comment \"irice sharing\""
                echo "insert rule $f $t $c tcp dport 53317-53320 accept comment \"irice sharing\""
            done
        } >/etc/.irice-nftables.tmp
        mv /etc/.irice-nftables.tmp "$persist"
        # (a file nftables can't load at boot would leave no firewall at all)
        if nft -c -f /etc/nftables.conf >/dev/null 2>&1; then
            echo "@@dhcp"
            echo "Saved in $persist: it stays after a restart"
            exit 0
        fi
        rm -f "$persist"
    fi
    echo "@@dhcp"
    echo "@@runtime"
    echo "Allowed until the next restart: /etc/nftables.conf doesn't include /etc/nftables.d. To keep it, add the same rules to /etc/nftables.conf"
elif command -v iptables >/dev/null 2>&1 && systemctl is-active --quiet iptables; then
    for ipt in iptables ip6tables; do
        [ "$ipt" = iptables ] || systemctl is-active --quiet ip6tables || continue
        command -v "$ipt" >/dev/null 2>&1 || continue
        for r in "-p udp --dport 5353" "-p udp --dport 53317" "-p udp --dport 67" "-p tcp --dport 53317:53320"; do
            # ($r splits into its words)
            $ipt -C INPUT $r -m comment --comment "irice sharing" -j ACCEPT 2>/dev/null ||
                $ipt -I INPUT $r -m comment --comment "irice sharing" -j ACCEPT
        done
    done
    echo "@@dhcp"
    echo "@@runtime"
    echo "Allowed until the next restart. To keep it: sudo iptables-save -f /etc/iptables/iptables.rules (and ip6tables-save -f /etc/iptables/ip6tables.rules)"
else
    echo "No firewall (ufw, firewalld, nftables or iptables) is turned on" >&2
    exit 1
fi
'''
FIREWALL_SHOWN = ("ufw allow 53317:53320/tcp; ufw allow 53317/udp; ufw allow 5353/udp; ufw allow 67/udp "
                  "(or firewalld: --add-port=53317-53320/tcp --add-port=53317/udp --add-service=mdns --add-service=dhcp)")
# What the page shows before the fix runs, by firewall (sharing-check.py's kind)
FIREWALL_SHOWN_BY = {
    "ufw": "ufw allow 53317:53320/tcp; ufw allow 53317/udp; ufw allow 5353/udp; ufw allow 67/udp",
    "firewalld": "firewall-cmd --permanent --add-port=53317-53320/tcp --add-port=53317/udp --add-service=mdns --add-service=dhcp; firewall-cmd --reload",
    "nftables": ("nft insert rule <each input chain> tcp dport 53317-53320 accept, and udp dport { 53317, 5353, 67 } accept "
                 "(kept in /etc/nftables.d when nftables.conf includes it, else until the next restart)"),
    "iptables": "iptables -I INPUT -p tcp --dport 53317:53320 -j ACCEPT, and udp 53317, 5353 and 67 (until the next restart)",
}

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

# NetworkManager, unless iwd or systemd-networkd runs the network instead:
# switching over is a choice (the installer offers it), not a fix
NETWORK_SCRIPT = r'''set -e
if ! systemctl is-enabled --quiet NetworkManager; then
    for s in iwd systemd-networkd; do
        if systemctl is-active --quiet "$s"; then
            echo "$s runs your network; iRice's installer can switch it to NetworkManager" >&2
            exit 1
        fi
    done
fi
systemctl enable --now NetworkManager
'''
NETWORK_SHOWN = "systemctl enable --now NetworkManager"

FIXES = {
    "networkmanager": ["/bin/sh", "-c", NETWORK_SCRIPT],
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


# ── fingerprint (Settings → Lock Screen → Fingerprint) ─────────────────
FP_MARK = "# iRice fingerprint"
# max-tries / timeout: a finger, or the password after a few seconds (PAM
# asks one thing at a time; the login screen waits longest)
FP_LINES = {"polkit": "auth       sufficient   pam_fprintd.so max-tries=2 timeout=10   " + FP_MARK,
            "sudo": "auth       sufficient   pam_fprintd.so max-tries=2 timeout=10   " + FP_MARK,
            "login": "auth       sufficient   pam_fprintd.so max-tries=2 timeout=15   " + FP_MARK}
# (GDM isn't here: it asks for a finger by itself, gdm-fingerprint, once one is added)
FP_FILES = {"polkit": ["polkit-1"], "sudo": ["sudo"], "login": ["greetd", "ly", "sddm", "lightdm", "lxdm"]}
PAM_ETC, PAM_VENDOR = "/etc/pam.d", "/usr/lib/pam.d"
PAM_FPRINTD = ("/usr/lib/security/pam_fprintd.so", "/lib/security/pam_fprintd.so")


def pam_without_fingerprint(text):
    return "".join(l for l in text.splitlines(keepends=True) if FP_MARK not in l)


def pam_with_fingerprint(text, line):
    """The line first among the PAM file's rules (after its header comments
    and blank lines), any earlier one of ours gone."""
    lines = pam_without_fingerprint(text).splitlines(keepends=True)
    at = 0
    while at < len(lines) and (lines[at].strip() == "" or lines[at].lstrip().startswith("#")):
        at += 1
    return "".join(lines[:at] + [line + "\n"] + lines[at:])


def fingerprint(on, where, etc=PAM_ETC, vendor=PAM_VENDOR, module=PAM_FPRINTD):
    """Returns the files changed."""
    if where not in FP_FILES:
        raise Refused("fingerprint: polkit, sudo or login")
    if on and not any(os.path.exists(m) for m in module):
        raise Refused("pam_fprintd isn't installed (fprintd)")
    changed = []
    for name in FP_FILES[where]:
        dest = os.path.join(etc, name)
        src = dest if os.path.exists(dest) else os.path.join(vendor, name)
        if not os.path.exists(src):
            continue  # (that login screen isn't installed)
        with open(src) as f:
            text = f.read()
        new = pam_with_fingerprint(text, FP_LINES[where]) if on else pam_without_fingerprint(text)
        if new != text or (on and src != dest):
            write_root(dest, new.encode(), 0o644)
            changed.append(dest)
    return changed


# ── face (Settings → Lock Screen → Face) ─────────────────────────────────
HOWDY_CONFIG = "/etc/howdy/config.ini"
CAMERA_PATH = re.compile(r"^/dev/(v4l/by-(path|id)/[\w:.\-]+|video\d{1,3})$")


def howdy_device(text, device):
    """config.ini with [video] device_path set, everything else as it was."""
    lines = text.splitlines()
    out, section, done = [], "", False
    for line in lines:
        m = re.match(r"^\s*\[(.+)\]\s*$", line)
        if m:
            if section == "video" and not done:
                out.append(f"device_path = {device}")
                done = True
            section = m.group(1).strip().lower()
        elif section == "video" and re.match(r"^\s*device_path\s*=", line):
            if not done:
                out.append(f"device_path = {device}")
                done = True
            continue
        out.append(line)
    if not done:
        if section != "video":
            out += ["", "[video]"]
        out.append(f"device_path = {device}")
    return "\n".join(out) + "\n"


def face(args, uid, run=subprocess.run, config=HOWDY_CONFIG, exists=os.path.exists, which=shutil.which,
         models="/usr/share/howdy/models"):
    """Returns the exit status; howdy's own output goes straight through."""
    howdy = which("howdy")
    if not howdy:
        raise Refused("Howdy isn't installed (howdy-next)")
    user = pwd.getpwuid(uid).pw_name
    if args[:1] == ["setup"] and len(args) == 2:
        device = args[1]
        if not CAMERA_PATH.match(device) or not exists(device):
            raise Refused(f"not a camera: {device}")
        if not glob.glob(os.path.join(models, "*.onnx")):
            r = run([howdy, "download-models"])
            if r.returncode != 0:
                raise Refused("Howdy's face models couldn't be downloaded (check the internet connection)")
        try:
            with open(config) as f:
                text = f.read()
        except FileNotFoundError:
            text = ""
        write_root(config, howdy_device(text, device).encode(), 0o644)
        return 0
    if args == ["add"]:
        return run([howdy, "-U", user, "--plain", "-y", "add", "iRice " + time.strftime("%Y-%m-%d %H.%M")]).returncode
    if args == ["clear"]:
        return run([howdy, "-U", user, "-y", "clear"]).returncode
    if args == ["list"]:
        return run([howdy, "-U", user, "--plain", "list"]).returncode
    raise Refused("face: setup <camera> | add | clear | list")


# ── phonecam (Settings → Phone → Use as webcam) ─────────────────────────
PHONECAM_DEV = 42
PHONECAM_OPTIONS = f'options v4l2loopback devices=1 video_nr={PHONECAM_DEV} card_label="Phone camera" exclusive_caps=1\n'
PHONECAM_FILES = ("/etc/modprobe.d/irice-phonecam.conf", "/etc/modules-load.d/irice-phonecam.conf")


def phonecam(on, etc="/etc", run=subprocess.run, dev_dir="/dev"):
    """Returns what changed. The module itself comes with the kernel or
    v4l2loopback-dkms (Software Update installs it)."""
    probe, load = (os.path.join(etc, f[len("/etc/"):]) for f in PHONECAM_FILES)
    changed = []
    if on:
        if run(["modinfo", "v4l2loopback"], capture_output=True).returncode != 0:
            raise Refused("v4l2loopback isn't installed (v4l2loopback-dkms)")
        for path, text in ((probe, PHONECAM_OPTIONS), (load, "v4l2loopback\n")):
            write_root(path, text.encode(), 0o644)
            changed.append(path)
        if not os.path.exists(os.path.join(dev_dir, f"video{PHONECAM_DEV}")):
            # loaded before with other options: again, with ours (only when no app has it open)
            run(["modprobe", "-r", "v4l2loopback"], capture_output=True)
            r = run(["modprobe", "v4l2loopback"], capture_output=True, text=True)
            if r.returncode != 0:
                raise Refused("another app is using a virtual camera: close it (or restart), then try again")
    else:
        for path in (probe, load):
            if os.path.exists(path):
                os.unlink(path)
                changed.append(path)
        run(["modprobe", "-r", "v4l2loopback"], capture_output=True)
    return changed


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
        if cmd == "fingerprint" and len(args) == 2 and args[0] in ("on", "off"):
            for d in fingerprint(args[0] == "on", args[1]):
                print("changed", d)
            return 0
        if cmd == "face" and args:
            return face(args, uid)
        if cmd == "phonecam" and len(args) == 1 and args[0] in ("on", "off"):
            for d in phonecam(args[0] == "on"):
                print("changed", d)
            return 0
        if cmd == "setup" and len(args) == 1:
            for d in setup(args[0], uid):
                print("installed", d)
            return 0
        raise Refused("usage: irice-root wallpaper <path> | fix <id> | wifi-helper | fingerprint on|off <where> | phonecam on|off | face … | "
                      "update … | setup <folder>")
    except Refused as e:
        print(f"irice-root: {e}", file=sys.stderr)
        return REFUSED_EXIT
    except OSError as e:
        print(f"irice-root: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
