#!/usr/bin/env python3
"""Wi-Fi networks as QR codes, both ways (services/WifiShare.qml).

  wifi-qr.py share SSID   the saved network as a QR code a phone's camera
                          joins: JSON {"ok", "ssid", "security", "password",
                          "hidden", "qr": an SVG file}; the file is private
                          (the runtime folder, 0600) and goes when the card
                          closes (forget)
  wifi-qr.py forget FILE  removes a code made by share
  wifi-qr.py scan IMAGE   Wi-Fi codes in an image (a screenshot): JSON
                          {"ok", "found": [{"ssid", "security", "password",
                          "hidden"}]}

The code is the common WIFI: format (ZXing; Android and iOS cameras read
it): WIFI:T:WPA;S:name;P:password;H:true;; with \\ ; , : " escaped.
Passwords come from NetworkManager (`nmcli -s`, allowed for the person at
the computer) and go to qrencode on stdin, never on a command line.
Enterprise (802.1X) networks have no password to share: refused.
"""
import json
import os
import re
import secrets
import shutil
import subprocess
import sys

SPECIAL = '\\;,:"'


def escape(s):
    return "".join("\\" + c if c in SPECIAL else c for c in s)


def wifi_text(ssid, security, password, hidden=False):
    """The WIFI: text for a network: security "WPA" | "SAE" | "WEP" | "nopass"."""
    parts = [f"T:{security}", f"S:{escape(ssid)}"]
    if security != "nopass":
        parts.append(f"P:{escape(password)}")
    if hidden:
        parts.append("H:true")
    return "WIFI:" + ";".join(parts) + ";;"


def split_fields(body):
    """`T:WPA;S:a\\;b;;` → ["T:WPA", "S:a;b"] (escapes undone)."""
    fields, cur, i = [], [], 0
    while i < len(body):
        c = body[i]
        if c == "\\" and i + 1 < len(body):
            cur.append(body[i + 1])
            i += 2
            continue
        if c == ";":
            if cur:
                fields.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    if cur:
        fields.append("".join(cur))
    return fields


def parse_wifi(text):
    """A WIFI: code → {"ssid", "security", "password", "hidden"}, or None."""
    text = (text or "").strip()
    if not text.upper().startswith("WIFI:"):
        return None
    out = {"ssid": "", "security": "nopass", "password": "", "hidden": False}
    for f in split_fields(text[5:]):
        key, _, value = f.partition(":")
        key = key.upper()
        if key == "S":
            out["ssid"] = value
        elif key == "T":
            out["security"] = value.upper() or "nopass"
        elif key == "P":
            out["password"] = value
        elif key == "H":
            out["hidden"] = value.lower() == "true"
    if out["security"] == "NOPASS":
        out["security"] = "nopass"
    if not out["ssid"] or len(out["ssid"].encode()) > 32:
        return None
    return out


def security_of(key_mgmt):
    """NetworkManager's key-mgmt → the code's T:, or None (can't share)."""
    return {"": "nopass", "none": "nopass", "wpa-psk": "WPA", "sae": "SAE", "owe": "nopass"}.get(key_mgmt)


def nmcli(*args):
    try:
        p = subprocess.run(["nmcli", "--escape", "no", *args], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return p.stdout if p.returncode == 0 else None


def profile_for(ssid):
    """The saved connection for this network name: the one named after it
    (usually), then the active one, first."""
    out = nmcli("-t", "-f", "NAME,UUID,TYPE,ACTIVE", "connection", "show") or ""
    rows = []
    for line in out.splitlines():
        # (--escape no: a NAME can hold colons; UUID, TYPE and ACTIVE can't)
        parts = line.rsplit(":", 3)
        if len(parts) == 4 and parts[2] == "802-11-wireless":
            rows.append((parts[0] == ssid, parts[3] == "yes", parts[1]))
    for _, _, uuid in sorted(rows, reverse=True):
        if (nmcli("-g", "802-11-wireless.ssid", "connection", "show", uuid) or "").rstrip("\n") == ssid:
            return uuid
    return None


def runtime_dir():
    d = os.path.join(os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}", "irice")
    os.makedirs(d, mode=0o700, exist_ok=True)
    return d


def share(ssid):
    if not shutil.which("nmcli"):
        return {"ok": False, "error": "Sharing a network needs NetworkManager."}
    if not shutil.which("qrencode"):
        return {"ok": False, "error": "Sharing a network needs qrencode: sudo pacman -S qrencode"}
    uuid = profile_for(ssid)
    if uuid is None:
        return {"ok": False, "error": "This computer hasn't saved that network, so it has no password to share."}
    key_mgmt = (nmcli("-g", "802-11-wireless-security.key-mgmt", "connection", "show", uuid) or "").strip()
    security = security_of(key_mgmt)
    wep = ""
    if key_mgmt == "none":
        # (NetworkManager calls WEP "none" too: it's open only without a key)
        wep = (nmcli("-s", "-g", "802-11-wireless-security.wep-key0", "connection", "show", uuid) or "").rstrip("\n")
        if wep:
            security = "WEP"
    if security is None:
        return {"ok": False, "error": "Networks you sign in to with a username (like eduroam) can't be shared "
                                      "with a code."}
    password = wep
    if security in ("WPA", "SAE"):
        password = (nmcli("-s", "-g", "802-11-wireless-security.psk", "connection", "show", uuid) or "").rstrip("\n")
        if not password:
            return {"ok": False, "error": "NetworkManager didn't give the password (it may be kept in a keyring "
                                          "that's locked). Unlock it and try again."}
    hidden = (nmcli("-g", "802-11-wireless.hidden", "connection", "show", uuid) or "").strip() == "yes"
    path = os.path.join(runtime_dir(), f"wifi-qr-{secrets.token_hex(6)}.svg")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(fd)
    p = subprocess.run(["qrencode", "-t", "SVG", "-m", "2", "-l", "M", "-o", path],
                       input=wifi_text(ssid, security, password, hidden), text=True, capture_output=True, timeout=10)
    if p.returncode != 0:
        forget(path)
        return {"ok": False, "error": "The code couldn't be made (qrencode failed)."}
    return {"ok": True, "ssid": ssid, "security": security, "password": password, "hidden": hidden, "qr": path}


def forget(path):
    # (only codes this script made: in our runtime folder, by their name)
    if os.path.dirname(os.path.realpath(path)) == os.path.realpath(runtime_dir()) and \
            re.fullmatch(r"wifi-qr-[0-9a-f]{12}\.svg", os.path.basename(path)):
        try:
            os.remove(path)
        except OSError:
            pass
    return {"ok": True}


def scan(image):
    if not shutil.which("zbarimg"):
        return {"ok": True, "found": []}  # (nothing to read codes with: nothing found)
    # (-Sbinary: the bytes as they are; zbar's own guess at the character set
    # turns UTF-8 names like "Café" into Japanese)
    p = subprocess.run(["zbarimg", "-q", "--raw", "--nodbus", "-Sdisable", "-Sqrcode.enable", "-Sbinary", image],
                       capture_output=True, timeout=20)
    text = p.stdout.decode("utf-8", "replace")
    found, seen = [], set()
    # (a code's text can run over several lines: split on each WIFI: start)
    for chunk in re.split(r"(?=WIFI:)", text, flags=re.I):
        w = parse_wifi(chunk.strip())
        if w and w["ssid"] not in seen:
            seen.add(w["ssid"])
            found.append(w)
    return {"ok": True, "found": found}


def main(argv):
    if len(argv) == 2 and argv[0] == "share":
        r = share(argv[1])
    elif len(argv) == 2 and argv[0] == "forget":
        r = forget(argv[1])
    elif len(argv) == 2 and argv[0] == "scan":
        r = scan(argv[1])
    else:
        r = {"ok": False, "error": "usage: wifi-qr.py share SSID | forget FILE | scan IMAGE"}
    print(json.dumps(r))
    return 0 if r.get("ok") else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
