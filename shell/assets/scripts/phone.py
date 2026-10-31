#!/usr/bin/env python3
"""Android phones on this computer (services/Phone.qml), with Google's own
Android tools: ADB to talk to the phone, scrcpy to show and control it.

  phone.py watch       follows phones (adb track-devices): a JSON line each
                       time they change, and every minute (the battery):
                       {"devices": [{"serial", "state": device|unauthorized|
                       offline, "wireless", "model", "android", "battery",
                       "charging"}]}
  phone.py pair        wireless pairing (Android 11+, Developer options ->
                       Wireless debugging -> Pair device with QR code):
                       JSON lines: {"qr": svg file} to show, then {"stage":
                       "pairing"|"connecting"}, ending with {"ok", "error"}.
                       Three minutes at most.

Pairing: the QR code holds a name and a password of ours; the phone scans it
and announces a pairing service with that name over mDNS (Avahi), where
`adb pair` uses the password; then the phone's own connect service is found
the same way and `adb connect` makes it a device. No cable needed after that
(until the phone turns wireless debugging off: many do after a restart or a
change of Wi-Fi).
"""
import json
import os
import re
import secrets
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def adb():
    return os.environ.get("IRICE_ADB") or shutil.which("adb")


def emit(obj):
    print(json.dumps(obj), flush=True)


def parse_track(block):
    """One `adb track-devices -l` listing: [{serial, state, model}]."""
    out = []
    for line in block.splitlines():
        parts = line.split()
        if len(parts) < 2:
            continue
        info = dict(p.split(":", 1) for p in parts[2:] if ":" in p)
        out.append({"serial": parts[0], "state": parts[1], "model": info.get("model", "").replace("_", " "),
                    "wireless": bool(re.match(r"^[\d.]+:\d+$|^adb-.*_adb-tls-connect", parts[0]))})
    return out


def shell(serial, *cmd, timeout=10):
    try:
        r = subprocess.run([adb(), "-s", serial, "shell"] + list(cmd), capture_output=True, text=True, timeout=timeout)
        return r.stdout if r.returncode == 0 else ""
    except (subprocess.TimeoutExpired, OSError):
        return ""


def details(dev):
    """Its name, Android version and battery (only for one that's allowed us in)."""
    if dev["state"] != "device":
        return dev
    props = shell(dev["serial"], "getprop ro.product.marketname; getprop ro.product.model; getprop ro.build.version.release")
    lines = [l.strip() for l in props.splitlines()]
    while len(lines) < 3:
        lines.append("")
    dev = dict(dev, model=lines[0] or lines[1] or dev["model"], android=lines[2])
    batt = shell(dev["serial"], "dumpsys", "battery")
    m = re.search(r"^\s*level:\s*(\d+)", batt, re.M)
    if m:
        dev["battery"] = int(m.group(1))
        st = re.search(r"^\s*status:\s*(\d+)", batt, re.M)
        dev["charging"] = bool(st and st.group(1) in ("2", "5"))  # (charging, full)
    return dev


def watch():
    exe = adb()
    if not exe:
        emit({"devices": [], "error": "adb isn't installed (android-tools)."})
        return 1
    proc = subprocess.Popen([exe, "track-devices", "-l"], stdout=subprocess.PIPE)
    buf, devices, last = b"", [], 0.0
    while True:
        r, _, _ = select.select([proc.stdout], [], [], 5)
        if r:
            chunk = os.read(proc.stdout.fileno(), 65536)
            if not chunk:
                return 0  # (adb's server went away; the shell starts us again)
            buf += chunk
            changed = False
            while len(buf) >= 4:
                n = int(buf[:4], 16)
                if len(buf) < 4 + n:
                    break
                devices = parse_track(buf[4:4 + n].decode(errors="replace"))
                buf = buf[4 + n:]
                changed = True
            if changed:
                emit({"devices": [details(d) for d in devices]})
                last = time.monotonic()
        if devices and time.monotonic() - last > 60:
            emit({"devices": [details(d) for d in devices]})
            last = time.monotonic()


def qr_svg(text):
    exe = shutil.which("qrencode")
    if not exe:
        return ""
    d = os.path.join(os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir(), "irice")
    os.makedirs(d, mode=0o700, exist_ok=True)
    path = os.path.join(d, "phone-pair-" + secrets.token_hex(4) + ".svg")
    r = subprocess.run([exe, "-t", "SVG", "-m", "1", "-o", path], input=text, text=True, capture_output=True)
    return path if r.returncode == 0 else ""


def browse(service, name=None, timeout=1.0):
    """Resolved services of a type: [(name, host, port)] (IPv4 first)."""
    try:
        r = subprocess.run(["avahi-browse", "-p", "-r", "-t", service], capture_output=True, text=True, timeout=timeout + 5)
    except (subprocess.TimeoutExpired, OSError):
        return []
    out = []
    for line in r.stdout.splitlines():
        f = line.split(";")
        if len(f) >= 9 and f[0] == "=" and (name is None or f[3] == name):
            out.append((f[3], f[7], int(f[8])))
    return sorted(out, key=lambda x: ":" in x[1])


def pair(limit=180):
    exe = adb()
    if not exe or not shutil.which("avahi-browse"):
        emit({"ok": False, "error": "Pairing needs adb (android-tools) and avahi."})
        return 1
    name = "irice-" + secrets.token_hex(3)
    password = "".join(secrets.choice("0123456789") for _ in range(6))
    path = qr_svg(f"WIFI:T:ADB;S:{name};P:{password};;")
    if not path:
        emit({"ok": False, "error": "Pairing needs qrencode."})
        return 1
    emit({"qr": path})
    end = time.monotonic() + limit
    try:
        while time.monotonic() < end:
            found = browse("_adb-tls-pairing._tcp", name)
            if not found:
                time.sleep(1)
                continue
            _, host, port = found[0]
            emit({"stage": "pairing"})
            try:
                # (the code on its input: adb asks for it when it isn't on the
                # command line, where any program could read it)
                r = subprocess.run([exe, "pair", f"{host}:{port}"], input=password + "\n",
                                   capture_output=True, text=True, timeout=30)
            except subprocess.TimeoutExpired:
                emit({"ok": False, "error": "The phone stopped answering. Try again."})
                return 1
            if r.returncode != 0 or "Successfully paired" not in r.stdout:
                emit({"ok": False, "error": "The phone refused the pairing. Try again."})
                return 1
            emit({"stage": "connecting"})
            # its connect service, from the same address
            while time.monotonic() < end:
                for _, h, p in browse("_adb-tls-connect._tcp"):
                    if h == host:
                        try:
                            c = subprocess.run([exe, "connect", f"{h}:{p}"], capture_output=True, text=True, timeout=30)
                        except subprocess.TimeoutExpired:
                            c = None
                        # (adb says "failed to connect" and still exits 0)
                        if c is None or "connected to" not in c.stdout:
                            emit({"ok": False, "error": "Paired, but the phone didn't connect. Connect it with a cable once, or pair again."})
                            return 1
                        emit({"ok": True, "error": ""})
                        return 0
                time.sleep(1)
        emit({"ok": False, "error": "No phone answered. On the phone, open Wireless debugging and choose Pair device with QR code."})
        return 1
    finally:
        try:
            os.unlink(path)
        except OSError:
            pass


class Stopped(BaseException):
    """SIGTERM (Cancel): raised so the code's file is removed."""


def stopped(*_):
    raise Stopped()


def main(argv):
    signal.signal(signal.SIGTERM, stopped)
    if argv[:1] == ["watch"]:
        return watch()
    if argv[:1] == ["pair"]:
        return pair()
    print(json.dumps({"error": "usage: phone.py watch | pair"}))
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Stopped:
        sys.exit(143)
