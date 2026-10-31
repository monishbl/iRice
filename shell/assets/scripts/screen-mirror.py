#!/usr/bin/env python3
"""Mirror the screen to a TV (services/Mirroring.qml), through GNOME Network
Displays' daemon (gnome-network-displays-daemon, on the session bus as
org.gnome.NetworkDisplays.Manager): Miracast over Wi-Fi Direct (the TV
needn't be on your network; the Wi-Fi card must do Wi-Fi Direct), Miracast
over the network (MICE, Windows' way), and Chromecast.

  screen-mirror.py watch        starts the daemon when it isn't running, then
                                a JSON line whenever the displays change:
                                {"displays": [{"uuid", "name", "protocol":
                                "miracast"|"chromecast"|"mice", "state":
                                "idle"|"connecting"|"streaming"|"error"}],
                                "p2p": bool (the Wi-Fi card does Wi-Fi Direct)}
  screen-mirror.py start UUID   {"ok", "stream": the stream's unit, "error"}
  screen-mirror.py stop STREAM  {"ok", "error"}
"""
import json
import os
import shutil
import subprocess
import sys
import time

BUS = "org.gnome.NetworkDisplays.Manager"
PATH = "/org/gnome/NetworkDisplays/Manager"
IFACE = "org.gnome.NetworkDisplays.Manager"
PROTOCOLS = {1: "miracast", 2: "chromecast", 3: "mice"}
# ND_SINK_STATE_*: disconnected, ensure-firewall, wait-p2p, wait-socket, wait-streaming, streaming, error
STATES = {0: "idle", 1: "connecting", 2: "connecting", 3: "connecting", 4: "connecting", 5: "streaming", 6: "error"}


def bus():
    from gi.repository import Gio
    return Gio.bus_get_sync(Gio.BusType.SESSION, None)


def call(conn, method, args=None, sig=None, iface=IFACE, timeout=30000):
    from gi.repository import Gio, GLib
    params = GLib.Variant(sig, args) if sig else None
    r = conn.call_sync(BUS, PATH, iface, method, params, None, Gio.DBusCallFlags.NONE, timeout, None)
    return r.unpack() if r is not None else ()


def displays(conn):
    (value,) = call(conn, "Get", (IFACE, "Displays"), "(ss)", iface="org.freedesktop.DBus.Properties")
    out = []
    for d in value:
        out.append({"uuid": d.get("uuid", ""), "name": d.get("display-name", "Display"),
                    "protocol": PROTOCOLS.get(d.get("protocol"), "other"), "state": STATES.get(d.get("state"), "idle")})
    return sorted(out, key=lambda d: d["name"].lower())


def p2p_capable():
    """The Wi-Fi card can do Wi-Fi Direct (iw lists P2P-client)."""
    if not shutil.which("iw"):
        return True  # (can't tell: don't say no)
    r = subprocess.run(["iw", "list"], capture_output=True, text=True)
    return "P2P-client" in r.stdout or "P2P-GO" in r.stdout


def has_owner(conn):
    from gi.repository import GLib
    try:
        (owned,) = conn.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner",
                                  GLib.Variant("(s)", (BUS,)), None, 0, 5000, None).unpack()
        return owned
    except Exception:  # noqa: BLE001
        return False


def watch():
    conn = bus()
    daemon = None
    if not has_owner(conn):
        exe = shutil.which("gnome-network-displays-daemon")
        if not exe:
            print(json.dumps({"displays": [], "error": "Screen mirroring needs gnome-network-displays."}), flush=True)
            return 1
        daemon = subprocess.Popen([exe], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            if has_owner(conn):
                break
            time.sleep(0.1)
    p2p = p2p_capable()
    last = None
    try:
        while True:
            try:
                now = displays(conn)
            except Exception:  # noqa: BLE001
                now = []
            if now != last:
                print(json.dumps({"displays": now, "p2p": p2p}), flush=True)
                last = now
            if daemon and daemon.poll() is not None:
                return 1
            time.sleep(1.5)
    finally:
        if daemon and daemon.poll() is None:
            daemon.terminate()


def start(uuid):
    try:
        (unit,) = call(bus(), "StartStream", (uuid,), "(s)")
        return {"ok": True, "stream": unit, "error": ""}
    except Exception as e:  # noqa: BLE001
        msg = getattr(e, "message", str(e))
        if "ServiceUnknown" in msg or "NoReply" in msg:
            return {"ok": False, "stream": "", "error": "Screen mirroring stopped looking for TVs. Open it again, then pick the TV."}
        return {"ok": False, "stream": "", "error": msg.split(":", 2)[-1].strip() or "The TV didn't answer."}


def stop(stream):
    try:
        call(bus(), "StopStream", (stream,), "(s)")
        return {"ok": True, "error": ""}
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "error": getattr(e, "message", str(e))}


def main(argv):
    if argv[:1] == ["watch"]:
        return watch()
    if len(argv) == 2 and argv[0] in ("start", "stop"):
        r = start(argv[1]) if argv[0] == "start" else stop(argv[1])
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    print(json.dumps({"error": "usage: screen-mirror.py watch | start UUID | stop STREAM"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
