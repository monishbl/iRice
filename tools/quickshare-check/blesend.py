#!/usr/bin/env python3
"""iRice's Quick Share service sending over Bluetooth only (no mDNS, and the
shared network left out), as when the phone is on another network: the
phone hosts its Wi-Fi link and NetworkManager joins it.

  blesend.py FILE MATCH [lan]
lan: keep the shared network on offer (a normal same-Wi-Fi send).
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

from nearby import wifi  # noqa: E402
from nearby.bluez import Bluetooth  # noqa: E402
from nearby.gbus import GioBus  # noqa: E402
from nearby.service import QuickShareService  # noqa: E402

T0 = time.monotonic()


async def main():
    path, match = sys.argv[1], sys.argv[2]
    lan = "lan" in sys.argv[3:]
    loop = asyncio.get_running_loop()
    done = asyncio.Event()
    svc = None
    sent = [False]

    def emit(ev):
        print(f"{time.monotonic() - T0:7.2f} {json.dumps(ev)}", flush=True)
        if ev.get("ev") == "devices" and not sent[0]:
            for d in ev["devices"]:
                if match.lower() in d["name"].lower():
                    sent[0] = True
                    print(f"{time.monotonic() - T0:7.2f} >>> sending to {d}", flush=True)
                    loop.create_task(svc.send(d["id"], [path]))
        if ev.get("ev") in ("finished", "failed", "declined"):
            done.set()

    bus = GioBus(loop)
    ble = Bluetooth(bus)
    await ble.start()
    svc = QuickShareService(emit, None, ble=ble, helper=None)
    if not lan:
        real = svc.net_probe

        async def elsewhere():
            n = await real()
            n["addresses"] = []  # (as on another network)
            return n
        svc.net_probe = elsewhere
        keep = wifi.mediums_we_join
        wifi.mediums_we_join = lambda h, nm_ok=False: [m for m in keep(h, nm_ok) if m != wifi.CR.WIFI_LAN]
    await svc.discover(True)
    try:
        await asyncio.wait_for(done.wait(), 240)
    except asyncio.TimeoutError:
        print("timed out", flush=True)
    await asyncio.sleep(3)  # (the link goes and the Wi-Fi comes back)
    await svc.close()
    await bus.close()


asyncio.run(main())
