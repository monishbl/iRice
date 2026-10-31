#!/usr/bin/env python3
"""iRice's Quick Share service as receiver over Bluetooth only (no mDNS), as
when the phone is on another network. Accepts everything; events on stdout.

  blerx.py NAME [noupgrade]
noupgrade: no faster link is offered (pure Bluetooth transfer).
"""
import asyncio
import json
import os
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(REPO) + "/shell/assets/scripts/quickshare")
os.environ.setdefault("IRICE_QS_DEBUG", "1")

from nearby.bluez import Bluetooth  # noqa: E402
from nearby.gbus import GioBus  # noqa: E402
from nearby.service import QuickShareService  # noqa: E402
from nearby.wifi import WifiHelper  # noqa: E402

T0 = time.monotonic()
OUT = os.path.expanduser("~/Downloads/qs-test")


async def main():
    name = sys.argv[1]
    noupgrade = "noupgrade" in sys.argv[2:]
    loop = asyncio.get_running_loop()
    svc = None

    def emit(ev):
        print(f"{time.monotonic() - T0:7.2f} {json.dumps(ev)}", flush=True)
        if ev.get("ev") == "incoming":
            loop.call_soon(svc.answer, ev["session"], True)

    bus = GioBus(loop)
    ble = Bluetooth(bus)
    await ble.start()
    # no Wi-Fi helper: never asks for a password (nobody is here to type it)
    svc = QuickShareService(emit, None, ble=ble, helper=None)
    if noupgrade:
        async def nothing(*a, **k):
            return {"addresses": [], "station": None, "frequency": None}
        svc.net_probe = nothing
    if "nmhotspot" in sys.argv[2:]:
        # as on another network: the phone's address doesn't answer, so only
        # the NetworkManager hotspot is left (DHCP said to be allowed)
        async def no(ip):
            return False
        svc.reachable = no
        svc.firewall = lambda: {"kind": "ufw", "allowed": True, "blocks": False, "hotspot": True}
    os.makedirs(OUT, exist_ok=True)
    await svc.configure(True, name, OUT)
    stop = asyncio.Event()
    for s in (15, 2):
        loop.add_signal_handler(s, stop.set)
    await stop.wait()
    await svc.close()
    await bus.close()


asyncio.run(main())
