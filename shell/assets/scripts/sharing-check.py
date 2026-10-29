#!/usr/bin/env python3
"""What on this computer keeps nearby sharing (LocalSend, Quick Share) from
working, and the fixed fixes for it. Used by Settings → Sharing, System
health (health-check.py) and the Quick Share helper (why a link failed).

  firewall   ufw or firewalld turned on, without iRice's ports: phones can't
             reach this computer (LocalSend 53317, Quick Share 53318 and its
             faster-link port 53319, mDNS 5353/udp to be found). Unprivileged
             programs can't read the rules, so "allowed" is known when the
             rules file is readable (or firewalld answers), or after iRice's
             own fix ran (a note in ~/.local/state/irice).
  pairing    BlueZ's experimental features on (iRice's setup turns them on
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
  sharing-check.py fix firewall|pairing
        runs the fixed script (irice-root.py) through pkexec: the installed
        root helper, without a password (/usr/lib/irice/irice-root fix …),
        or before it's installed the script itself, after iRice's password
        dialog; nothing from the command line reaches it. JSON {"ok", "error"}.
IRICE_SYSTEM_ROOT (tests) is where /etc and /usr are looked for.
"""
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.environ.get("IRICE_SYSTEM_ROOT", "/")
STATE = os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "irice")
MARKER = os.path.join(STATE, "sharing-firewall")
# The fixed scripts live in the root helper (irice-root.py), which runs them
# without a password once installed; until then they go through pkexec here
_spec = importlib.util.spec_from_file_location("irice_root", os.path.join(os.path.dirname(os.path.abspath(__file__)), "irice-root.py"))
irice_root = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(irice_root)
DROPIN = irice_root.DROPIN
PLUGINS = irice_root.PLUGINS
FIREWALL_SCRIPT, FIREWALL_SHOWN = irice_root.FIREWALL_SCRIPT, irice_root.FIREWALL_SHOWN
PAIRING_SCRIPT, PAIRING_SHOWN = irice_root.PAIRING_SCRIPT, irice_root.PAIRING_SHOWN
# The installed helper (/usr/lib/irice/irice-root): its fixes don't ask
HELPER = os.path.join(ROOT, "usr/lib/irice/irice-root")


def helper_installed():
    return os.access(HELPER, os.X_OK)


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


def _opens(rules, port):
    """Whether ufw's user.rules accept this TCP port: "--dport 53318", or a
    list or range ("--dports 53317:53319", as `ufw allow 53317:53319/tcp`
    writes it)."""
    for line in rules.splitlines():
        if "-j ACCEPT" not in line or "-p udp" in line:
            continue
        m = re.search(r"--dports?\s+(\S+)", line)
        for part in (m.group(1).split(",") if m else []):
            lo, _, hi = part.partition(":")
            if lo.isdigit() and int(lo) <= port <= int(hi if hi.isdigit() else lo):
                return True
    return False


def firewall():
    """{"kind": "ufw" | "firewalld" | None, "allowed": True | False | None,
    "blocks": bool (on, and not known to let sharing through)}."""
    kind = None
    conf = _read("/etc/ufw/ufw.conf")
    if conf is not None and re.search(r"^\s*ENABLED\s*=\s*yes", conf, re.M) and \
            _run("systemctl", "is-active", "ufw")[1].strip() == "active":
        kind = "ufw"
    elif os.path.exists(_path("/usr/bin/firewall-cmd")) and \
            _run("systemctl", "is-active", "firewalld")[1].strip() == "active":
        kind = "firewalld"
    if kind is None:
        return {"kind": None, "allowed": None, "blocks": False}
    allowed = None
    try:
        with open(MARKER) as f:
            allowed = True if f.read().strip() == kind else None
    except OSError:
        pass
    if allowed is None and kind == "ufw":
        rules = _read("/etc/ufw/user.rules")
        if rules is not None:
            allowed = _opens(rules, 53318)
    elif allowed is None and kind == "firewalld":
        code, _ = _run("firewall-cmd", "--query-port=53318/tcp")
        allowed = {0: True, 1: False}.get(code)
    return {"kind": kind, "allowed": allowed, "blocks": allowed is not True}


def pairing():
    """{"experimental": bool, "rap_off": bool, "risk": bool}; rap_off: every
    plugin in PLUGINS is left out (an older drop-in with rap alone isn't enough)."""
    conf = _read("/etc/bluetooth/main.conf") or ""
    experimental = bool(re.search(r"^\s*Experimental\s*=\s*true", conf, re.M))
    text = _read(DROPIN)
    if text is None:
        _, text = _run("systemctl", "show", "-p", "ExecStart", "--value", "bluetooth")
    off = set()
    for m in re.finditer(r"--noplugin=(\S+)", text or ""):
        off.update(m.group(1).split(","))
    rap_off = off.issuperset(PLUGINS)
    return {"experimental": experimental, "rap_off": rap_off, "risk": experimental and not rap_off}


def status():
    return {"firewall": firewall(), "pairing": pairing(), "passwordless": helper_installed()}


def fix(what):
    script = {"firewall": FIREWALL_SCRIPT, "pairing": PAIRING_SCRIPT}.get(what)
    if script is None:
        return {"ok": False, "error": "There's no fix for that."}
    if not shutil.which("pkexec"):
        return {"ok": False, "error": "pkexec (from polkit) is needed to ask for your password."}
    kind = firewall()["kind"] if what == "firewall" else None
    try:
        p = None
        if helper_installed():
            p = subprocess.run(["pkexec", HELPER, "fix", "btpairing" if what == "pairing" else what],
                               capture_output=True, text=True, timeout=120)
            if p.returncode == irice_root.REFUSED_EXIT:  # (an older helper without this fix: ask as before)
                p = None
        if p is None:
            p = subprocess.run(["pkexec", "/bin/sh", "-c", script], capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "It took too long; try again."}
    if p.returncode in (126, 127):
        return {"ok": False, "error": "Cancelled: no password was given"}
    if p.returncode != 0:
        return {"ok": False, "error": (p.stderr or p.stdout).strip()[:200] or "It didn't work."}
    if what == "firewall" and kind:
        os.makedirs(STATE, exist_ok=True)
        with open(MARKER, "w") as f:
            f.write(kind + "\n")
    return {"ok": True, "error": ""}


def main(argv):
    if argv[:1] == ["status"]:
        print(json.dumps(status()))
        return 0
    if len(argv) == 2 and argv[0] == "fix":
        r = fix(argv[1])
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    print(json.dumps({"ok": False, "error": "usage: sharing-check.py status | fix firewall|pairing"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
