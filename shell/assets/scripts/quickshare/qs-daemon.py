#!/usr/bin/env python3
"""iRice's Quick Share helper (services/QuickShare.qml), over stdin/stdout.

Commands, one JSON object per line on stdin:
  {"cmd": "config", "visible": bool, "name": str, "folder": str}
  {"cmd": "discover", "on": bool}
  {"cmd": "send", "to": device id, "files": [paths]}
  {"cmd": "answer", "session": id, "accept": bool}
  {"cmd": "cancel", "session": id}
  {"cmd": "bluetooth_on"}   the user's "Turn on Bluetooth" (nothing else powers it)
Events, one per line on stdout: status, devices, incoming, connected,
progress, finished, declined, ended, failed, error, bluetooth (adapter
state), transport (which link a session uses, as it changes), warning (a
transfer stays on slow Bluetooth: why), auth (a password prompt is coming,
and what for) — see nearby/service.py.

IRICE_QS_DEBUG=1 logs what happens (frames by kind and size, no secrets) to
$XDG_RUNTIME_DIR/irice-quickshare.log (nearby/debuglog.py).

It ends when stdin closes: the shell quit, restarted, or no longer needs it
(nothing visible, nothing being sent). It's started only for that long.
"""
import asyncio
import json
import os
import signal
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    from nearby import debuglog
    from nearby.service import QuickShareService
except ImportError as e:
    print(json.dumps({"ev": "error", "message": str(e) if "Quick Share" in str(e) else
                      f"Quick Share needs python-cryptography and python-protobuf ({e}). Install them, then try again."}),
          flush=True)
    sys.exit(1)


def emit(obj):
    try:
        sys.stdout.write(json.dumps(obj) + "\n")
        sys.stdout.flush()
    except (BrokenPipeError, ValueError):
        os._exit(0)  # the shell is gone


async def main():
    loop = asyncio.get_running_loop()
    discovery = None
    # (IRICE_QS_NO_MDNS=1: tests, which mustn't announce on the real network)
    try:
        if os.environ.get("IRICE_QS_NO_MDNS") == "1":
            raise ImportError
        from nearby.avahi import Avahi, AvahiUnavailable
        try:
            discovery = Avahi(loop)
        except AvahiUnavailable as e:
            emit({"ev": "error", "message": f"Finding devices needs avahi-daemon ({e}). Install avahi and start it: "
                                            "sudo systemctl enable --now avahi-daemon"})
        except (AttributeError, ValueError) as e:
            # gi present but not PyGObject (or without Gio): no discovery,
            # but receiving by address and the rest still work
            emit({"ev": "error", "message": f"Finding devices needs python-gobject ({e}). Install it, then try again."})
    except ImportError:
        pass
    # Bluetooth LE (IRICE_QS_NO_BLE=1, and the tests' IRICE_QS_NO_MDNS=1: off)
    ble = bus = None
    if os.environ.get("IRICE_QS_NO_BLE") != "1" and os.environ.get("IRICE_QS_NO_MDNS") != "1":
        try:
            from nearby.bluez import Bluetooth
            from nearby.gbus import BusUnavailable, GioBus
            try:
                bus = GioBus(loop)
                ble = Bluetooth(bus, scan_uuids=os.environ.get("IRICE_QS_BLE_SCAN") != "all")
                if not await ble.start():
                    debuglog.log("daemon", "no Bluetooth adapter (yet)")
            except BusUnavailable as e:
                debuglog.log("daemon", f"no Bluetooth: {e}")
                ble = None
        except ImportError:
            pass
    from nearby.wifi import WifiHelper
    service = QuickShareService(emit, discovery, ble=ble, helper=WifiHelper())
    service.emit_bluetooth()

    reader = asyncio.StreamReader()
    await loop.connect_read_pipe(lambda: asyncio.StreamReaderProtocol(reader), sys.stdin)
    stop = asyncio.Event()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stop.set)

    async def commands():
        while True:
            line = await reader.readline()
            if not line:
                break
            try:
                c = json.loads(line)
            except ValueError:
                continue
            kind = c.get("cmd")
            try:
                if kind == "config":
                    await service.configure(c.get("visible"), c.get("name"), c.get("folder"))
                elif kind == "discover":
                    await service.discover(bool(c.get("on")))
                elif kind == "send":
                    await service.send(c.get("to", ""), [str(p) for p in c.get("files", [])])
                elif kind == "answer":
                    service.answer(c.get("session"), bool(c.get("accept")))
                elif kind == "cancel":
                    await service.cancel(c.get("session"))
                elif kind == "bluetooth_on":
                    await service.bluetooth_on()
            except Exception as e:  # noqa: BLE001 (one bad command mustn't end the helper)
                emit({"ev": "error", "message": f"Quick Share: {e}"})
        stop.set()

    task = asyncio.ensure_future(commands())
    await stop.wait()
    task.cancel()
    await service.close()
    if discovery is not None:
        await discovery.close()
    if bus is not None:
        await bus.close()


if __name__ == "__main__":
    asyncio.run(main())
