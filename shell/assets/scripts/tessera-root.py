#!/usr/bin/python3 -I
"""Everything Tessera does as root, as one fixed program: installed (by the
next update, or the installer) to /usr/lib/tessera/tessera-root, owned by root,
with what it runs next to it, so nothing it starts lives in your home.
Started through pkexec; each subcommand is a polkit action of its own
(org.tessera.root.<subcommand>, matched on the first argument:
shell/assets/polkit/org.tessera.root.policy).

Without a password, for an administrator (wheel) at this computer
(shell/assets/polkit/49-tessera.rules):
  wallpaper <path>   copies one image (JPEG, PNG or WebP, up to 64 MB, read
                     as you) to /usr/share/tessera/wallpaper.jpg, the login
                     screen's copy
  login-look <json>  the login screen's look (Settings → Lock Screen → Login
                     screen) as /usr/share/tessera/greeter.json, which it can
                     read (your home it can't): a style, the look, light or
                     dark, Material's colours and the clock's style, each
                     checked against its few values; nothing else is taken
  fix <id>          one of the fixed fixes below (system services,
                     Bluetooth's pairing prompts); nothing free-form ever
                     runs. (fix firewall, from shells before sharing-ports:
                     the same as sharing-ports open)
  sharing-ports open [<set>...] | close
                     the firewall's way in for nearby sharing, only while
                     it's needed (services/SharingPorts.qml): open lets in
                     exactly these of the fixed sets in PORT_SETS (all of
                     them without one), close none. Runtime rules where the
                     firewall has them, so a restart starts closed; both
                     also take out the permanent rules the fix before this
                     left
  wifi-helper        Quick Share's Wi-Fi helper (qs-wifi-helper.py, next to
                     this file): JSON on stdin and stdout
  priority on|off    the shell's priority with ananicy-cpp: its fixed rule
                     in /etc/ananicy.d (on), or none and a marker in
                     /etc/tessera that keeps updates from putting it back (off)
With the password:
  fingerprint on|off polkit|sudo|login
      a finger is (or isn't) accepted for password prompts (polkit), sudo, or
      the login screen (greetd, ly, SDDM): one marked line, `auth sufficient
      pam_fprintd.so`, first in that PAM file (greetd, ly, SDDM, LightDM or
      LXDM for the login screen; a vendor file in /usr/lib/pam.d
      is copied to /etc/pam.d first). "sufficient": no finger in time, or no
      pam_fprintd, and the password is asked as before. Tessera's own lock
      screen needs none of this (its PAM file is in the shell).
  face setup <camera> | add | clear | list
      face unlock for Tessera's lock screen, with Howdy (howdy-next): setup
      downloads Howdy's face models (once) and sets the infrared camera
      (a /dev/v4l/by-path, /dev/v4l/by-id or /dev/videoN path that exists) in
      /etc/howdy/config.ini; add, clear and list the faces of the person who
      asked (never anyone else's). Tessera's lock screen uses its own PAM file;
      no system PAM file is touched.
  phonecam on|off
      a virtual camera for an Android phone's camera (Settings → Phone): the
      v4l2loopback module with one device, /dev/video42 "Phone camera",
      loaded now and at every start (modprobe.d and modules-load.d drop-ins
      named tessera-phonecam.conf); off removes them and unloads it when no app
      is using it.
With the password, once, at the start:
  update [--snapshot snapper|timeshift] [--system] [--ignore a,b]
         [--install a,b] [--remove a,b] [--downgrade f,g] [--setup <shell folder>] [--broker]
      the whole system part of an update (sysupdate.py starts it): snapshot,
      this helper's own files (--setup), pacman -Syu, Tessera's packages,
      dropped ones. "@@step" lines say where it is; exit codes say which
      step failed (20 snapshot, 21 system, 22 install, 23 remove, 24
      downgrade). --downgrade: going back to before an update, pacman -U of
      earlier package files, only from pacman's cache, your yay/paru cache
      or Arch's archive (archive.archlinux.org). Then
      "@@root-done", and with --broker it stays for the AUR step: the AUR
      helper builds as you and runs `tessera-sudo pacman …` for the root
      parts, which sysupdate.py passes here over stdin (a socket only the
      update has). Only pacman runs, from /usr/bin, its arguments checked
      (check_pacman); output and exit code go back the same way. It ends
      when sysupdate.py closes the socket, so nothing outlives the update.
  quickshare-trust on|off
      Quick Share's Wi-Fi helper starts without the password for you (a
      polkit rule naming you, at this computer, in the session on screen);
      off removes it. /etc/tessera/quickshare-trusted says it's on
  bootmenu on|off
      snapshots in GRUB's boot menu: grub-btrfs's daemon (grub-btrfsd), on
      or off; on also writes the submenu now
  bootsplash breathe|orbit|frost|bloom|system|none
      the boot animation (Plymouth's theme; Settings → Lock & Login Screen):
      one of Tessera's (copied from /usr/lib/tessera/plymouth, where setup put
      them, to /usr/share/plymouth/themes/tessera-<id>), the system's own (the
      theme set before Tessera first changed it, kept in
      /etc/tessera/bootsplash.json) or none (Plymouth's text theme); then the
      boot image is rebuilt (mkinitcpio -P). The boot images are copied
      first: if the rebuild fails, they and the theme setting are put back,
      and the error says why
  setup <shell folder>
      the same files as --setup, on their own (install.sh, with sudo), and
      the boot animations (assets/plymouth) into /usr/lib/tessera/plymouth
      Both also refresh the login screen's own copy of Tessera
      (/usr/share/tessera/shell and material, and greeter.lua; only where
      ./install.sh --greeter made one) from the same version: the shell
      folder's checkout, or the update's staged one (<folder>/tree)
      Tessera was called iRice: setup also moves what that name installed
      to Tessera's (migrate_from_irice): /etc/irice to /etc/tessera, its
      drop-ins and PAM marks renamed, Quick Share's trust rule and the boot
      animation in use under the new name, then the old helper
      (/usr/lib/irice), its polkit action and rule, its ananicy rule and its
      boot animations' copies removed. Until then the shell keeps using that
      old helper. The login screen's copy moves with ./install.sh
      --only=login; until then wallpaper and login-look write to the old one
      (/usr/share/irice), which setup leaves as it is

Files are read as you (the person who started pkexec or sudo), never as
root: a symlink in your home can't make root read anything you can't.
"""
import contextlib
import glob
import importlib.util
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

LIB = "/usr/lib/tessera"
PACMAN = "/usr/bin/pacman"
LOGIN_SHARE = "/usr/share/tessera"  # the login screen's (./install.sh --greeter)
LOGIN_WALLPAPER = LOGIN_SHARE + "/wallpaper.jpg"
LOGIN_LOOK = LOGIN_SHARE + "/greeter.json"
# The login screen installed while Tessera was called iRice, until
# ./install.sh --only=login installs it again as Tessera's
OLD_LOGIN_SHARE = "/usr/share/irice"


def login_share(prefix="/"):
    """The login screen's copy in use: Tessera's, else iRice's (still the
    one greetd starts until the login screen is installed again)."""
    new, old = (os.path.join(prefix, p.lstrip("/")) for p in (LOGIN_SHARE, OLD_LOGIN_SHARE))
    return old if not os.path.isdir(new) and os.path.isdir(old) else new
MAX_IMAGE = 64 * 1024 * 1024
MAX_SOURCE = 1024 * 1024
PREFIX = "/"  # where FILES go (tests: a folder)

# What setup installs: from the shell's folder (the repo's shell/, or its
# installed copy) → where, with which mode
FILES = [
    ("assets/scripts/tessera-root.py", LIB + "/tessera-root", 0o755),
    ("assets/scripts/quickshare/qs-wifi-helper.py", LIB + "/qs-wifi-helper.py", 0o644),
    ("assets/polkit/org.tessera.root.policy", "/usr/share/polkit-1/actions/org.tessera.root.policy", 0o644),
    ("assets/polkit/49-tessera.rules", "/etc/polkit-1/rules.d/49-tessera.rules", 0o644),
]
# ananicy-cpp's rule that keeps the shell ("tessera-shell", started so by
# launch-shell.sh) interactive, like GNOME Shell and KWin (CachyOS files "qs"
# as a background service otherwise). Fixed here, never read from a folder:
# `priority on` runs without a password, so it writes nothing it's given.
# Installed where ananicy-cpp is (its /etc/ananicy.d), unless turned off in
# Settings → Battery & Power (the marker).
ANANICY_RULE = "/etc/ananicy.d/tessera.rules"
ANANICY_TEXT = b"""# Tessera's shell: interactive, so panels and animations stay smooth while
# apps are busy (Tessera's root helper; Settings > Battery & Power turns it off).
# irice-shell: its name while Tessera was called iRice, until it restarts
{ "name": "tessera-shell", "type": "LowLatency_RT" }
{ "name": "irice-shell", "type": "LowLatency_RT" }
"""
OLD_ANANICY_RULE = "/etc/ananicy.d/irice.rules"  # (iRice's, replaced by this one)
PRIORITY_OFF = "/etc/tessera/priority-off"


# The boot animations (tools/make-bootsplash.py): setup copies each theme's
# folder from the shell's assets/plymouth to LIB/plymouth, root's own, the
# only place bootsplash copies a theme from
BOOTSPLASH_IDS = ("breathe", "orbit", "frost", "bloom")
PLYMOUTH_LIB = LIB + "/plymouth"
THEME_FILE = re.compile(r"^([a-z0-9]+@[12]x\.png|tessera-[a-z]+\.(script|plymouth))$")
MAX_THEME_FILES = 400


class Refused(Exception):
    """Arguments this helper won't act on; the message says why."""


REFUSED_EXIT = 64  # (an older helper without that subcommand or fix: callers ask as before)


# ── The fixes (System health, Settings → Sharing) ───────────────────────
# BlueZ plugins that make phones ask to pair for Quick Share (sharing-check.py)
# rap: Ranging; vcp: volume (with volume offset, audio input); micp: microphone;
# mcp: media control; bass: broadcast audio scan
PLUGINS = ("rap", "vcp", "micp", "mcp", "bass")
DROPIN = "/etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf"
OLD_DROPIN = "/etc/systemd/system/bluetooth.service.d/irice-no-rap.conf"  # (its name while Tessera was called iRice)
PAIRING_SCRIPT = r'''set -e
NOPLUGIN=@PLUGINS@
exe=$(systemctl show -p ExecStart --value bluetooth | sed -n 's/.*path=\([^ ;]*\).*/\1/p')
[ -x "$exe" ] || exe=/usr/lib/bluetooth/bluetoothd
mkdir -p /etc/systemd/system/bluetooth.service.d
printf '# Tessera: no Ranging or LE Audio control profiles, which make phones ask to pair for Quick Share\n[Service]\nExecStart=\nExecStart=%s --noplugin=%s\n' "$exe" "$NOPLUGIN" > /etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf
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
            echo "$s runs your network; Tessera's installer can switch it to NetworkManager" >&2
            exit 1
        fi
    done
fi
systemctl enable --now NetworkManager
'''
NETWORK_SHOWN = "systemctl enable --now NetworkManager"

# fprintd stopped while Broadcom's driver updates the reader's firmware (about
# a minute, on its first start): longer to start, then started (this waits
# for it)
FPRINTD_SCRIPT = r'''set -e
mkdir -p /etc/systemd/system/fprintd.service.d
printf '%s\n' '# Tessera: a fingerprint reader may update its firmware as fprintd starts' \
    '[Service]' 'TimeoutStartSec=5min' 'TimeoutStopSec=2min' > /etc/systemd/system/fprintd.service.d/tessera-timeout.conf
systemctl daemon-reload
systemctl reset-failed fprintd || true
systemctl start fprintd
'''

FIXES = {
    "networkmanager": ["/bin/sh", "-c", NETWORK_SCRIPT],
    "bluetooth": ["systemctl", "enable", "--now", "bluetooth"],
    "avahi": ["systemctl", "enable", "--now", "avahi-daemon"],
    "upower": ["systemctl", "enable", "--now", "upower"],
    "profiles": ["systemctl", "enable", "--now", "power-profiles-daemon"],
    "btpairing": ["/bin/sh", "-c", PAIRING_SCRIPT],
    "fprintd": ["/bin/sh", "-c", FPRINTD_SCRIPT],
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
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".tessera-")
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
FP_MARK = "# Tessera fingerprint"
OLD_FP_MARK = "# iRice fingerprint"  # (a line marked before iRice became Tessera: still ours)
# max-tries / timeout: a finger, or the password after a few seconds (PAM
# asks one thing at a time; the login screen waits longest). Five tries: some
# readers (Broadcom's) answer "no match" at once for a finger still resting
# on them, which used up two tries before a real touch
FP_LINES = {"polkit": "auth       sufficient   pam_fprintd.so max-tries=5 timeout=10   " + FP_MARK,
            "sudo": "auth       sufficient   pam_fprintd.so max-tries=5 timeout=10   " + FP_MARK,
            "login": "auth       sufficient   pam_fprintd.so max-tries=5 timeout=15   " + FP_MARK}
# (GDM isn't here: it asks for a finger by itself, gdm-fingerprint, once one is added)
FP_FILES = {"polkit": ["polkit-1"], "sudo": ["sudo"], "login": ["greetd", "ly", "sddm", "lightdm", "lxdm"]}
PAM_ETC, PAM_VENDOR = "/etc/pam.d", "/usr/lib/pam.d"
PAM_FPRINTD = ("/usr/lib/security/pam_fprintd.so", "/lib/security/pam_fprintd.so")


def pam_without_fingerprint(text):
    return "".join(l for l in text.splitlines(keepends=True) if FP_MARK not in l and OLD_FP_MARK not in l)


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
        # Off, and /etc's copy is the package's file again (it was copied
        # here to turn this on): removed, so the package's updates apply
        vsrc = os.path.join(vendor, name)
        if not on and src == dest and os.path.exists(vsrc):
            with open(vsrc) as f:
                if f.read() == new:
                    os.unlink(dest)
                    changed.append(dest)
                    continue
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
        return run([howdy, "-U", user, "--plain", "-y", "add", "Tessera " + time.strftime("%Y-%m-%d %H.%M")]).returncode
    if args == ["clear"]:
        return run([howdy, "-U", user, "-y", "clear"]).returncode
    if args == ["list"]:
        return run([howdy, "-U", user, "--plain", "list"]).returncode
    raise Refused("face: setup <camera> | add | clear | list")


# ── phonecam (Settings → Phone → Use as webcam) ─────────────────────────
PHONECAM_DEV = 42
PHONECAM_OPTIONS = f'options v4l2loopback devices=1 video_nr={PHONECAM_DEV} card_label="Phone camera" exclusive_caps=1\n'
PHONECAM_FILES = ("/etc/modprobe.d/tessera-phonecam.conf", "/etc/modules-load.d/tessera-phonecam.conf")


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
            # (modprobe says yes when it was loaded already, with other
            # options, by an app still using it: the camera is checked for)
            run(["udevadm", "settle", "--timeout=5"], capture_output=True)
            if not os.path.exists(os.path.join(dev_dir, f"video{PHONECAM_DEV}")):
                raise Refused("another app's virtual camera is in the way (OBS?): close it, or restart, then try again")
    else:
        for path in (probe, load):
            if os.path.exists(path):
                os.unlink(path)
                changed.append(path)
        run(["modprobe", "-r", "v4l2loopback"], capture_output=True)
    return changed


# ── quickshare-trust (Settings → Sharing → Fast transfers without asking) ─
# Quick Share's Wi-Fi helper (it makes and joins Wi-Fi Direct links) asks
# for the password each time it starts. On: a polkit rule lets it start
# without asking, for the person who turned it on, at this computer, in the
# session on screen; nothing else changes (updates and the rest still ask).
# A marker readable by all says it's on (polkit's rules.d is root's only).
QS_TRUST_RULE = "/etc/polkit-1/rules.d/49-tessera-quickshare.rules"
QS_TRUST_MARK = "/etc/tessera/quickshare-trusted"


def quickshare_trust_rule(user):
    if not re.fullmatch(r"[a-z_][a-z0-9_-]{0,31}\$?", user or ""):
        raise Refused("unusable user name")
    return f"""// Tessera: Quick Share's Wi-Fi helper starts without the password for {user}
// (Settings → Sharing → Fast transfers without asking; off removes this file)
polkit.addRule(function (action, subject) {{
    if (action.id !== "org.tessera.root.wifi-helper")
        return polkit.Result.NOT_HANDLED;
    if (action.lookup("program") !== "/usr/lib/tessera/tessera-root")
        return polkit.Result.NOT_HANDLED;
    var words = String(action.lookup("command_line") || "").split(" ");
    if (words[0] !== "/usr/lib/tessera/tessera-root" || words[1] !== "wifi-helper" || words.length !== 2)
        return polkit.Result.NOT_HANDLED;
    if (subject.user === "{user}" && subject.active && subject.local)
        return polkit.Result.YES;
    return polkit.Result.NOT_HANDLED;
}});
"""


def quickshare_trust(on, uid, prefix=None):
    """Returns the files changed."""
    prefix = PREFIX if prefix is None else prefix
    rule = os.path.join(prefix, QS_TRUST_RULE.lstrip("/"))
    mark = os.path.join(prefix, QS_TRUST_MARK.lstrip("/"))
    if on:
        try:
            user = pwd.getpwuid(uid).pw_name
        except KeyError:
            raise Refused("unknown user") from None
        if uid == 0:
            raise Refused("turn it on as yourself, not as root")
        write_root(rule, quickshare_trust_rule(user).encode(), 0o644)
        write_root(mark, (user + "\n").encode(), 0o644)
        return [rule, mark]
    changed = []
    for path in (rule, mark):
        if os.path.exists(path):
            os.unlink(path)
            changed.append(path)
    return changed


# ── bootmenu (Settings → Software Update → Going back) ──────────────────
GRUB_BTRFS_SCRIPT = "/etc/grub.d/41_snapshots-btrfs"


def bootmenu(on, run=subprocess.run, which=shutil.which, exists=os.path.exists):
    """Snapshots in GRUB's boot menu: grub-btrfs's daemon (grub-btrfsd)
    keeps a "snapshots" submenu up to date as snapper makes them. On also
    writes the submenu now. Returns what it did."""
    if not which("grub-btrfsd") or not exists(GRUB_BTRFS_SCRIPT):
        raise Refused("grub-btrfs isn't installed")
    if on:
        if run(["systemctl", "enable", "--now", "grub-btrfsd.service"], capture_output=True).returncode != 0:
            raise Refused("grub-btrfsd didn't start")
        run([GRUB_BTRFS_SCRIPT], capture_output=True)
        return ["enabled grub-btrfsd"]
    run(["systemctl", "disable", "--now", "grub-btrfsd.service"], capture_output=True)
    return ["disabled grub-btrfsd"]


def priority(on, prefix=None, run=subprocess.run):
    """The shell's priority with ananicy-cpp (Settings → Battery & Power):
    on writes its fixed rule, off removes it and leaves a marker so updates
    (setup) don't put it back. Returns what changed."""
    prefix = PREFIX if prefix is None else prefix
    rule = os.path.join(prefix, ANANICY_RULE.lstrip("/"))
    marker = os.path.join(prefix, PRIORITY_OFF.lstrip("/"))
    if not os.path.isdir(os.path.dirname(rule)):
        raise Refused("ananicy-cpp isn't installed (its /etc/ananicy.d is missing)")
    changed = []
    old = os.path.join(prefix, OLD_ANANICY_RULE.lstrip("/"))
    if os.path.exists(old):
        os.remove(old)
        changed.append(old)
    if on:
        write_root(rule, ANANICY_TEXT, 0o644)
        changed.append(rule)
        if os.path.exists(marker):
            os.remove(marker)
            changed.append(marker)
    else:
        os.makedirs(os.path.dirname(marker), exist_ok=True)
        write_root(marker, "Turned off in Settings → Battery & Power (Keep Tessera smooth when busy)\n".encode(), 0o644)
        changed.append(marker)
        if os.path.exists(rule):
            os.remove(rule)
            changed.append(rule)
    # ananicy-cpp reads its rules as it starts
    if prefix == "/":
        with contextlib.suppress(OSError, subprocess.SubprocessError):
            run(["systemctl", "try-restart", "ananicy-cpp.service"], timeout=20, check=False)
    return changed


# ── sharing-ports (Settings → Sharing: while you're visible) ────────────
# The firewall's way in for nearby sharing, open only while something needs
# it (services/SharingPorts.qml), closed otherwise: never for good. Fixed
# sets, asked for by name; each (protocol, first port, last port)
PORT_SETS = {
    # Quick Share's port and its faster link's (nearby/wifi.py UPGRADE_PORT),
    # and DHCP for phones joining its hotspot
    "quickshare": (("tcp", 53318, 53319), ("udp", 67, 67)),
    # LocalSend's HTTPS port and its multicast discovery (devices answer an
    # announcement on either)
    "localsend": (("tcp", 53317, 53317), ("udp", 53317, 53317)),
    # mDNS: phones find this computer, and it finds them, on the network
    "mdns": (("udp", 5353, 5353),),
    # A QR-code share (link-share.py: 53320, else 53319) and Cast's file (53320)
    "links": (("tcp", 53319, 53320),),
}
PORTS_TAG = "tessera sharing"  # (nftables' and iptables' comment on ours)
OLD_PORTS_TAG = "irice sharing"  # (the same, opened while Tessera was called iRice: closed alike)
PORTS_TAGS = "(?:" + PORTS_TAG + "|" + OLD_PORTS_TAG + ")"
# ufw keeps its rules through a restart (it has no runtime ones), so ours
# carry a comment and close takes out exactly those. The fix before this
# left the same "iRice sharing", and these (ours from before iRice became
# Tessera: the tags then)
UFW_TAG = "Tessera sharing"
UFW_OLD = ("iRice sharing", "iRice sharing (LocalSend)", "iRice Quick Share hotspot (DHCP)")
UFW_RULE = re.compile(r"^\[\s*(\d+)\]\s+(\S+)(?: \(v6\))?\s+ALLOW(?: IN)?\s.*?(?:#\s*(.*?))?\s*$")
# firewalld's ports have no comment: the ones we added, noted where a
# restart clears it, as it clears them
FIREWALLD_ADDED = "/run/tessera/sharing-ports"
OLD_FIREWALLD_ADDED = "/run/irice/sharing-ports"  # (noted by the helper from before iRice became Tessera)
# The fix before this, kept for good: firewalld's ports and services (the
# services only where the zone doesn't come with them), nftables' drop-in
FIREWALLD_OLD = ("53317-53320/tcp", "53317/udp")
FIREWALLD_OLD_SERVICES = ("mdns", "dhcp")
NFT_OLD = ("/etc/nftables.d/irice.nft", "/etc/nftables.d/irice.conf")  # (named while Tessera was called iRice)


def _cmd(run, *cmd):
    """(exit code, output) of a command; (None, "") when it can't run."""
    try:
        p = run(list(cmd), capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None, ""
    return p.returncode, p.stdout or ""


def firewall_kind(run=subprocess.run):
    """The firewall that's on: ufw, firewalld, nftables, iptables or None."""
    if re.search(r"^Status: active", _cmd(run, "ufw", "status")[1], re.M):
        return "ufw"
    if _cmd(run, "firewall-cmd", "--state")[0] == 0:
        return "firewalld"
    for kind in ("nftables", "iptables"):
        if _cmd(run, "systemctl", "is-active", "--quiet", kind)[0] == 0:
            return kind
    return None


def _ports_ufw(wanted, run, did):
    want = {f"{lo}:{hi}/{p}" if hi != lo else f"{lo}/{p}" for p, lo, hi in wanted}
    rules = [(int(m[1]), m[2], m[3] or "") for m in map(UFW_RULE.match, _cmd(run, "ufw", "status", "numbered")[1].splitlines()) if m]
    # Ours that aren't wanted, last first (the numbers after one move up)
    for n, to, note in sorted(rules, reverse=True):
        if (note in (UFW_TAG,) + UFW_OLD or (note == "mDNS" and to == "5353/udp")) and to not in want:
            did(f"closed {to}", "ufw", "--force", "delete", str(n))
    # (a port someone allowed already is left to them)
    for to in sorted(want - {to for _n, to, _note in rules}):
        did(f"opened {to}", "ufw", "allow", to, "comment", UFW_TAG)


def _ports_firewalld(wanted, run, did, prefix):
    want = {f"{lo}-{hi}/{p}" if hi != lo else f"{lo}/{p}" for p, lo, hi in wanted}
    # The fix before this: out of the permanent set, and out of this boot's
    if _cmd(run, "firewall-cmd", "--permanent", f"--query-port={FIREWALLD_OLD[0]}")[0] == 0:
        zone = _cmd(run, "firewall-cmd", "--get-default-zone")[1].strip()
        shipped = ""
        if re.fullmatch(r"[\w-]+", zone):
            with contextlib.suppress(OSError), open(os.path.join(prefix, f"usr/lib/firewalld/zones/{zone}.xml")) as f:
                shipped = f.read()
        old = [f"--remove-port={p}" for p in FIREWALLD_OLD] + \
              [f"--remove-service={s}" for s in FIREWALLD_OLD_SERVICES if f'name="{s}"' not in shipped]
        did("removed the permanent sharing rules", "firewall-cmd", "--permanent", *old)
        did("closed the permanent sharing rules for now", "firewall-cmd", *old)
    path = os.path.join(prefix, FIREWALLD_ADDED.lstrip("/"))
    added = set()
    for p in (path, os.path.join(prefix, OLD_FIREWALLD_ADDED.lstrip("/"))):
        try:
            with open(p) as f:
                added |= set(f.read().split())
        except OSError:
            pass
    with contextlib.suppress(OSError):
        os.remove(os.path.join(prefix, OLD_FIREWALLD_ADDED.lstrip("/")))
    for port in sorted(added - want):
        if did(f"closed {port}", "firewall-cmd", f"--remove-port={port}"):
            added.discard(port)
    for port in sorted(want):
        # (one there already, ours or the zone's own, stays as it is)
        if _cmd(run, "firewall-cmd", f"--query-port={port}")[0] != 0 and did(f"opened {port}", "firewall-cmd", f"--add-port={port}"):
            added.add(port)
    if added:
        write_root(path, "".join(p + "\n" for p in sorted(added)).encode(), 0o644)
    elif os.path.exists(path):
        os.remove(path)


def _nft_input_chains(text):
    """qs-wifi-helper.py's list of nftables' input chains (next to this file
    once installed; in quickshare/ in the shell's folder)."""
    here = os.path.dirname(os.path.realpath(__file__))
    path = next(p for p in (os.path.join(here, "qs-wifi-helper.py"), os.path.join(here, "quickshare/qs-wifi-helper.py"))
                if os.path.exists(p))
    spec = importlib.util.spec_from_file_location("qs_wifi_helper", path)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    return helper.nft_input_chains(text)


def _ports_nft(wanted, run, did):
    # An accept in a table of our own wouldn't do: nftables runs every base
    # chain on the input hook, and a drop in any of them is final. So ours go
    # first in each input chain (iptables' own INPUT chains are left to it)
    want = {f"{p} dport {lo}-{hi}" if hi != lo else f"{p} dport {lo}" for p, lo, hi in wanted}
    for f, t, c in _nft_input_chains(_cmd(run, "nft", "list", "chains")[1]):
        kept = set()
        for line in _cmd(run, "nft", "-a", "list", "chain", f, t, c)[1].splitlines():
            m = re.search(r'^\s*(.*?) accept comment "' + PORTS_TAGS + r'" # handle (\d+)', line)
            if m and (m[1] not in want or m[1] in kept):
                did(f"closed {m[1]} ({f} {t} {c})", "nft", "delete", "rule", f, t, c, "handle", m[2])
            elif m:
                kept.add(m[1])
        for rule in sorted(want - kept):
            did(f"opened {rule} ({f} {t} {c})", "nft", "insert", "rule", f, t, c, *rule.split(), "accept", "comment", f'"{PORTS_TAG}"')


def _ports_iptables(wanted, run, did):
    want = {f"{p} {lo}:{hi}" if hi != lo else f"{p} {lo}" for p, lo, hi in wanted}
    for ipt in ("iptables", "ip6tables"):
        if ipt == "ip6tables" and _cmd(run, "systemctl", "is-active", "--quiet", "ip6tables")[0] != 0:
            continue
        ours = []
        for line in _cmd(run, ipt, "-L", "INPUT", "-n", "--line-numbers")[1].splitlines():
            m = re.match(r"^(\d+)\s.*\b(tcp|udp) dpts?:(\S+).*/\* " + PORTS_TAGS + r" \*/", line)
            if m:
                ours.append((int(m[1]), f"{m[2]} {m[3]}"))
        # Ours that aren't wanted, last first (the numbers after one move up)
        for n, rule in sorted(ours, reverse=True):
            if rule not in want:
                did(f"closed {rule} ({ipt})", ipt, "-D", "INPUT", str(n))
        for rule in sorted(want - {r for _n, r in ours}):
            p, port = rule.split()
            did(f"opened {rule} ({ipt})", ipt, "-I", "INPUT", "-p", p, "--dport", port,
                "-m", "comment", "--comment", PORTS_TAG, "-j", "ACCEPT")


def sharing_ports(sets, run=subprocess.run, prefix=None):
    """Lets in exactly these sets' ports (none: closes them all), and takes
    out what the permanent fix before this left. Returns (what it did, what
    failed)."""
    for s in sets:
        if s not in PORT_SETS:
            raise Refused("sharing-ports: " + " | ".join(PORT_SETS))
    prefix = PREFIX if prefix is None else prefix
    wanted = sorted({p for s in sets for p in PORT_SETS[s]})
    done, failed = [], []

    def did(what, *cmd):
        ok = _cmd(run, *cmd)[0] == 0
        (done if ok else failed).append(what)
        return ok

    for old in NFT_OLD:
        path = os.path.join(prefix, old.lstrip("/"))
        if os.path.exists(path):
            os.remove(path)
            done.append(f"removed {old}")
    kind = firewall_kind(run)
    if kind == "ufw":
        _ports_ufw(wanted, run, did)
    elif kind == "firewalld":
        _ports_firewalld(wanted, run, did, prefix)
    elif kind == "nftables":
        _ports_nft(wanted, run, did)
    elif kind == "iptables":
        _ports_iptables(wanted, run, did)
    else:
        done.append("no firewall is turned on: nothing to open or close")
    return done, failed
# ── bootsplash (Settings → Lock & Login Screen → Boot animation) ───────
PLYMOUTH_THEMES = "/usr/share/plymouth/themes"
PLYMOUTH_CONF = "/etc/plymouth/plymouthd.conf"
PLYMOUTH_DEFAULTS = "/usr/share/plymouth/plymouthd.defaults"
BOOTSPLASH_RECORD = "/etc/tessera/bootsplash.json"  # {"original": the system's theme, "applied": Tessera's}
OLD_BOOTSPLASH_RECORD = "/etc/irice/bootsplash.json"  # (until setup moves it)
INITRAMFS_BACKUP = "/var/lib/tessera/initramfs-backup"
THEME_NAME = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")


def theme_files(folder):
    """A theme folder's files ({name: (bytes, mode)}): only the names a theme
    of ours has, regular files, no links."""
    if os.path.islink(folder) or not os.path.isdir(folder):
        return {}
    files = read_tree(folder)
    bad = [n for n in files if not THEME_FILE.match(n)]
    if bad or len(files) > MAX_THEME_FILES:
        raise Refused(f"{folder} holds files a boot animation doesn't: " + ", ".join(sorted(bad)[:3]))
    return files


def install_themes(src, uid, prefix):
    """setup's part: each boot animation's folder from the shell's
    assets/plymouth (read as you) to LIB/plymouth, whole, when it changed.
    Returns the folders written."""
    written = []
    for id in BOOTSPLASH_IDS:
        name = "tessera-" + id
        top = os.path.join(src, "assets", "plymouth", name)
        with as_user(uid):
            if not os.path.isdir(top):
                continue  # (a shell folder from before them)
            names = sorted(os.listdir(top))
        if len(names) > MAX_THEME_FILES or not all(THEME_FILE.match(n) for n in names):
            raise Refused(f"{top} holds files a boot animation doesn't")
        files = {n: (read_as(uid, os.path.join(top, n), MAX_SOURCE), 0o644) for n in names}
        dest = os.path.join(prefix, PLYMOUTH_LIB.lstrip("/"), name)
        try:
            same = theme_files(dest) == files
        except (OSError, Refused):
            same = False
        if not same:
            write_tree(files, dest)
            written.append(dest)
    return written


def ini_value(text, key):
    """The last `key=value` in a Plymouth config (its one section, [Daemon])."""
    value = ""
    for line in text.splitlines():
        m = re.match(rf"^\s*{key}\s*=\s*(.*?)\s*$", line)
        if m:
            value = m.group(1)
    return value


def read_text(path):
    try:
        with open(path) as f:
            return f.read()
    except (OSError, UnicodeDecodeError):
        return None


def current_theme(prefix):
    """Plymouth's theme now: its config's, else the distribution's default."""
    for path in (PLYMOUTH_CONF, PLYMOUTH_DEFAULTS):
        value = ini_value(read_text(os.path.join(prefix, path.lstrip("/"))) or "", "Theme")
        if value:
            return value
    return ""


def mkinitcpio_hooks(prefix):
    """The HOOKS mkinitcpio builds with (its config, then conf.d's; the last wins)."""
    hooks = []
    paths = [os.path.join(prefix, "etc/mkinitcpio.conf")] + sorted(glob.glob(os.path.join(prefix, "etc/mkinitcpio.conf.d/*.conf")))
    for path in paths:
        for line in (read_text(path) or "").splitlines():
            m = re.match(r"^\s*HOOKS(\+?)=\((.*)\)", line)
            if m:
                hooks = (hooks if m.group(1) else []) + m.group(2).split()
    return hooks


def boot_images(prefix):
    """The boot images mkinitcpio -P writes (each preset's image and UKI),
    and any initramfs in /boot: the ones copied before a rebuild."""
    found = set(glob.glob(os.path.join(prefix, "boot/initramfs-*.img")))
    for preset in glob.glob(os.path.join(prefix, "etc/mkinitcpio.d/*.preset")):
        for line in (read_text(preset) or "").splitlines():
            m = re.match(r"""^\s*[A-Za-z0-9]+_(image|uki)=(["']?)(/[^"'\s]+)\2\s*$""", line)
            if m:
                found.add(os.path.join(prefix, m.group(3).lstrip("/")))
    return sorted(p for p in found if os.path.isfile(p) and not os.path.islink(p))


def copy_file(src, dest):
    """dest replaced whole by a copy of src (a temporary file, then a rename)."""
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(dest), prefix=".tessera-")
    try:
        with os.fdopen(fd, "wb") as out, open(src, "rb") as f:
            shutil.copyfileobj(f, out, 1 << 20)
            out.flush()
            os.fsync(out.fileno())
        shutil.copystat(src, tmp)
        os.rename(tmp, dest)
    except BaseException:
        with contextlib.suppress(OSError):
            os.unlink(tmp)
        raise


def bootsplash(choice, prefix=None, run=subprocess.run, which=shutil.which):
    """Sets the boot animation and rebuilds the boot image; on any failure
    the theme setting and the boot images are as they were. Returns what it
    did, a line each."""
    prefix = PREFIX if prefix is None else prefix

    def at(path):
        return os.path.join(prefix, path.lstrip("/"))
    if choice not in BOOTSPLASH_IDS + ("system", "none"):
        raise Refused("bootsplash: " + " | ".join(BOOTSPLASH_IDS) + " | system | none")
    if not which("plymouth-set-default-theme") or not os.path.isdir(at(PLYMOUTH_THEMES)):
        raise Refused("Plymouth isn't installed. Install the plymouth package, add plymouth to HOOKS in /etc/mkinitcpio.conf "
                      "and splash to the kernel's command line, then try again")
    if not which("mkinitcpio"):
        if which("dracut"):
            raise Refused("this computer builds its boot image with dracut, which Tessera doesn't rebuild. "
                          "Set the theme in a terminal: sudo plymouth-set-default-theme -R <theme>")
        if which("booster"):
            raise Refused("this computer builds its boot image with booster, which doesn't show Plymouth's animations")
        raise Refused("there's no mkinitcpio to rebuild the boot image with")
    if "plymouth" not in mkinitcpio_hooks(prefix):
        raise Refused("Plymouth isn't in the boot image. Add plymouth to HOOKS in /etc/mkinitcpio.conf (after kms), then try again")

    current = current_theme(prefix)
    try:
        record = json.loads(read_text(at(BOOTSPLASH_RECORD)) or read_text(at(OLD_BOOTSPLASH_RECORD)) or "null")
    except ValueError:
        record = None
    if not isinstance(record, dict) or not all(THEME_NAME.match(str(record.get(k, ""))) for k in ("original", "applied")):
        record = None
    # The system's own: the theme before Tessera first changed it, unless
    # someone has set another since (then that one)
    original = record["original"] if record and record["applied"] == current else current
    if not THEME_NAME.match(original or "") or original.startswith(("tessera-", "irice-")):
        original = ini_value(read_text(at(PLYMOUTH_DEFAULTS)) or "", "Theme") or "text"
    target = {"system": original, "none": "text"}.get(choice, "tessera-" + choice)

    theme_dir = at(os.path.join(PLYMOUTH_THEMES, target))
    files = {}
    if choice in BOOTSPLASH_IDS:
        src = at(os.path.join(PLYMOUTH_LIB, target))
        if os.path.islink(src) or not os.path.isdir(src) or (os.geteuid() == 0 and os.stat(src).st_uid != 0):
            raise Refused("Tessera's boot animations aren't installed yet. Update Tessera (Settings → Software Update), then try again")
        files = theme_files(src)
        if f"{target}.plymouth" not in files or f"{target}.script" not in files:
            raise Refused(f"{src} is incomplete. Update Tessera, then try again")
    elif not os.path.isfile(os.path.join(theme_dir, target + ".plymouth")):
        raise Refused(f"the theme {target} isn't installed any more. Reinstall it, or pick another")
    fresh = bool(files) and theme_files(theme_dir) != files
    if target == current and not fresh:
        return [f"already {target}"]

    # Copies of the boot images first: a failed rebuild must never leave the
    # computer unable to start
    images = boot_images(prefix)
    backup = at(INITRAMFS_BACKUP)
    shutil.rmtree(backup, ignore_errors=True)
    os.makedirs(backup, mode=0o700)
    need = sum(os.path.getsize(p) for p in images)
    if shutil.disk_usage(backup).free < need + 64 * 1024 * 1024:
        os.rmdir(backup)
        raise Refused(f"there isn't room to keep a copy of the boot images while they're rebuilt "
                      f"({need // (1024 * 1024) + 64} MB free in {os.path.dirname(INITRAMFS_BACKUP)}). Free some space, then try again")
    conf = at(PLYMOUTH_CONF)
    try:
        with open(conf, "rb") as f:
            conf_before = f.read()
    except FileNotFoundError:
        conf_before = None
    had_dir = os.path.isdir(theme_dir)
    kept = []

    def undo():
        if conf_before is None:
            with contextlib.suppress(FileNotFoundError):
                os.unlink(conf)
        else:
            write_root(conf, conf_before, 0o644)
        for p, copy in kept:
            copy_file(copy, p)
        if files and not had_dir:
            shutil.rmtree(theme_dir, ignore_errors=True)

    done = []
    try:
        for n, p in enumerate(images):
            copy = os.path.join(backup, f"{n}.img")
            shutil.copy2(p, copy)
            kept.append((p, copy))
        if fresh:
            write_tree(files, theme_dir)
            done.append(f"installed {theme_dir}")
        r = run(["plymouth-set-default-theme", target], capture_output=True, text=True)
        if r.returncode != 0:
            raise Refused(f"Plymouth didn't take the theme {target}, so nothing changed: " + (r.stderr or r.stdout).strip()[-400:])
        r = run(["mkinitcpio", "-P"], capture_output=True, text=True)
        if r.returncode != 0:
            out = (r.stdout or "") + (r.stderr or "")
            said = [l.strip() for l in out.splitlines() if "ERROR" in l] or out.strip().splitlines()
            raise Refused("rebuilding the boot image failed, so nothing changed: " + " / ".join(said[-3:])[-600:])
    except BaseException:
        undo()
        raise
    finally:
        shutil.rmtree(backup, ignore_errors=True)
    done.append(f"theme {target}, boot image rebuilt")

    # Remembered: the system's own theme (for "system"), and what Tessera set
    rec = at(BOOTSPLASH_RECORD)
    if target == original:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(rec)
    else:
        write_root(rec, json.dumps({"original": original, "applied": target}).encode(), 0o644)
    with contextlib.suppress(FileNotFoundError):
        os.unlink(at(OLD_BOOTSPLASH_RECORD))
    # Tessera's other themes, no longer used (and iRice's: Tessera's before
    # the rename)
    for id in BOOTSPLASH_IDS:
        for name in ("tessera-" + id, "irice-" + id):
            other = at(os.path.join(PLYMOUTH_THEMES, name))
            if name != target and os.path.isdir(other) and not os.path.islink(other):
                shutil.rmtree(other, ignore_errors=True)
    return done


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
        raise Refused("the Tessera login screen isn't installed (./install.sh --greeter)")
    write_root(dest, data, 0o644)


# ── login-look: what the login screen looks like ──────────────────────
LOOK_CHOICES = {
    "style": ("look", "text", "plain"),
    "look": ("glass", "material"),
    "mode": ("dark", "light"),
    "materialStyle": ("tonalSpot", "vibrant", "expressive", "neutral", "content", "fidelity", "monochrome"),
    "clockFace": ("dynamic", "stacked", "wide", "overlap", "analog", "weather"),
}
LOOK_RANGES = {"clockWeight": (100, 1000), "clockWidth": (25, 151)}
LOOK_KEYS = set(LOOK_CHOICES) | set(LOOK_RANGES) | {"seed", "clockRound"}
LOOK_OPTIONAL = {"clock24"}  # (true or false; a shell from before it sends none)
SEED = re.compile(r"^#[0-9a-fA-F]{6}$")


def check_login_look(text):
    """The look, as JSON with exactly LOOK_KEYS, each one of its values."""
    if not isinstance(text, str) or len(text) > 2048:
        raise Refused("give the look as one short JSON object")
    try:
        d = json.loads(text)
    except ValueError:
        raise Refused("that isn't JSON") from None
    if not isinstance(d, dict) or not LOOK_KEYS <= set(d) <= LOOK_KEYS | LOOK_OPTIONAL:
        raise Refused("give exactly: " + ", ".join(sorted(LOOK_KEYS)) + " (and clock24)")
    for k, values in LOOK_CHOICES.items():
        if not isinstance(d[k], str) or d[k] not in values:
            raise Refused(f"{k} is one of: " + ", ".join(values))
    for k, (lo, hi) in LOOK_RANGES.items():
        if type(d[k]) is not int or not lo <= d[k] <= hi:
            raise Refused(f"{k} is a whole number from {lo} to {hi}")
    if not isinstance(d["seed"], str) or not SEED.match(d["seed"]):
        raise Refused("seed is a colour as #rrggbb")
    for k in ("clockRound", "clock24"):
        if k in d and type(d[k]) is not bool:
            raise Refused(f"{k} is true or false")
    return d


def login_look(text, dest=LOGIN_LOOK):
    d = check_login_look(text)
    folder = os.path.dirname(dest)
    if not os.path.isdir(folder) or os.path.islink(folder):
        raise Refused("the Tessera login screen isn't installed (./install.sh --greeter)")
    # (one line, keys sorted: the shell compares it with what it would send)
    write_root(dest, json.dumps(d, sort_keys=True, separators=(",", ":")).encode(), 0o644)


# ── The login screen's copy of Tessera (setup) ──────────────────────────
TREE_SKIP = {".git", "__pycache__", "build", "node_modules"}
MAX_TREE_FILE = 32 * 1024 * 1024
MAX_TREE = 256 * 1024 * 1024
MAX_TREE_FILES = 20000
VM_MARK = b"\n-- virtual machine ("  # (what install.sh adds to greeter.lua in one)


def login_tree(src):
    """The checkout the login screen's copy comes from: the shell folder's
    own (install.sh, setup) or the update's staged one, else None."""
    src = src.rstrip("/")
    if os.path.isfile(os.path.join(src, "shell.qml")):
        return os.path.dirname(src)
    staged = os.path.join(src, "tree")
    if os.path.isfile(os.path.join(staged, "shell", "shell.qml")):
        return staged
    return None


def read_tree(top, uid=None):
    """{relative path: (bytes, mode)} of a folder's regular files (links,
    devices and TREE_SKIP folders left out), read as uid when one's given."""
    out, total = {}, 0
    with as_user(uid) if uid is not None else contextlib.nullcontext():
        for d, dirs, names in os.walk(top):
            dirs[:] = sorted(x for x in dirs if x not in TREE_SKIP and not os.path.islink(os.path.join(d, x)))
            for n in sorted(names):
                p = os.path.join(d, n)
                st = os.lstat(p)
                if n.endswith(".pyc") or st.st_mode & 0o170000 != 0o100000:
                    continue
                if uid is not None:
                    data = read_as(uid, p, MAX_TREE_FILE)
                else:
                    with open(p, "rb") as f:
                        data = f.read()
                total += len(data)
                if total > MAX_TREE or len(out) >= MAX_TREE_FILES:
                    raise Refused(f"{top} is too big for the login screen's copy")
                out[os.path.relpath(p, top)] = (data, 0o755 if st.st_mode & 0o111 else 0o644)
    return out


def write_tree(files, dest):
    """Puts files ({path: (bytes, mode)}) at dest whole: built next to it,
    then swapped in, so the login screen never starts from half a copy."""
    new, old = dest + ".new", dest + ".old"
    for p in (new, old):
        if os.path.islink(p):
            os.unlink(p)
        shutil.rmtree(p, ignore_errors=True)
    os.makedirs(new, mode=0o755)
    for rel, (data, mode) in files.items():
        p = os.path.join(new, rel)
        os.makedirs(os.path.dirname(p), mode=0o755, exist_ok=True)
        fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, mode)
        try:
            os.fchmod(fd, mode)
            os.write(fd, data)
        finally:
            os.close(fd)
    if os.path.islink(dest):
        os.unlink(dest)
    elif os.path.exists(dest):
        os.rename(dest, old)
    os.rename(new, dest)
    shutil.rmtree(old, ignore_errors=True)


def refresh_login(src, uid, prefix=None):
    """The login screen's copy of Tessera, from the same checkout as setup's
    files, where ./install.sh --greeter made one. Returns what it wrote."""
    prefix = PREFIX if prefix is None else prefix
    # (Tessera's only: iRice's, while greetd still starts that one, is left as
    # it was installed, whole, until ./install.sh --only=login replaces it)
    share = os.path.join(prefix, LOGIN_SHARE.lstrip("/"))
    if os.path.islink(share) or os.path.islink(os.path.join(share, "shell")) or not os.path.isdir(os.path.join(share, "shell")):
        return []
    repo = login_tree(src)
    if repo is None:
        return []
    written = []
    for name in ("shell", "material"):
        top = os.path.join(repo, name)
        if not os.path.isfile(os.path.join(top, "shell.qml")):
            continue  # (a version without the Material look)
        files = read_tree(top, uid)
        dest = os.path.join(share, name)
        try:
            same = os.path.isdir(dest) and not os.path.islink(dest) and read_tree(dest) == files
        except (OSError, Refused):
            same = False
        if not same:
            write_tree(files, dest)
            written.append(dest)
    lua = os.path.join(repo, "hypr", "greeter.lua")
    if os.path.isfile(lua):
        data = read_as(uid, lua, MAX_SOURCE)
        dest = os.path.join(share, "greeter.lua")
        try:
            with open(dest, "rb") as f:
                before = f.read()
        except OSError:
            before = b""
        # (a virtual machine's software cursor, which install.sh added, stays)
        i = before.find(VM_MARK)
        data = data.rstrip(b"\n") + b"\n" + (before[i:] if i >= 0 else b"")
        if data != before:
            write_root(dest, data, 0o644)
            written.append(dest)
    return written


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


def setup(src, uid, prefix=None, run=subprocess.run):
    """Installs FILES from the shell's folder, only the ones that changed,
    and ananicy-cpp's rule where it's installed (unless turned off). Returns
    the destinations written."""
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
    # (iRice's /etc/irice first: its priority-off marker counts below)
    written += move_folders_from_irice(prefix)
    rule = os.path.join(prefix, ANANICY_RULE.lstrip("/"))
    if os.path.isdir(os.path.dirname(rule)) and not os.path.exists(os.path.join(prefix, PRIORITY_OFF.lstrip("/"))):
        try:
            with open(rule, "rb") as f:
                same = f.read() == ANANICY_TEXT
        except OSError:
            same = False
        if not same:
            write_root(rule, ANANICY_TEXT, 0o644)
            written.append(rule)
    written += install_themes(src, uid, prefix)
    written += refresh_login(src, uid, prefix)
    written += migrate_from_irice(prefix, run)
    # ananicy-cpp reads its rules as it starts
    if prefix == "/" and any(d.startswith("/etc/ananicy.d/") or d == OLD_ANANICY_RULE for d in written):
        with contextlib.suppress(OSError, subprocess.SubprocessError):
            run(["systemctl", "try-restart", "ananicy-cpp.service"], timeout=20, check=False)
    return written


# ── iRice is now Tessera: what setup moves, once ───────────────────────
# What the helper installed under iRice's name. setup puts Tessera's in
# place first (FILES above), so there's never a moment without a helper.
OLD_LIB = "/usr/lib/irice"
OLD_FILES = ("/usr/share/polkit-1/actions/org.irice.root.policy", "/etc/polkit-1/rules.d/49-irice.rules",
             "/etc/dbus-1/system.d/zzz-irice-quickshare.conf")  # (that one: left by a Wi-Fi helper that died)
OLD_QS_TRUST_RULE = "/etc/polkit-1/rules.d/49-irice-quickshare.rules"
OLD_FOLDERS = (("/etc/irice", "/etc/tessera"), ("/var/lib/irice", "/var/lib/tessera"))
# Drop-ins and module options: the same content under Tessera's name
RENAMED = (
    (OLD_DROPIN, DROPIN),
    ("/etc/systemd/system/fprintd.service.d/irice-timeout.conf", "/etc/systemd/system/fprintd.service.d/tessera-timeout.conf"),
    ("/etc/systemd/system/plymouth-quit.service.d/irice.conf", "/etc/systemd/system/plymouth-quit.service.d/tessera.conf"),
    ("/etc/modprobe.d/irice-phonecam.conf", PHONECAM_FILES[0]),
    ("/etc/modules-load.d/irice-phonecam.conf", PHONECAM_FILES[1]),
)


def move_folders_from_irice(prefix):
    """/etc/irice and /var/lib/irice into Tessera's (what's in both stays
    Tessera's). Returns what it moved."""
    done = []
    for old, new in OLD_FOLDERS:
        o, n = (os.path.join(prefix, p.lstrip("/")) for p in (old, new))
        if os.path.islink(o) or not os.path.isdir(o):
            continue
        os.makedirs(n, mode=0o755, exist_ok=True)
        for name in os.listdir(o):
            if not os.path.lexists(os.path.join(n, name)):
                os.rename(os.path.join(o, name), os.path.join(n, name))
        shutil.rmtree(o, ignore_errors=True)
        done.append(new)
    return done


def migrate_from_irice(prefix, run=subprocess.run):
    """Everything else iRice's helper left under its name, now that
    Tessera's is installed. Returns what it wrote or removed."""
    def at(path):
        return os.path.join(prefix, path.lstrip("/"))
    done = []
    # Quick Share's trust rule, for the same person, for Tessera's helper
    old_rule = at(OLD_QS_TRUST_RULE)
    if os.path.exists(old_rule):
        user = (read_text(at(QS_TRUST_MARK)) or "").strip()
        if not user:
            m = re.search(r'subject\.user === "([^"]+)"', read_text(old_rule) or "")
            user = m.group(1) if m else ""
        with contextlib.suppress(Refused):
            write_root(at(QS_TRUST_RULE), quickshare_trust_rule(user).encode(), 0o644)
            write_root(at(QS_TRUST_MARK), (user + "\n").encode(), 0o644)
            done.append(QS_TRUST_RULE)
        os.unlink(old_rule)
        done.append(OLD_QS_TRUST_RULE)
    # Drop-ins and module options: renamed, same content
    reload = False
    for old, new in RENAMED:
        o, n = at(old), at(new)
        if os.path.isfile(o) and not os.path.islink(o):
            if not os.path.exists(n):
                with open(o, "rb") as f:
                    write_root(n, f.read().replace(b"iRice", b"Tessera").replace(b"irice uninstall", b"tessera uninstall"), 0o644)
                done.append(new)
            os.unlink(o)
            done.append(old)
            reload = reload or "/systemd/" in old
    # Fingerprint lines in PAM files: marked with Tessera's name
    for names in FP_FILES.values():
        for name in names:
            p = os.path.join(at(PAM_ETC), name)
            text = read_text(p)
            if text and OLD_FP_MARK in text:
                write_root(p, text.replace(OLD_FP_MARK, FP_MARK).encode(), 0o644)
                done.append(os.path.join(PAM_ETC, name))
    # The boot animation in use, under its new name (the boot image keeps
    # the old copy, the same animation, until it's next rebuilt)
    current = current_theme(prefix)
    if current.startswith("irice-") and current[len("irice-"):] in BOOTSPLASH_IDS:
        target = "tessera-" + current[len("irice-"):]
        src = at(os.path.join(PLYMOUTH_LIB, target))
        try:
            files = theme_files(src)
        except (OSError, Refused):
            files = {}
        conf = at(PLYMOUTH_CONF)
        text = read_text(conf) or ""
        if f"{target}.plymouth" in files and re.search(r"^\s*Theme\s*=\s*" + re.escape(current) + r"\s*$", text, re.M):
            write_tree(files, at(os.path.join(PLYMOUTH_THEMES, target)))
            write_root(conf, re.sub(r"^(\s*Theme\s*=\s*)" + re.escape(current) + r"(\s*)$", r"\g<1>" + target + r"\g<2>", text, flags=re.M).encode(), 0o644)
            done += [os.path.join(PLYMOUTH_THEMES, target), PLYMOUTH_CONF]
            try:
                record = json.loads(read_text(at(BOOTSPLASH_RECORD)) or "null")
            except ValueError:
                record = None
            if isinstance(record, dict) and record.get("applied") == current:
                record["applied"] = target
                write_root(at(BOOTSPLASH_RECORD), json.dumps(record).encode(), 0o644)
    # iRice's boot animations, unless one is still the one in use
    current = current_theme(prefix)
    for id in BOOTSPLASH_IDS:
        old = at(os.path.join(PLYMOUTH_THEMES, "irice-" + id))
        if "irice-" + id != current and os.path.isdir(old) and not os.path.islink(old):
            shutil.rmtree(old, ignore_errors=True)
            done.append(os.path.join(PLYMOUTH_THEMES, "irice-" + id))
    # The old helper, its polkit action and rule, its ananicy rule
    for old in OLD_FILES + (OLD_ANANICY_RULE,):
        if os.path.exists(at(old)):
            os.unlink(at(old))
            done.append(old)
    if os.path.isdir(at(OLD_LIB)) and not os.path.islink(at(OLD_LIB)):
        shutil.rmtree(at(OLD_LIB), ignore_errors=True)
        done.append(OLD_LIB)
    if reload and prefix == "/":
        with contextlib.suppress(OSError, subprocess.SubprocessError):
            run(["systemctl", "daemon-reload"], timeout=60, check=False)
    return done


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
    "snapper": ["snapper", "-c", "root", "create", "-d", "Before Tessera update", "-c", "number"],
    "timeshift": ["timeshift", "--create", "--comments", "Before Tessera update", "--scripted"],
}
# A package name: never starting with "-" (it would pass as a pacman option)
NAME = re.compile(r"^[a-z0-9@_+][a-z0-9@._+-]*$")


# Going back: an earlier package's file, from where earlier packages are kept
PKGFILE = re.compile(r"^[a-z0-9@_+][a-z0-9@._+-]*-[^-/]+-[^-/]+-[a-z0-9_]+\.pkg\.tar\.(zst|xz|gz|bz2)$")
ARCHIVE_URL = re.compile(r"^https://archive\.archlinux\.org/packages/[a-z0-9]/[a-z0-9@._+-]+/[a-z0-9@._+:-]+\.pkg\.tar\.(zst|xz)$")  # (":": an epoch)


def package_dirs(uid, run=subprocess.run):
    dirs = []
    try:
        r = run(["pacman-conf", "CacheDir"], capture_output=True, text=True, timeout=10)
        dirs = [l.strip() for l in r.stdout.splitlines() if l.strip()]
    except (OSError, subprocess.SubprocessError):
        pass
    try:
        home = pwd.getpwuid(uid).pw_dir
    except KeyError:
        home = ""
    return (dirs or ["/var/cache/pacman/pkg/"]) + ([os.path.join(home, ".cache/yay"), os.path.join(home, ".cache/paru/clone")] if home else [])


def check_package_file(v, dirs):
    if ARCHIVE_URL.match(v):
        return v
    path = os.path.realpath(v)
    if not v.startswith("/") or not PKGFILE.match(os.path.basename(path)) or not os.path.isfile(path):
        raise Refused(f"not a package file: {v}")
    if not any(path.startswith(os.path.realpath(d).rstrip("/") + "/") for d in dirs):
        raise Refused(f"not from a package cache: {v}")
    return path


def parse_update(args, dirs=None):
    """update's arguments, strictly: known flags only, package names checked."""
    p = {"snapshot": "", "system": False, "ignore": [], "install": [], "remove": [], "downgrade": [], "setup": "", "broker": False}
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("--system", "--broker"):
            p[a[2:]] = True
            i += 1
            continue
        if a not in ("--snapshot", "--ignore", "--install", "--remove", "--downgrade", "--setup") or i + 1 >= len(args):
            raise Refused(f"unknown argument: {a}")
        v = args[i + 1]
        i += 2
        if a == "--snapshot":
            if v not in SNAPSHOT:
                raise Refused("unknown snapshot tool")
            p["snapshot"] = v
        elif a == "--setup":
            p["setup"] = check_folder(v)
        elif a == "--downgrade":
            p["downgrade"] = [check_package_file(f, dirs or []) for f in v.split(",") if f]
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
            msg = f"tessera-root: refused: {e}\n"
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
    p = parse_update(args, package_dirs(uid))
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
            say("Installed " + ", ".join(done) if done else "Tessera's system helper is up to date")
        except (OSError, Refused) as e:
            say(f"Tessera's system helper wasn't installed: {e}")
    if p["downgrade"]:
        say("@@step undo")
        if subprocess.run([PACMAN, "-U", "--noconfirm", "--"] + p["downgrade"], stdout=out, stderr=subprocess.STDOUT,
                          env=dict(os.environ, LANG="C", LC_ALL="C")).returncode != 0:
            return 24
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
            say("tessera-root: no channel for the AUR step")
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
            wallpaper(args[0], uid, os.path.join(login_share(), "wallpaper.jpg"))
            return 0
        if cmd == "login-look" and len(args) == 1:
            login_look(args[0], os.path.join(login_share(), "greeter.json"))
            return 0
        if cmd == "fix" and args == ["firewall"]:
            cmd, args = "sharing-ports", ["open"]  # (a shell from before sharing-ports: all of them, as before)
        if cmd == "sharing-ports" and (args[:1] == ["open"] or args == ["close"]):
            done, failed = sharing_ports((args[1:] or list(PORT_SETS)) if args[0] == "open" else [])
            for d in done:
                print(d)
            for d in failed:
                print(f"tessera-root: couldn't: {d}", file=sys.stderr)
            return 1 if failed else 0
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
        if cmd == "quickshare-trust" and len(args) == 1 and args[0] in ("on", "off"):
            for d in quickshare_trust(args[0] == "on", uid):
                print("changed", d)
            return 0
        if cmd == "bootmenu" and len(args) == 1 and args[0] in ("on", "off"):
            for d in bootmenu(args[0] == "on"):
                print(d)
            return 0
        if cmd == "priority" and len(args) == 1 and args[0] in ("on", "off"):
            for d in priority(args[0] == "on"):
                print("changed", d)
            return 0
        if cmd == "bootsplash" and len(args) == 1:
            for d in bootsplash(args[0]):
                print(d)
            return 0
        if cmd == "setup" and len(args) == 1:
            for d in setup(args[0], uid):
                print("installed", d)
            return 0
        raise Refused("usage: tessera-root wallpaper <path> | login-look <json> | fix <id> | wifi-helper | fingerprint on|off <where> | phonecam on|off | face … | bootmenu on|off | "
                      "bootsplash <id>|system|none | quickshare-trust on|off | priority on|off | sharing-ports open [<set>…]|close | "
                      "update … | setup <folder>")
    except Refused as e:
        print(f"tessera-root: {e}", file=sys.stderr)
        return REFUSED_EXIT
    except OSError as e:
        print(f"tessera-root: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
