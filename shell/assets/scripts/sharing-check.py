#!/usr/bin/env python3
"""What on this computer keeps nearby sharing (LocalSend, Quick Share) from
working, and the fixed fixes for it. Used by Settings → Sharing, System
health (health-check.py) and the Quick Share helper (why a link failed).

  firewall   ufw, firewalld, nftables or iptables turned on: phones reach
             this computer (LocalSend 53317, Quick Share 53318 and its
             faster-link port 53319, mDNS 5353/udp to be found, DHCP for its
             hotspot) only through the ports Tessera opens while you're
             visible to others or sharing (`ports`, below). Unprivileged
             programs can't read the rules, so what's open is known from
             `ports`' own note (~/.local/state/tessera/sharing-ports.json:
             this boot's, except ufw's, which stay through a restart), or
             when the rules file is readable (or firewalld answers).
  pairing    BlueZ's experimental features on (Tessera's setup turns them on
             for headphone batteries) bring profiles that need encryption the
             moment a phone connects for Quick Share, so the phone asks to
             pair (phone-to-phone Quick Share never does): Ranging (RAP)
             reads the phone's Ranging Service, and the LE Audio controls
             (volume, volume offset, audio input, microphone, media control,
             broadcast scan) are offered to the phone, which takes the
             computer for earbuds and pairs to use them. Without LE Audio's
             core (no PACS here) those controls do nothing anyway. The fix
             starts bluetoothd without those plugins (a systemd drop-in);
             headphone batteries keep working.

  sharing-check.py status          JSON {"firewall": {...}, "pairing": {...}}
  sharing-check.py fix pairing
        runs the fixed script (tessera-root.py) through pkexec: the installed
        root helper, without a password (/usr/lib/tessera/tessera-root fix …),
        or before it's installed the script itself, after Tessera's password
        dialog; nothing from the command line reaches it. JSON {"ok", "error"}.
  sharing-check.py ports <set>[,<set>…]|none [why]
        lets in exactly these of the root helper's fixed sets (PORT_SETS),
        or none (services/SharingPorts.qml): `tessera-root sharing-ports
        open|close` through pkexec, no password for an administrator at this
        computer. Asks nothing with no firewall on, or when they're so
        already. Notes what's open, and why (for System health). JSON {"ok",
        "error", "kind"}; the exit code is pkexec's (64: the installed
        helper is older, or missing; 126 cancelled; 127 not allowed).
  sharing-check.py ports again
        the last ones asked for, asked again (System health's Fix button)
TESSERA_SYSTEM_ROOT (tests) is where /etc and /usr are looked for.
"""
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.environ.get("TESSERA_SYSTEM_ROOT", "/")
_state = os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state")
# (…/irice: Tessera's folder from before it was renamed, not moved yet)
STATE = os.path.join(_state, "irice" if not os.path.exists(os.path.join(_state, "tessera")) and os.path.isdir(os.path.join(_state, "irice")) else "tessera")
PORTS_NOTE = os.path.join(STATE, "sharing-ports.json")
# The fixed scripts live in the root helper (tessera-root.py), which runs them
# without a password once installed; until then they go through pkexec here
_spec = importlib.util.spec_from_file_location("tessera_root", os.path.join(os.path.dirname(os.path.abspath(__file__)), "tessera-root.py"))
tessera_root = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tessera_root)
DROPIN = tessera_root.DROPIN
PLUGINS = tessera_root.PLUGINS
PAIRING_SCRIPT, PAIRING_SHOWN = tessera_root.PAIRING_SCRIPT, tessera_root.PAIRING_SHOWN
# The installed helper (/usr/lib/tessera/tessera-root): its fixes don't ask.
# Until the root setup installs it, the one from before Tessera was called
# iRice (/usr/lib/irice/irice-root) does, through its own polkit rule.
HELPER = os.path.join(ROOT, "usr/lib/tessera/tessera-root")
OLD_HELPER = os.path.join(ROOT, "usr/lib/irice/irice-root")
if not os.access(HELPER, os.X_OK) and os.access(OLD_HELPER, os.X_OK):
    HELPER = OLD_HELPER


def helper_installed():
    return os.access(HELPER, os.X_OK)


def helper_current():
    """The installed helper is this folder's (Software Update installs it
    in its root step; until then it can be older, with older fixes)."""
    try:
        with open(HELPER, "rb") as a, open(_spec.origin, "rb") as b:
            return a.read() == b.read()
    except OSError:
        return False


def _path(p):
    return os.path.join(ROOT, p.lstrip("/"))


def _run(*cmd, timeout=2):
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return p.returncode, p.stdout
    except (OSError, subprocess.TimeoutExpired):
        return None, ""


def _read(p):
    try:
        with open(_path(p)) as f:
            return f.read()
    except OSError:
        return None


def _opens(rules, port, proto="tcp"):
    """Whether ufw's user.rules accept this port: "--dport 53318", or a
    list or range ("--dports 53317:53319", as `ufw allow 53317:53319/tcp`
    writes it). TCP, or proto="udp"."""
    other = "-p udp" if proto == "tcp" else "-p tcp"
    for line in rules.splitlines():
        if "-j ACCEPT" not in line or other in line:
            continue
        m = re.search(r"--dports?\s+(\S+)", line)
        for part in (m.group(1).split(",") if m else []):
            lo, _, hi = part.partition(":")
            if lo.isdigit() and int(lo) <= port <= int(hi if hi.isdigit() else lo):
                return True
    return False


def _boot():
    try:
        with open("/proc/sys/kernel/random/boot_id") as f:
            return f.read().strip()
    except OSError:
        return ""


def _active(unit):
    return _run("systemctl", "is-active", unit)[1].strip() == "active"


def _accepts_all(rules):
    """An iptables-save or nftables file that drops or rejects nothing."""
    return not re.search(r"\b(drop|reject|DROP|REJECT)\b", rules)


def firewall_kind():
    conf = _read("/etc/ufw/ufw.conf")
    if conf is not None and re.search(r"^\s*ENABLED\s*=\s*yes", conf, re.M) and _active("ufw"):
        return "ufw"
    for kind, tool in (("firewalld", "firewall-cmd"), ("nftables", "nft"), ("iptables", "iptables")):
        if os.path.exists(_path("/usr/bin/" + tool)) and _active(kind):
            return kind
    return None


def ports_note():
    """What `ports` last asked for and got: {"kind", "want", "open", "why",
    "boot", "code", "error"}, or {} before it ever ran."""
    try:
        with open(PORTS_NOTE) as f:
            note = json.load(f)
        return note if isinstance(note, dict) else {}
    except (OSError, ValueError):
        return {}


def _current(note, kind):
    """The note is about this firewall's rules as they are now (ufw keeps
    them through a restart; the others' go with it)."""
    return bool(note) and note.get("kind") == kind and (kind == "ufw" or note.get("boot") == _boot())


def firewall():
    """{"kind": "ufw" | "firewalld" | "nftables" | "iptables" | None,
    "allowed": True | False | None (Quick Share's port gets through now),
    "blocks": bool (on, and not known to let it through), "hotspot": True |
    False | None (DHCP, 67/udp, gets through: phones joining Quick Share's
    hotspot need an address from it), "open": the sets Tessera has open now,
    and why; "want" (what was last asked for), "error" and "code" (how that
    went), "asked": bool (ever)}."""
    kind = firewall_kind()
    note = ports_note()
    mine = note.get("kind") == kind
    opened = note.get("open", []) if kind and _current(note, kind) else []
    out = {"kind": kind, "open": opened, "why": note.get("why", "") if opened else "", "want": note.get("want", []) if mine else [],
           "error": note.get("error", "") if mine else "", "code": note.get("code", 0) if mine else 0, "asked": bool(note)}
    if kind is None:
        return dict(out, allowed=None, blocks=False, hotspot=True)
    allowed = hotspot = True if "quickshare" in opened else None
    if allowed is None and kind == "ufw":
        rules = _read("/etc/ufw/user.rules")
        if rules is not None:
            allowed = _opens(rules, 53318)
    elif allowed is None and kind == "firewalld":
        code, _ = _run("firewall-cmd", "--query-port=53318/tcp")
        allowed = {0: True, 1: False}.get(code)
    elif allowed is None and kind == "nftables":
        if _read("/etc/nftables.d/irice.nft") is not None or _read("/etc/nftables.d/irice.conf") is not None:
            allowed = True  # (the drop-in Tessera's earlier, permanent fix kept: sharing-ports takes it out)
        else:
            rules = _read("/etc/nftables.conf")
            if rules is not None and _accepts_all(rules) and "include" not in rules:
                allowed = True
    elif allowed is None and kind == "iptables":
        rules = _read("/etc/iptables/iptables.rules")
        if rules is not None and (_accepts_all(rules) or _opens(rules, 53318)):
            allowed = True
    if hotspot is None and kind == "ufw":
        rules = _read("/etc/ufw/user.rules")
        if rules is not None:
            hotspot = _opens(rules, 67, "udp")
    elif hotspot is None and kind == "firewalld":
        # (NetworkManager puts its hotspot in firewalld's nm-shared zone, which allows DHCP)
        hotspot = True
    elif hotspot is None and kind == "nftables":
        drop = _read("/etc/nftables.d/irice.nft") or _read("/etc/nftables.d/irice.conf")
        if drop is not None:
            hotspot = " 67 " in drop.replace("}", " ").replace(",", " ")
    elif hotspot is None and kind == "iptables":
        rules = _read("/etc/iptables/iptables.rules")
        if rules is not None:
            hotspot = _accepts_all(rules) or _opens(rules, 67, "udp")
    return dict(out, allowed=allowed, blocks=allowed is not True, hotspot=hotspot)


def pairing():
    """{"experimental": bool, "rap_off": bool, "risk": bool}; rap_off: every
    plugin in PLUGINS is left out (an older drop-in with rap alone isn't enough)."""
    conf = _read("/etc/bluetooth/main.conf") or ""
    experimental = bool(re.search(r"^\s*Experimental\s*=\s*true", conf, re.M))
    text = _read(DROPIN) or _read(tessera_root.OLD_DROPIN)  # (the old name, until the root setup renames it)
    if text is None:
        _, text = _run("systemctl", "show", "-p", "ExecStart", "--value", "bluetooth")
    off = set()
    for m in re.finditer(r"--noplugin=(\S+)", text or ""):
        off.update(m.group(1).split(","))
    rap_off = off.issuperset(PLUGINS)
    return {"experimental": experimental, "rap_off": rap_off, "risk": experimental and not rap_off}


def status():
    return {"firewall": firewall(), "pairing": pairing(), "passwordless": helper_installed() and helper_current()}


def fix(what):
    script = {"pairing": PAIRING_SCRIPT}.get(what)
    if script is None:
        return {"ok": False, "error": "There's no fix for that."}
    if not shutil.which("pkexec"):
        return {"ok": False, "error": "pkexec (from polkit) is needed to ask for your password."}
    try:
        p = None
        # (an older installed helper may have older fixes: this folder's
        # script then, with the password)
        if helper_installed() and helper_current():
            p = subprocess.run(["pkexec", HELPER, "fix", "btpairing"], capture_output=True, text=True, timeout=120)
            if p.returncode == tessera_root.REFUSED_EXIT:  # (an older helper without this fix: ask as before)
                p = None
        if p is None:
            p = subprocess.run(["pkexec", "/bin/sh", "-c", script], capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "It took too long; try again."}
    if p.returncode in (126, 127):
        return {"ok": False, "error": "Cancelled: no password was given"}
    if p.returncode != 0:
        return {"ok": False, "error": (p.stderr or p.stdout).strip()[:200] or "It didn't work."}
    return {"ok": True, "error": ""}


def ports(sets, why=""):
    """Lets in exactly these of the root helper's sets (none: closes), and
    notes what's open. Asks nothing with no firewall on, or when they're so
    already and that went well. Returns (exit code, error, kind)."""
    note, kind = ports_note(), firewall_kind()
    sets = sorted(set(sets))
    code, error = 0, ""
    if kind and not (_current(note, kind) and note.get("open") == sets and note.get("code") == 0):
        if not helper_installed():
            code, error = tessera_root.REFUSED_EXIT, "Tessera's system helper isn't installed yet. Run Software Update once."
        elif not shutil.which("pkexec"):
            code, error = 1, "pkexec (from polkit) is needed to open the firewall's ports."
        else:
            try:
                p = subprocess.run(["pkexec", HELPER, "sharing-ports"] + (["open"] + sets if sets else ["close"]),
                                   capture_output=True, text=True, timeout=120)
                code = p.returncode
                error = "" if code == 0 else {
                    tessera_root.REFUSED_EXIT: "Tessera's system helper is from before this. Run Software Update once.",
                    126: "Cancelled: no password was given.", 127: "Not allowed: it needs an administrator's password.",
                }.get(code) or (p.stderr or p.stdout).replace("tessera-root: ", "").replace("irice-root: ", "").strip()[:300] or "It didn't work."
            except subprocess.TimeoutExpired:
                code, error = 1, "The firewall took too long to answer."
    # (what didn't change stays as it was: open until a close went through)
    opened = [] if not kind else sets if code == 0 else note.get("open", []) if _current(note, kind) else []
    os.makedirs(STATE, exist_ok=True)
    with open(PORTS_NOTE, "w") as f:
        json.dump({"kind": kind, "want": sets, "open": opened, "why": why, "boot": _boot(), "code": code, "error": error}, f)
    return code, error, kind


def main(argv):
    if argv[:1] == ["status"]:
        print(json.dumps(status()))
        return 0
    if len(argv) == 2 and argv[0] == "fix":
        r = fix(argv[1])
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    if argv == ["ports", "again"]:
        note = ports_note()
        argv = ["ports", ",".join(note.get("want", [])) or "none", note.get("why", "")]
    sets = ([] if argv[1] == "none" else argv[1].split(",")) if argv[:1] == ["ports"] and len(argv) in (2, 3) else None
    if sets is not None and set(sets) <= set(tessera_root.PORT_SETS):
        code, error, kind = ports(sets, argv[2] if len(argv) == 3 else "")
        print(json.dumps({"ok": code == 0, "error": error, "kind": kind}))
        if error:
            print(error, file=sys.stderr)
        return code
    print(json.dumps({"ok": False, "error": "usage: sharing-check.py status | fix pairing | ports <set>[,<set>…]|none [why] | ports again"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
