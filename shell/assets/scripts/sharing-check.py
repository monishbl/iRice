#!/usr/bin/env python3
"""What on this computer keeps nearby sharing (LocalSend, Quick Share) from
working, and the fixed fixes for it. Used by Settings → Sharing, System
health (health-check.py) and the Quick Share helper (why a link failed).

  firewall   ufw, firewalld, nftables or iptables turned on, without iRice's
             ports: phones can't reach this computer (LocalSend 53317, Quick
             Share 53318 and its faster-link port 53319, mDNS 5353/udp to be
             found). Unprivileged programs can't read the rules, so "allowed"
             is known when the rules file is readable (or firewalld answers),
             or after iRice's own fix ran (a note in ~/.local/state/irice;
             a fix that only lasts until the next restart, nftables without
             a drop-in folder or iptables, notes this boot too).
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
        dialog; nothing from the command line reaches it. JSON {"ok", "error",
        "note"}: note says how long the firewall fix lasts, when it says.
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
FIREWALL_SHOWN_BY = irice_root.FIREWALL_SHOWN_BY
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


def firewall():
    """{"kind": "ufw" | "firewalld" | "nftables" | "iptables" | None,
    "allowed": True | False | None, "blocks": bool (on, and not known to let
    sharing through), "runtime": bool (allowed only until the next restart)}."""
    kind = None
    conf = _read("/etc/ufw/ufw.conf")
    if conf is not None and re.search(r"^\s*ENABLED\s*=\s*yes", conf, re.M) and _active("ufw"):
        kind = "ufw"
    elif os.path.exists(_path("/usr/bin/firewall-cmd")) and _active("firewalld"):
        kind = "firewalld"
    elif os.path.exists(_path("/usr/bin/nft")) and _active("nftables"):
        kind = "nftables"
    elif os.path.exists(_path("/usr/bin/iptables")) and _active("iptables"):
        kind = "iptables"
    if kind is None:
        return {"kind": None, "allowed": None, "blocks": False, "runtime": False}
    allowed, runtime = None, False
    try:
        with open(MARKER) as f:
            # "kind", or "kind boot=<id>" for a fix that lasts until a restart
            words = f.read().split()
        boot = next((w[5:] for w in words[1:] if w.startswith("boot=")), None)
        if words[:1] == [kind] and (boot is None or boot == _boot()):
            allowed, runtime = True, boot is not None
    except OSError:
        pass
    if allowed is None and kind == "ufw":
        rules = _read("/etc/ufw/user.rules")
        if rules is not None:
            allowed = _opens(rules, 53318)
    elif allowed is None and kind == "firewalld":
        code, _ = _run("firewall-cmd", "--query-port=53318/tcp")
        allowed = {0: True, 1: False}.get(code)
    elif allowed is None and kind == "nftables":
        if _read("/etc/nftables.d/irice.nft") is not None or _read("/etc/nftables.d/irice.conf") is not None:
            allowed = True  # (iRice's own drop-in)
        else:
            rules = _read("/etc/nftables.conf")
            if rules is not None and _accepts_all(rules) and "include" not in rules:
                allowed = True
    elif allowed is None and kind == "iptables":
        rules = _read("/etc/iptables/iptables.rules")
        if rules is not None and (_accepts_all(rules) or _opens(rules, 53318)):
            allowed = True
    return {"kind": kind, "allowed": allowed, "blocks": allowed is not True, "runtime": runtime}


def remember(kind, out):
    """After the firewall fix: the rules can't be read without root, so a note
    says they're in (with this boot's id when they last only until a restart:
    the fix said "@@runtime"). Returns what the fix said, for the person."""
    if not kind:
        return ""
    os.makedirs(STATE, exist_ok=True)
    runtime = "@@runtime" in (out or "").splitlines()
    with open(MARKER, "w") as f:
        f.write(kind + (f" boot={_boot()}" if runtime else "") + "\n")
    return " ".join(l for l in (out or "").splitlines() if l.strip() and not l.startswith("@@"))[:300]


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


def older_firewall_fix(p):
    """An installed helper from before nftables and iptables were fixed (an
    update replaces it): the script here does it instead, with the password."""
    return p.returncode == 1 and "(ufw or firewalld)" in (p.stderr or "")


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
            if p.returncode == irice_root.REFUSED_EXIT or older_firewall_fix(p):  # (an older helper without this fix: ask as before)
                p = None
        if p is None:
            p = subprocess.run(["pkexec", "/bin/sh", "-c", script], capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "It took too long; try again."}
    if p.returncode in (126, 127):
        return {"ok": False, "error": "Cancelled: no password was given"}
    if p.returncode != 0:
        return {"ok": False, "error": (p.stderr or p.stdout).strip()[:200] or "It didn't work."}
    note = remember(kind, p.stdout) if what == "firewall" else ""
    return {"ok": True, "error": "", "note": note}


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
