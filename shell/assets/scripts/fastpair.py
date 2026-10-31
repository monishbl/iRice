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
  fastpair.py connect ADDRESS
                        pairs, trusts and connects them: {"ok", "error"}

Scanning is light: only while Bluetooth is on; continuous on the charger,
else 4 seconds every 30. Devices not heard from for 20 seconds are dropped.
(The device's real name and picture come from Google's model lookup, which
needs an API key: the Bluetooth name is used instead.)
"""
import json
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


def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


class Watcher:
    def __init__(self):
        from gi.repository import Gio, GLib
        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        self.devices = {}  # path -> info
        self.last = None
        self.adapter = None
        self.scanning = False

    def call(self, path, iface, method, args=None, sig=None, timeout=10000):
        params = self.GLib.Variant(sig, args) if sig else None
        r = self.bus.call_sync("org.bluez", path, iface, method, params, None, self.Gio.DBusCallFlags.NONE, timeout, None)
        return r.unpack() if r is not None else ()

    def objects(self):
        return self.call("/", "org.freedesktop.DBus.ObjectManager", "GetManagedObjects")[0]

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
        elif signal == "PropertiesChanged":
            iface_name, changed, _ = params.unpack()
            if iface_name == "org.bluez.Device1" and ("ServiceData" in changed or "RSSI" in changed):
                try:
                    (props,) = self.call(path, "org.freedesktop.DBus.Properties", "GetAll", ("org.bluez.Device1",), "(s)")
                except Exception:  # noqa: BLE001
                    return
                self.seen(path, props)

    def report(self):
        now = time.monotonic()
        self.devices = {p: d for p, d in self.devices.items() if now - d["at"] < 20}
        state = [{k: d.get(k) for k in ("address", "name", "model", "pairing", "rssi", "battery", "showUi")} for d in self.devices.values()]
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
    def connect(self, address):
        """Returns {"ok", "error"}."""
        self.adapter, _ = self.find_adapter()
        try:
            found = [(p, i["org.bluez.Device1"]) for p, i in self.objects().items()
                     if "org.bluez.Device1" in i and i["org.bluez.Device1"].get("Address") == address]
            path = found[0][0] if found else None
            name = (found[0][1].get("Alias") or found[0][1].get("Name") or "") if found else ""
            target = self.classic_twin(name) or path
            if not target:
                raise RuntimeError("The earbuds went out of range. Open the case near the computer and try again.")
            dev = "org.bluez.Device1"
            try:
                self.call(target, dev, "Pair", timeout=60000)
            except Exception as e:  # noqa: BLE001
                if "AlreadyExists" not in str(e):
                    raise
            self.call(target, "org.freedesktop.DBus.Properties", "Set", (dev, "Trusted", self.GLib.Variant("b", True)), "(ssv)")
            self.call(target, dev, "Connect", timeout=30000)
            return {"ok": True, "error": ""}
        except Exception as e:  # noqa: BLE001
            msg = getattr(e, "message", str(e))
            return {"ok": False, "error": "Pairing didn't finish. Put the earbuds in pairing mode and try again."
                    if "Failed" in msg or "Timeout" in msg else msg.split(":")[-1].strip()}

    def classic_twin(self, name):
        """An unpaired audio device with the same name, found by a short
        classic (BR/EDR) scan."""
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
        try:
            for _ in range(10):
                time.sleep(1)
                for p, ifaces in self.objects().items():
                    d = ifaces.get("org.bluez.Device1")
                    if not d or d.get("Paired") or not str(d.get("Address", "")).strip():
                        continue
                    audio = AUDIO_UUIDS & set(d.get("UUIDs", [])) or (d.get("Class", 0) >> 8) & 0x1F == 0x04
                    if audio and name and (d.get("Alias") == name or d.get("Name") == name):
                        return p
            return None
        finally:
            try:
                self.call(self.adapter, "org.bluez.Adapter1", "StopDiscovery")
            except Exception:  # noqa: BLE001
                pass
            self.scanning = False

    def run(self):
        self.bus.signal_subscribe("org.bluez", "org.freedesktop.DBus.ObjectManager", "InterfacesAdded", None, None,
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
    if len(argv) == 2 and argv[0] == "connect":
        try:
            r = Watcher().connect(argv[1])
        except Exception as e:  # noqa: BLE001
            r = {"ok": False, "error": str(e)}
        print(json.dumps(r))
        return 0 if r["ok"] else 1
    print(json.dumps({"error": "usage: fastpair.py watch | connect ADDRESS"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
