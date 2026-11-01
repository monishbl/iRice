#!/usr/bin/env python3
"""Earbuds nearby (services/FastPair.qml), from Google's Fast Pair
advertisements (Bluetooth LE service data 0xFE2C, a public spec), through
BlueZ:

  - in pairing mode, earbuds announce their 3-byte model ID: the shell offers
    Connect, which pairs them like Bluetooth settings would (their classic
    side when it's found, else their LE side), trusts and connects them
  - with the case open, they announce their batteries: left, right and case
    (level, and whether each is charging)

  fastpair.py watch     JSON lines: {"devices": [{"address", "name", "model",
                        "pairing", "rssi", "battery": {"left", "right",
                        "case": {"level", "charging"}}}]}
  fastpair.py connect ADDRESS [NAME]
                        connects them (pairs and trusts them first, unless
                        they're paired already): {"ok", "error", "address"
                        (theirs, on success), "known" (a paired address that
                        wouldn't connect)}
  fastpair.py repair ADDRESS KNOWN [NAME]
                        forgets KNOWN, their old pairing (earbuds that were
                        reset no longer take it), and pairs them afresh

Scanning is light: only while Bluetooth is on; continuous on the charger,
else 4 seconds every 30. Devices not heard from for 20 seconds are dropped.
(The device's real name and picture come from Google's model lookup, which
needs an API key: the Bluetooth name is used instead.)
"""
import json
import re
import sys
import time

FAST_PAIR = "0000fe2c-0000-1000-8000-00805f9b34fb"
AUDIO_UUIDS = {"0000110b-0000-1000-8000-00805f9b34fb", "0000110d-0000-1000-8000-00805f9b34fb",
               "0000111e-0000-1000-8000-00805f9b34fb", "0000184e-0000-1000-8000-00805f9b34fb"}


def parse(data):
    """Fast Pair service data -> {"model"} (pairing mode) or {"battery"}.

    Pairing mode: exactly the 3-byte model ID. Otherwise: a flags byte, then
    fields each led by a byte 0bLLLLTTTT (length, type): 0 the account key
    filter, 1 its salt, 3/4 battery (shown/hidden): up to three bytes, left,
    right, case, each 0bCLLLLLLL (C: charging; 0x7F: unknown)."""
    data = bytes(data)
    if len(data) == 3:
        return {"model": data.hex().upper(), "pairing": True}
    out = {"pairing": False}
    i = 1
    while i < len(data):
        head = data[i]
        length, kind = head >> 4, head & 0x0F
        body = data[i + 1:i + 1 + length]
        if kind in (3, 4) and body:
            names = ("left", "right", "case")
            bat = {}
            for n, b in zip(names, body):
                level = b & 0x7F
                if level != 0x7F:
                    bat[n] = {"level": level, "charging": bool(b & 0x80)}
            if bat:
                out["battery"] = bat
                out["showUi"] = kind == 3
        i += 1 + length
    return out


ADDRESS_LIKE = re.compile(r"^[0-9A-F]{2}([-:][0-9A-F]{2}){5}$", re.I)


def real_name(name):
    """A name, or "" for none: BlueZ calls a nameless LE advert by its
    address ("98-34-8C-03-82-DC"), which no classic device is called."""
    name = (name or "").strip()
    return "" if ADDRESS_LIKE.match(name) else name


BASE_UUID = "-0000-1000-8000-00805f9b34fb"


def is_audio(d):
    return bool(AUDIO_UUIDS & set(d.get("UUIDs", [])) or (d.get("Class", 0) >> 8) & 0x1F == 0x04)


def advert_vendors(d):
    """Company IDs an advert names as 16-bit service UUIDs: realme's buds
    put theirs (0x05D6) next to Fast Pair's in their nameless advert."""
    out = set()
    for u in d.get("UUIDs", []):
        u = str(u).lower()
        if u.startswith("0000") and u.endswith(BASE_UUID) and u[4:8] != "fe2c":
            out.add(int(u[4:8], 16))
    return out


def device_vendor(d):
    """The company ID in a classic device's Device ID ("bluetooth:v05D6p…";
    a "usb:" one counts USB vendors, another numbering)."""
    m = re.match(r"bluetooth:v([0-9A-F]{4})", str(d.get("Modalias", "")), re.I)
    return int(m.group(1), 16) if m else None


def pick_known(devices, advert):
    """Earbuds paired with this computer already, among devices [(path,
    Device1 props)], that the Fast Pair advert `advert` (Device1 props) comes
    from: the advert's own device, or a paired audio device with its name,
    the last three bytes of its address, the company it names (the one
    such), or the front three (the one such). Earbuds in pairing mode
    advertise from a nameless address of their own, so "already paired"
    can't be told by name."""
    address = str(advert.get("Address", "")).upper()
    name = real_name(advert.get("Alias") or advert.get("Name"))
    paired = [(p, d) for p, d in devices if d.get("Paired") and is_audio(d)]
    for p, d in devices:
        if d.get("Paired") and str(d.get("Address", "")).upper() == address:
            return p
    for p, d in paired:
        if name and name in (d.get("Alias"), d.get("Name")):
            return p
    for p, d in paired:
        if address and str(d.get("Address", "")).upper()[-8:] == address[-8:]:
            return p
    vendors = advert_vendors(advert)
    by_vendor = [p for p, d in paired if device_vendor(d) in vendors]
    if len(by_vendor) == 1:
        return by_vendor[0]
    by_front = [p for p, d in paired if address and str(d.get("Address", "")).upper()[:8] == address[:8]]
    return by_front[0] if len(by_front) == 1 else None


def pick_twin(devices, name, address, fallback=False):
    """The earbuds' classic (BR/EDR) side among devices [(path, Device1
    props)] found by a classic scan, for the Fast Pair advert at `address`
    (LE) called `name` (or ""): an unpaired audio device with that name,
    or that address, or one sharing its last three bytes (buds give their LE
    and classic sides addresses that differ only at the front). With
    `fallback` (the scan is over), the one audio device in pairing mode
    that's close by, when there's exactly one."""
    name = real_name(name)
    tail = (address or "").upper()[-8:]
    near = []
    for path, d in devices:
        if d.get("Paired") or not str(d.get("Address", "")).strip():
            continue
        if not is_audio(d):
            continue
        if name and name in (d.get("Alias"), d.get("Name")):
            return path
        if tail and str(d.get("Address", "")).upper()[-8:] == tail:
            return path
        if (d.get("RSSI") or -127) >= -70:
            near.append(path)
    return near[0] if fallback and len(near) == 1 else None


def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


class Watcher:
    def __init__(self):
        from gi.repository import Gio, GLib
        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        self.devices = {}  # path -> info
        self.other = set()  # paths asked about that had no Fast Pair data
        self.last = None
        self.adapter = None
        self.scanning = False
        self.cached = (0, {})

    def call(self, path, iface, method, args=None, sig=None, timeout=10000):
        params = self.GLib.Variant(sig, args) if sig else None
        r = self.bus.call_sync("org.bluez", path, iface, method, params, None, self.Gio.DBusCallFlags.NONE, timeout, None)
        return r.unpack() if r is not None else ()

    def objects(self):
        return self.call("/", "org.freedesktop.DBus.ObjectManager", "GetManagedObjects")[0]

    def recent_objects(self):
        """objects(), at most 5 s old (asked only while earbuds are in pairing
        mode nearby)"""
        at, objs = self.cached
        if time.monotonic() - at > 5:
            objs = self.objects()
            self.cached = (time.monotonic(), objs)
        return objs

    def known(self, path):
        """The paired earbuds the advert at `path` comes from: {"address",
        "name", "connected"}, or None."""
        objs = self.recent_objects()
        advert = objs.get(path, {}).get("org.bluez.Device1")
        if not advert:
            return None
        devices = [(p, i["org.bluez.Device1"]) for p, i in objs.items() if "org.bluez.Device1" in i]
        twin = pick_known(devices, advert)
        if not twin:
            return None
        d = objs[twin]["org.bluez.Device1"]
        return {"address": d.get("Address", ""), "name": d.get("Alias") or d.get("Name") or "", "connected": bool(d.get("Connected"))}

    def find_adapter(self):
        for path, ifaces in self.objects().items():
            if "org.bluez.Adapter1" in ifaces:
                return path, ifaces["org.bluez.Adapter1"]
        return None, None

    def seen(self, path, props):
        sd = props.get("ServiceData") or {}
        data = sd.get(FAST_PAIR)
        if data is None:
            return
        info = parse(data)
        old = self.devices.get(path, {})
        self.devices[path] = dict(old, **info, address=props.get("Address", old.get("address", "")),
                                  name=props.get("Alias") or props.get("Name") or old.get("name", ""),
                                  rssi=props.get("RSSI", old.get("rssi", -100)), at=time.monotonic())

    def on_signal(self, conn, sender, path, iface, signal, params):
        if signal == "InterfacesAdded":
            obj, ifaces = params.unpack()
            if "org.bluez.Device1" in ifaces:
                self.seen(obj, ifaces["org.bluez.Device1"])
        elif signal == "InterfacesRemoved":
            # (BlueZ forgets a device it hasn't heard from: so does this.
            # Phones and watches change their addresses every ~15 minutes,
            # and `other` kept every one it was ever asked about.)
            obj, ifaces = params.unpack()
            if "org.bluez.Device1" in ifaces:
                self.other.discard(obj)
                self.devices.pop(obj, None)
        elif signal == "PropertiesChanged":
            # Each advert from every device nearby comes here (DuplicateData):
            # the bus is asked about a device only when this can't tell
            iface_name, changed, _ = params.unpack()
            if iface_name != "org.bluez.Device1":
                return
            sd = changed.get("ServiceData")
            if sd is not None:
                if FAST_PAIR not in sd:
                    return
                self.other.discard(path)
                if path in self.devices:
                    self.seen(path, changed)
                    return
            elif "RSSI" in changed:
                if path in self.devices:
                    self.devices[path].update(rssi=changed["RSSI"], at=time.monotonic())
                    return
                if path in self.other:
                    return  # (asked before: not Fast Pair)
            else:
                return
            try:
                (props,) = self.call(path, "org.freedesktop.DBus.Properties", "GetAll", ("org.bluez.Device1",), "(s)")
            except Exception:  # noqa: BLE001
                return
            self.seen(path, props)
            if path not in self.devices:
                if len(self.other) >= 2000:  # (in case removals never come)
                    self.other.clear()
                self.other.add(path)

    def report(self):
        now = time.monotonic()
        self.devices = {p: d for p, d in self.devices.items() if now - d["at"] < 20}
        state = []
        for path, d in self.devices.items():
            out = {k: d.get(k) for k in ("address", "name", "model", "pairing", "rssi", "battery", "showUi")}
            if d.get("pairing"):
                try:
                    out["known"] = self.known(path)
                except Exception:  # noqa: BLE001
                    out["known"] = None
            state.append(out)
        state.sort(key=lambda d: d["address"])
        if state != self.last:
            emit({"devices": state})
            self.last = state
        return True

    def on_battery_power(self):
        try:
            r = self.Gio.bus_get_sync(self.Gio.BusType.SYSTEM, None).call_sync(
                "org.freedesktop.UPower", "/org/freedesktop/UPower", "org.freedesktop.DBus.Properties", "Get",
                self.GLib.Variant("(ss)", ("org.freedesktop.UPower", "OnBattery")), None, 0, 3000, None)
            return bool(r.unpack()[0])
        except Exception:  # noqa: BLE001
            return False

    def scan(self, on):
        if not self.adapter or on == self.scanning:
            return
        try:
            if on:
                self.call(self.adapter, "org.bluez.Adapter1", "SetDiscoveryFilter",
                          ({"Transport": self.GLib.Variant("s", "le"), "DuplicateData": self.GLib.Variant("b", True)},), "(a{sv})")
                self.call(self.adapter, "org.bluez.Adapter1", "StartDiscovery")
            else:
                self.call(self.adapter, "org.bluez.Adapter1", "StopDiscovery")
            self.scanning = on
        except Exception:  # noqa: BLE001
            self.scanning = False

    def duty(self):
        """Every 30 s: scan 4 s (continuously on the charger), only while powered."""
        self.adapter, props = self.find_adapter()
        if not self.adapter or not props.get("Powered"):
            self.scan(False)
            return True
        if not self.on_battery_power():
            self.scan(True)
            return True
        self.scan(True)
        self.GLib.timeout_add_seconds(4, lambda: (self.scan(False), False)[1])
        return True

    # Connect: the classic side when it shows up, else the LE side
    def connect(self, address, hint=""):
        """Returns {"ok", "error"}. `hint`: the name the card showed."""
        self.adapter, _ = self.find_adapter()
        try:
            found = [(p, i["org.bluez.Device1"]) for p, i in self.objects().items()
                     if "org.bluez.Device1" in i and i["org.bluez.Device1"].get("Address") == address]
            path = found[0][0] if found else None
            name = real_name(hint) or (real_name(found[0][1].get("Alias") or found[0][1].get("Name")) if found else "")
            dev = "org.bluez.Device1"
            # Paired already (an advert in pairing mode says nothing about
            # whom they've met): just connect them
            known = pick_known([(p, i[dev]) for p, i in self.objects().items() if dev in i], found[0][1]) if found else None
            if known:
                return self.connect_known(known)
            # (Earbuds pair over classic Bluetooth: pairing with the LE
            # advert itself is refused, "failed to pair". Without the classic
            # side there's nothing to pair with.)
            target = self.classic_twin(name, address)
            if not target:
                raise RuntimeError("Couldn't find the earbuds. Hold the button on the case until the light flashes, then try again." if path
                                   else "The earbuds went out of range. Open the case near the computer and try again.")
            try:
                self.call(target, dev, "Pair", timeout=60000)
            except Exception as e:  # noqa: BLE001
                if "AlreadyExists" not in str(e):
                    raise
            self.call(target, "org.freedesktop.DBus.Properties", "Set", (dev, "Trusted", self.GLib.Variant("b", True)), "(ssv)")
            self.call(target, dev, "Connect", timeout=30000)
            return {"ok": True, "error": "", "address": self.address_of(target)}
        except Exception as e:  # noqa: BLE001
            msg = getattr(e, "message", str(e))
            return {"ok": False, "error": "Pairing didn't finish. Put the earbuds in pairing mode and try again."
                    if "Failed" in msg or "Timeout" in msg else msg.split(":")[-1].strip()}

    def address_of(self, path):
        try:
            return self.call(path, "org.freedesktop.DBus.Properties", "Get", ("org.bluez.Device1", "Address"), "(ss)")[0]
        except Exception:  # noqa: BLE001
            return ""

    def connect_known(self, path):
        """Connects earbuds paired before (a second try when BlueZ is busy
        with them: "Device or resource busy" when they reconnect at the same
        moment)."""
        dev = "org.bluez.Device1"
        known = {}
        try:
            known = self.call(path, "org.freedesktop.DBus.Properties", "GetAll", (dev,), "(s)")[0]
            address = known.get("Address", "")
            self.call(path, "org.freedesktop.DBus.Properties", "Set", (dev, "Trusted", self.GLib.Variant("b", True)), "(ssv)")
            for attempt in range(2):
                try:
                    self.call(path, dev, "Connect", timeout=30000)
                    return {"ok": True, "error": "", "address": address}
                except Exception as e:  # noqa: BLE001
                    msg = getattr(e, "message", str(e))
                    if attempt or not re.search(r"busy|InProgress|AlreadyConnected", msg, re.I):
                        raise
                    time.sleep(2)
        except Exception as e:  # noqa: BLE001
            msg = getattr(e, "message", str(e))
            if "AlreadyConnected" in msg:
                return {"ok": True, "error": "", "address": known.get("Address", "")}
        # (in pairing mode and paired, yet they won't connect: they were reset,
        # or they're busy with a phone; the card offers to pair them again)
        return {"ok": False, "known": known.get("Address", ""),
                "error": "They didn't connect. If they were reset, pair them again."}

    def repair(self, address, known, hint=""):
        """Forgets the old pairing `known` (an address), then pairs the
        earbuds afresh (connect)."""
        self.adapter, _ = self.find_adapter()
        dev = "org.bluez.Device1"
        objs = self.objects()
        old = next((p for p, i in objs.items() if dev in i and i[dev].get("Address") == known), None)
        name = real_name(hint) or (real_name(objs[old][dev].get("Alias") or objs[old][dev].get("Name")) if old else "")
        if old and self.adapter:
            try:
                self.call(self.adapter, "org.bluez.Adapter1", "RemoveDevice", (old,), "(o)")
            except Exception:  # noqa: BLE001
                pass
        target = self.classic_twin(name, address)
        if not target:
            return {"ok": False, "error": "Couldn't find the earbuds. Hold the button on the case until the light flashes, then pair them in Bluetooth settings."}
        try:
            try:
                self.call(target, dev, "Pair", timeout=60000)
            except Exception as e:  # noqa: BLE001
                if "AlreadyExists" not in str(e):
                    raise
            self.call(target, "org.freedesktop.DBus.Properties", "Set", (dev, "Trusted", self.GLib.Variant("b", True)), "(ssv)")
            self.call(target, dev, "Connect", timeout=30000)
            return {"ok": True, "error": "", "address": self.address_of(target)}
        except Exception as e:  # noqa: BLE001
            msg = getattr(e, "message", str(e))
            return {"ok": False, "error": "Pairing didn't finish. Put the earbuds in pairing mode and try again."
                    if "Failed" in msg or "Timeout" in msg else msg.split(":")[-1].strip()}

    def classic_twin(self, name, address):
        """The earbuds' classic side (pick_twin), found by a short classic
        (BR/EDR) scan."""
        if not self.adapter:
            return None
        try:
            self.call(self.adapter, "org.bluez.Adapter1", "StopDiscovery")
        except Exception:  # noqa: BLE001
            pass
        try:
            self.call(self.adapter, "org.bluez.Adapter1", "SetDiscoveryFilter", ({"Transport": self.GLib.Variant("s", "bredr")},), "(a{sv})")
            self.call(self.adapter, "org.bluez.Adapter1", "StartDiscovery")
        except Exception:  # noqa: BLE001
            return None
        def devices():
            return [(p, i["org.bluez.Device1"]) for p, i in self.objects().items() if "org.bluez.Device1" in i]
        try:
            for _ in range(10):
                time.sleep(1)
                p = pick_twin(devices(), name, address)
                if p:
                    return p
            return pick_twin(devices(), name, address, fallback=True)
        finally:
            try:
                self.call(self.adapter, "org.bluez.Adapter1", "StopDiscovery")
            except Exception:  # noqa: BLE001
                pass
            self.scanning = False

    def run(self):
        self.bus.signal_subscribe("org.bluez", "org.freedesktop.DBus.ObjectManager", "InterfacesAdded", None, None,
                                  self.Gio.DBusSignalFlags.NONE, self.on_signal)
        self.bus.signal_subscribe("org.bluez", "org.freedesktop.DBus.ObjectManager", "InterfacesRemoved", None, None,
                                  self.Gio.DBusSignalFlags.NONE, self.on_signal)
        self.bus.signal_subscribe("org.bluez", "org.freedesktop.DBus.Properties", "PropertiesChanged", None, None,
                                  self.Gio.DBusSignalFlags.NONE, self.on_signal)
        for path, ifaces in self.objects().items():
            if "org.bluez.Device1" in ifaces:
                self.seen(path, ifaces["org.bluez.Device1"])
        self.duty()
        self.GLib.timeout_add_seconds(30, self.duty)
        self.GLib.timeout_add_seconds(1, self.report)
        loop = self.GLib.MainLoop()
        try:
            loop.run()
        finally:
            self.scan(False)


def main(argv):
    if argv[:1] == ["watch"]:
        try:
            Watcher().run()
        except Exception as e:  # noqa: BLE001
            emit({"devices": [], "error": str(e)})
            return 1
        return 0
    if len(argv) in (2, 3) and argv[0] == "connect":
        try:
            r = Watcher().connect(argv[1], argv[2] if len(argv) == 3 else "")
        except Exception as e:  # noqa: BLE001
            r = {"ok": False, "error": str(e)}
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    if len(argv) in (3, 4) and argv[0] == "repair":
        try:
            r = Watcher().repair(argv[1], argv[2], argv[3] if len(argv) == 4 else "")
        except Exception as e:  # noqa: BLE001
            r = {"ok": False, "error": str(e)}
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    print(json.dumps({"error": "usage: fastpair.py watch | connect ADDRESS [NAME] | repair ADDRESS KNOWN [NAME]"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
