"""Bluetooth LE through BlueZ: being found by phones, and finding them.

Receiving (a phone sends to this computer without a shared network):
  advertise   LEAdvertisingManager1: an extended advertisement with the full
              0xFEF3 service data (ble_format.advertisement), and a legacy one
              with the 17-byte header for phones that only scan those
  GATT server GattManager1.RegisterApplication: service 0xFEF3 with the
              advertisement in slot 0 (read), and the weave pair: the phone
              writes packets to …0101 and gets ours as notifications of …0102
              (weave.py). One phone at a time: BlueZ sends a characteristic's
              notifications to everyone subscribed.
Sending (to a phone on its Receive screen, visible to everyone):
  scan        discovery filtered to LE and the 0xFEF3 service, only while the
              share sheet looks (an open-ended unfiltered scan stops BlueZ
              reconnecting LE mice and keyboards on some chips)
  connect     an L2CAP channel when the phone advertises a PSM (faster to set
              up), else GATT: Device1.Connect, then the weave pair
Bluetooth off: nothing is powered on from here; the shell offers a button
(power_on), the user's choice.

All D-Bus goes through a bus object (gbus.GioBus, or a fake in the tests).
"""
import asyncio
import ctypes
import ctypes.util
import errno
import os
import socket
import sys

from . import ble_format, debuglog
from .gbus import DBusError
from .transport import TransportClosed
from .weave import MIN_PACKET, BleSocketTransport, L2capMessages, WeaveError, WeaveSocket

BLUEZ = "org.bluez"
OM = "org.freedesktop.DBus.ObjectManager"
PROPS = "org.freedesktop.DBus.Properties"
ADAPTER = "org.bluez.Adapter1"
DEVICE = "org.bluez.Device1"
GATT_SERVICE = "org.bluez.GattService1"
GATT_CHAR = "org.bluez.GattCharacteristic1"
ADV = "org.bluez.LEAdvertisement1"
ADV_MANAGER = "org.bluez.LEAdvertisingManager1"
GATT_MANAGER = "org.bluez.GattManager1"
ROOT = "/org/irice/quickshare"
SERVICE_PATH = ROOT + "/service0"
SLOT_PATH = SERVICE_PATH + "/char0"
WRITE_PATH = SERVICE_PATH + "/char1"
NOTIFY_PATH = SERVICE_PATH + "/char2"
ADV_EXTENDED = ROOT + "/adv0"
ADV_LEGACY = ROOT + "/adv1"
ADV_WAKE = ROOT + "/adv2"
# "Someone nearby is sharing": while this computer looks for devices to send
# to, the broadcast Android's Quick Share sends then. Phones nearby that
# hear it make themselves visible (their "device nearby is sharing" prompt)
# instead of only when their Quick Share screen is open. Service 0xFE2C with
# this fixed data (as RQuickShare sends it: core_lib/src/hdl/blea.rs)
WAKE_UUID = "0000fe2c-0000-1000-8000-00805f9b34fb"
WAKE_DATA = bytes([252, 18, 142, 1, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 191, 45, 91, 160, 225, 216, 117, 36, 202, 0])
CONNECT_TIMEOUT = 20
SEEN_FOR = 30  # a phone not heard from for this long is gone
SUBSCRIBE_WAIT = 5  # a phone that wrote but never subscribed to our packets

CHAR_IFACE = {"methods": {"ReadValue": ("a{sv}", "ay"), "WriteValue": ("aya{sv}", ""),
                          "StartNotify": ("", ""), "StopNotify": ("", ""), "Confirm": ("", "")},
              "properties": {"UUID": "s", "Service": "o", "Flags": "as", "Value": "ay", "Notifying": "b"}}
SERVICE_IFACE = {"properties": {"UUID": "s", "Primary": "b", "Characteristics": "ao"}}
ADV_IFACE = {"methods": {"Release": ("", "")},
             "properties": {"Type": "s", "ServiceData": "a{sv}", "SecondaryChannel": "s", "Includes": "as"}}
OM_IFACE = {"methods": {"GetManagedObjects": ("", "a{oa{sa{sv}}}")},
            "signals": {"InterfacesAdded": "oa{sa{sv}}", "InterfacesRemoved": "oas"}}


def _bytes(v):
    return bytes(v) if v is not None else b""


def advertisement_properties(service_data, extended):
    """LEAdvertisement1's properties for one of the two advertisements."""
    props = {"Type": ("s", "peripheral"),
             "ServiceData": ("a{sv}", {ble_format.COPRESENCE_UUID: ("ay", service_data)}),
             "Includes": ("as", [])}
    if extended:
        # (asks BlueZ for an extended advertisement even when it would fit)
        props["SecondaryChannel"] = ("s", "1M")
    return props


def wake_properties():
    """LEAdvertisement1's properties for the "someone is sharing" broadcast."""
    return {"Type": ("s", "broadcast"), "ServiceData": ("a{sv}", {WAKE_UUID: ("ay", WAKE_DATA)}),
            "Includes": ("as", [])}


def gatt_objects(slot_value):
    """The GATT application, as GetManagedObjects returns it."""
    def char(uuid, flags):
        return {GATT_CHAR: {"UUID": ("s", uuid), "Service": ("o", SERVICE_PATH), "Flags": ("as", flags)}}
    return {
        SERVICE_PATH: {GATT_SERVICE: {"UUID": ("s", ble_format.COPRESENCE_UUID), "Primary": ("b", True),
                                      "Characteristics": ("ao", [SLOT_PATH, WRITE_PATH, NOTIFY_PATH])}},
        SLOT_PATH: char(ble_format.SLOT_UUID.format(0), ["read"]),
        # (write with response: one packet at a time, so they arrive in order)
        WRITE_PATH: char(ble_format.WEAVE_WRITE_UUID, ["write"]),
        # Indicate, as Nearby's own peripherals (phones; the Apple code's
        # GNSPeripheralServiceManager.m CreateWeaveOutgoingCharacteristic);
        # notify too, for a central that subscribes that way
        NOTIFY_PATH: char(ble_format.WEAVE_NOTIFY_UUID, ["indicate", "notify"]),
    }


def discovery_filter(uuids=True):
    f = {"Transport": ("s", "le"), "DuplicateData": ("b", True)}
    if uuids:
        f["UUIDs"] = ("as", [ble_format.COPRESENCE_UUID])
    return f


class _NotifyLink:
    """Packets to the phone: indications (or notifications) of …0102.

    Never waits: WriteValue handlers call this, and BlueZ answers the phone's
    next ATT request (its subscription, say) only after ours returns. Before
    the phone subscribes, packets wait in a queue; StartNotify sends them.
    (A first version waited here, inside the phone's write, for a
    subscription that could only come after it: Nearby's central gives up on
    the confirm after 2 s — GNSCentralPeerManager.m
    kMaxConnectionConfirmWaitTimeInSeconds — which is what a phone did.)"""

    def __init__(self, ble):
        self.ble = ble

    async def send(self, packet):
        if not self.ble.notifying or self.ble.pending:
            self.ble.pending.append(packet)
            self.ble.log_packet("queued (not subscribed yet)", packet)
            return
        self.ble.notify(packet)


class _WriteLink:
    """Packets to the phone, as the central: writes to its …0101."""

    def __init__(self, bus, path):
        self.bus, self.path = bus, path

    async def send(self, packet):
        try:
            await self.bus.call(BLUEZ, self.path, GATT_CHAR, "WriteValue", "aya{sv}",
                                (packet, {"type": ("s", "request")}))
        except DBusError as e:
            raise TransportClosed(f"Bluetooth write failed ({e})") from e


class Bluetooth:
    """One adapter's Quick Share roles. on_session(transport) gets each
    phone that connects to us; on_state(state) hears about power changes."""

    def __init__(self, bus, on_session=None, on_state=None, scan_uuids=True):
        self.bus = bus
        self.on_session = on_session or (lambda transport: None)
        self.on_state = on_state or (lambda state: None)
        self.scan_uuids = scan_uuids
        self.adapter = None
        self.address = ""
        self.powered = False
        self.advertising = []
        self.served = False
        self.slot_value = b""
        self.notifying = False
        self.notify_value = b""
        self.pending = []  # packets for the phone before it subscribed
        self.session = None  # (device path, WeaveSocket) of the phone connected to us
        self.scanning = False
        self.waking = False  # the "someone is sharing" broadcast is on
        self.seen = {}  # device path → {"endpoint_id", "endpoint_info", "psm", "address", "type", "at"}
        self._found = self._lost = None
        self._subs = []
        self._prune = None

    def _log(self, what):
        debuglog.log("bluetooth", what)

    def log_packet(self, what, packet, char=""):
        """A weave packet in the log: its size, and the bytes of control
        packets (handshake and errors: nothing secret); data packets carry
        encrypted frames and are only counted."""
        if not debuglog.enabled() or not packet:
            return
        h = packet[0]
        kind = f"control {packet[:8].hex()}" if h & 0x80 else \
            f"data #{(h >> 4) & 7}{' first' if h & 8 else ''}{' last' if h & 4 else ''}"
        self._log(f"{what}{' ' + char if char else ''}: {len(packet)} bytes, {kind}")

    def notify(self, packet):
        self.notify_value = packet
        self.log_packet("→ indicate", packet, "0102")
        self.bus.emit(NOTIFY_PATH, PROPS, "PropertiesChanged", "sa{sv}as",
                      (GATT_CHAR, {"Value": ("ay", packet)}, []))

    def _flush(self):
        pending, self.pending = self.pending, []
        for p in pending:
            self.notify(p)

    # ── Adapter ──────────────────────────────────────────────────────────
    async def start(self):
        """Finds the adapter and follows it. False without one (or BlueZ)."""
        self._subs.append(self.bus.subscribe(BLUEZ, PROPS, "PropertiesChanged", None, self._changed))
        self._subs.append(self.bus.subscribe(BLUEZ, OM, "InterfacesAdded", None, self._added))
        self._subs.append(self.bus.subscribe(BLUEZ, OM, "InterfacesRemoved", None, self._removed))
        await self.refresh()
        return self.adapter is not None

    async def refresh(self):
        try:
            (objects,) = await self.bus.call(BLUEZ, "/", OM, "GetManagedObjects")
        except DBusError as e:
            self._log(f"BlueZ isn't answering: {e}")
            objects = {}
        adapters = sorted(p for p, ifaces in objects.items() if ADAPTER in ifaces)
        was = (self.adapter, self.powered)
        if adapters:
            self.adapter = adapters[0]
            a = objects[self.adapter][ADAPTER]
            self.powered = bool(a.get("Powered"))
            self.address = a.get("Address", "")
        else:
            self.adapter, self.powered, self.address = None, False, ""
        if (self.adapter, self.powered) != was:
            self.on_state(self.state())
        return objects

    def state(self):
        return {"available": self.adapter is not None, "powered": self.powered}

    async def power_on(self):
        """Only ever called for the user's "Turn on Bluetooth"."""
        if self.adapter:
            await self.bus.set_property(BLUEZ, self.adapter, ADAPTER, "Powered", "b", True)

    def _changed(self, path, signal, args):
        iface, changed, _ = args
        if iface == ADAPTER and path == self.adapter and "Powered" in changed:
            self.powered = bool(changed["Powered"])
            self._log(f"adapter powered: {self.powered}")
            self.on_state(self.state())
            if not self.powered:
                self._end_session("Bluetooth was turned off")
        elif iface == DEVICE:
            noted = {k: changed[k] for k in ("Connected", "Paired", "Bonded", "Trusted", "ServicesResolved")
                     if k in changed}
            if noted:
                self._log(f"device {path.rsplit('/', 1)[-1]}: {noted}")
                if noted.get("Paired") and self.session and path == self.session[0]:
                    # (Quick Share never pairs: BlueZ's own profiles did, see
                    # sharing-check.py pairing())
                    self._log("BlueZ paired with the phone during Quick Share: its Ranging (RAP) profile "
                              "most likely asked; Settings → Sharing can turn that off")
            if self.session and path == self.session[0] and changed.get("Connected") is False:
                self._end_session("the phone disconnected")
            if self.scanning and ("ServiceData" in changed or "RSSI" in changed):
                asyncio.ensure_future(self._look_at(path, changed))

    def _added(self, path, signal, args):
        obj, ifaces = args
        if ADAPTER in ifaces and self.adapter is None:
            asyncio.ensure_future(self.refresh())
        if DEVICE in ifaces and self.scanning:
            asyncio.ensure_future(self._look_at(obj, ifaces[DEVICE]))

    def _removed(self, path, signal, args):
        obj, ifaces = args
        if ADAPTER in ifaces and obj == self.adapter:
            self.adapter, self.powered = None, False
            self.advertising, self.served = [], False
            self.on_state(self.state())
        if DEVICE in ifaces:
            self._forget(obj)

    # ── Being found (receiving) ──────────────────────────────────────────
    async def max_advertisement(self):
        """The longest advertising data this controller takes (BlueZ's
        LEAdvertisingManager1.SupportedCapabilities MaxAdvLen; 31 for a
        controller without extended advertising), or None if BlueZ doesn't say."""
        if not self.adapter:
            return None
        try:
            caps = await self.bus.get_property(BLUEZ, self.adapter, ADV_MANAGER, "SupportedCapabilities")
        except DBusError:
            return None
        n = (caps or {}).get("MaxAdvLen")
        return int(n) if n else None

    async def advertise(self, advertisement, extended=None):
        """Advertises (or re-advertises) this computer; GATT server included.
        advertisement: the full one (GATT slot 0); extended: what fits in
        the extended advertisement (the same, or with a shorter name), or
        False to send the legacy header only."""
        await self.stop_advertising()
        if not (self.adapter and self.powered):
            return False
        self.slot_value = advertisement
        if not self.served:
            await self._serve()
        if extended is None:
            extended = advertisement
        ok_ext = bool(extended) and await self._register_adv(ADV_EXTENDED, advertisement_properties(extended, True))
        header = ble_format.header([(ble_format.SERVICE_ID, advertisement)], extended=ok_ext)
        ok_legacy = await self._register_adv(ADV_LEGACY, advertisement_properties(header, False))
        self._log(f"advertising: extended {ok_ext}, legacy {ok_legacy}")
        return ok_ext or ok_legacy

    async def _register_adv(self, path, props):
        async def release(args, sender):
            if path in self.advertising:
                self.advertising.remove(path)
            return ()
        await self.bus.export(path, {ADV: ADV_IFACE}, lambda iface: props if iface == ADV else {},
                              {(ADV, "Release"): release})
        try:
            await self.bus.call(BLUEZ, self.adapter, ADV_MANAGER, "RegisterAdvertisement", "oa{sv}", (path, {}))
        except DBusError as e:
            if e.name.endswith("AlreadyExists"):  # (kept by BlueZ across a power cycle)
                self.advertising.append(path)
                return True
            self._log(f"advertisement {path} refused: {e.name} {e}")
            await self.bus.unexport(path)
            return False
        self.advertising.append(path)
        return True

    async def stop_advertising(self):
        for path in (ADV_EXTENDED, ADV_LEGACY):
            if path in self.advertising and self.adapter:
                try:
                    await self.bus.call(BLUEZ, self.adapter, ADV_MANAGER, "UnregisterAdvertisement", "o", (path,))
                except DBusError:
                    pass
            await self.bus.unexport(path)
        self.advertising = []

    async def _serve(self):
        objects = gatt_objects(self.slot_value)

        async def managed(args, sender):
            return (gatt_objects(self.slot_value),)

        async def read_slot(args, sender):
            (options,) = args
            offset = int(options.get("offset", 0))
            value = self.slot_value[offset:]
            self._log(f"← read slot 0 (offset {offset}): {len(value)} bytes"
                      f" from {str(options.get('device', '?')).rsplit('/', 1)[-1]}")
            return (value,)

        async def write(args, sender):
            value, options = args
            packet = _bytes(value)
            self.log_packet(f"← write ({options.get('type', 'request')}, MTU {options.get('mtu', '?')})", packet, "0101")
            await self._packet(packet, options)
            return ()

        async def start_notify(args, sender):
            self._log(f"← subscribed to 0102 ({len(self.pending)} packets waiting)")
            self.notifying = True
            self._flush()
            return ()

        async def stop_notify(args, sender):
            self._log("← unsubscribed from 0102")
            self.notifying = False
            self.pending = []
            self._end_session("the phone stopped listening")
            return ()

        async def confirm(args, sender):
            return ()  # (an indication arrived)

        async def nothing(args, sender):
            self._log("← an unsupported GATT request")
            raise DBusError("org.bluez.Error.NotSupported", "not supported")

        def char_props(path):
            p = dict(objects[path][GATT_CHAR])
            if path == NOTIFY_PATH:
                p["Value"] = ("ay", self.notify_value)
                p["Notifying"] = ("b", self.notifying)
            if path == SLOT_PATH:
                p["Value"] = ("ay", self.slot_value)
            return p

        await self.bus.export(ROOT, {OM: OM_IFACE}, lambda iface: {}, {(OM, "GetManagedObjects"): managed})
        await self.bus.export(SERVICE_PATH, {GATT_SERVICE: SERVICE_IFACE},
                              lambda iface: objects[SERVICE_PATH][GATT_SERVICE], {})
        for path, handlers in ((SLOT_PATH, {"ReadValue": read_slot}), (WRITE_PATH, {"WriteValue": write}),
                               (NOTIFY_PATH, {"StartNotify": start_notify, "StopNotify": stop_notify})):
            methods = {(GATT_CHAR, m): handlers.get(m, nothing) for m in CHAR_IFACE["methods"]}
            methods[(GATT_CHAR, "Confirm")] = confirm
            await self.bus.export(path, {GATT_CHAR: CHAR_IFACE}, lambda iface, path=path: char_props(path), methods)
        try:
            await self.bus.call(BLUEZ, self.adapter, GATT_MANAGER, "RegisterApplication", "oa{sv}", (ROOT, {}))
            self.served = True
        except DBusError as e:
            if e.name.endswith("AlreadyExists"):
                self.served = True
                return
            self._log(f"GATT server refused: {e.name} {e}")
            for path in (SLOT_PATH, WRITE_PATH, NOTIFY_PATH, SERVICE_PATH, ROOT):
                await self.bus.unexport(path)
            raise

    async def unserve(self):
        await self.stop_advertising()
        self._end_session("no longer visible")
        if self.served:
            try:
                await self.bus.call(BLUEZ, self.adapter, GATT_MANAGER, "UnregisterApplication", "o", (ROOT,))
            except DBusError:
                pass
            self.served = False
        for path in (SLOT_PATH, WRITE_PATH, NOTIFY_PATH, SERVICE_PATH, ROOT):
            await self.bus.unexport(path)

    async def _packet(self, packet, options):
        """A weave packet a phone wrote to …0101."""
        device = options.get("device", "")
        if self.session and self.session[1].closed:
            self.session = None
        if self.session is None:
            mtu = int(options.get("mtu", 0) or 0)
            sock = WeaveSocket(_NotifyLink(self), is_client=False,
                               max_packet=mtu - 3 if mtu > 23 else MIN_PACKET, log=self._log)
            self.session = (device, sock)
            self._log(f"phone connected over GATT (MTU {mtu or 'unknown'}, "
                      f"{'subscribed' if self.notifying else 'not subscribed yet'})")
            asyncio.ensure_future(self._await_subscription(sock))
            transport = BleSocketTransport(sock, is_client=False, peer=device, log=self._log)
            await sock.feed(packet)
            if sock.connected.is_set() and not sock.closed:
                self.on_session(transport)
            return
        if device and self.session[0] and device != self.session[0]:
            raise DBusError("org.bluez.Error.InProgress", "busy with another phone")
        await self.session[1].feed(packet)

    async def _await_subscription(self, sock):
        await asyncio.sleep(SUBSCRIBE_WAIT)
        if self.session and self.session[1] is sock and self.pending and not self.notifying:
            self._end_session("the phone never subscribed to our packets")

    def _end_session(self, why):
        if self.session:
            _, sock = self.session
            self.session = None
            self.pending = []
            self._log(f"session ended: {why}")
            sock._end(why)

    # ── Finding phones (sending) ─────────────────────────────────────────
    async def scan(self, found, lost):
        """found(key, {"endpoint_id", "endpoint_info", "psm"}) / lost(key)."""
        if not (self.adapter and self.powered) or self.scanning:
            return False
        self._found, self._lost = found, lost
        try:
            await self.bus.call(BLUEZ, self.adapter, ADAPTER, "SetDiscoveryFilter", "a{sv}",
                                (discovery_filter(self.scan_uuids),))
            await self.bus.call(BLUEZ, self.adapter, ADAPTER, "StartDiscovery")
        except DBusError as e:
            self._log(f"scan refused: {e.name} {e}")
            return False
        self.scanning = True
        self.waking = await self._register_adv(ADV_WAKE, wake_properties())
        self._log(f"looking for devices; nearby phones told someone is sharing: {self.waking}")
        self._prune = asyncio.ensure_future(self._prune_loop())
        objects = await self.refresh()
        for path, ifaces in objects.items():
            if DEVICE in ifaces and ifaces[DEVICE].get("RSSI") is not None:
                await self._look_at(path, ifaces[DEVICE])
        return True

    async def stop_scan(self):
        if not self.scanning:
            return
        self.scanning = False
        if self._prune:
            self._prune.cancel()
        if self.waking:
            self.waking = False
            try:
                await self.bus.call(BLUEZ, self.adapter, ADV_MANAGER, "UnregisterAdvertisement", "o", (ADV_WAKE,))
            except DBusError:
                pass
            if ADV_WAKE in self.advertising:
                self.advertising.remove(ADV_WAKE)
            await self.bus.unexport(ADV_WAKE)
        try:
            await self.bus.call(BLUEZ, self.adapter, ADAPTER, "StopDiscovery")
            await self.bus.call(BLUEZ, self.adapter, ADAPTER, "SetDiscoveryFilter", "a{sv}", ({},))
        except DBusError:
            pass
        for path in list(self.seen):
            self._forget(path)

    async def _look_at(self, path, props):
        data = (props.get("ServiceData") or {}).get(ble_format.COPRESENCE_UUID)
        if data is None:
            if path in self.seen and "RSSI" in props:
                self.seen[path]["at"] = asyncio.get_running_loop().time()
            return
        try:
            adv = ble_format.parse_service_data(_bytes(data))
        except ble_format.AdvertisementError:
            return  # a header only, a contacts-only beacon, another service
        if path not in self.seen or "address" not in self.seen[path]:
            try:
                address = await self.bus.get_property(BLUEZ, path, DEVICE, "Address")
                kind = await self.bus.get_property(BLUEZ, path, DEVICE, "AddressType")
            except DBusError:
                address, kind = "", "random"
            adv["address"], adv["address_type"] = address, kind
        else:
            adv["address"] = self.seen[path]["address"]
            adv["address_type"] = self.seen[path]["address_type"]
        adv["at"] = asyncio.get_running_loop().time()
        changed = {k: v for k, v in adv.items() if k != "at"} != \
            {k: v for k, v in self.seen.get(path, {}).items() if k != "at"}
        self.seen[path] = adv
        if changed and self._found:
            self._found(path, adv)

    def _forget(self, path):
        if self.seen.pop(path, None) is not None and self._lost:
            self._lost(path)

    async def _prune_loop(self):
        while self.scanning:
            await asyncio.sleep(5)
            now = asyncio.get_running_loop().time()
            for path, adv in list(self.seen.items()):
                if now - adv["at"] > SEEN_FOR:
                    self._forget(path)

    # ── Connecting to a phone (sending) ──────────────────────────────────
    async def connect(self, path):
        """A BleSocketTransport to the phone found at this device path:
        L2CAP when it advertises a PSM, GATT (weave) otherwise or if that fails."""
        adv = self.seen.get(path) or {}
        if adv.get("psm") and adv.get("address"):
            try:
                return await connect_l2cap(adv["address"], adv.get("address_type", "random"), adv["psm"],
                                           log=self._log)
            except (OSError, WeaveError, TransportClosed, asyncio.TimeoutError) as e:
                self._log(f"L2CAP failed ({e}); trying GATT")
        return await self.connect_gatt(path)

    async def connect_gatt(self, path):
        # A link left from an earlier advertising set shows no Quick Share
        # service: always start from a fresh connection
        try:
            if await self.bus.get_property(BLUEZ, path, DEVICE, "Connected"):
                await self.bus.call(BLUEZ, path, DEVICE, "Disconnect")
        except DBusError:
            pass
        resolved = asyncio.get_running_loop().create_future()

        def changed(p, signal, args):
            iface, props, _ = args
            if iface == DEVICE and props.get("ServicesResolved") and not resolved.done():
                resolved.set_result(True)
        sub = self.bus.subscribe(BLUEZ, PROPS, "PropertiesChanged", path, changed)
        try:
            try:
                await self.bus.call_async(BLUEZ, path, DEVICE, "Connect", timeout=CONNECT_TIMEOUT * 1000)
            except DBusError as e:
                raise TransportClosed(f"couldn't connect over Bluetooth ({e})") from e
            try:
                if not await self.bus.get_property(BLUEZ, path, DEVICE, "ServicesResolved"):
                    await asyncio.wait_for(resolved, CONNECT_TIMEOUT)
            except asyncio.TimeoutError as e:
                raise TransportClosed("the phone's Bluetooth services didn't show up") from e
        finally:
            self.bus.unsubscribe(sub)
        (objects,) = await self.bus.call(BLUEZ, "/", OM, "GetManagedObjects")
        write = notify = None
        for p, ifaces in objects.items():
            c = ifaces.get(GATT_CHAR)
            if c and p.startswith(path + "/"):
                if c.get("UUID", "").lower() == ble_format.WEAVE_WRITE_UUID:
                    write = p
                elif c.get("UUID", "").lower() == ble_format.WEAVE_NOTIFY_UUID:
                    notify = p
        if not (write and notify):
            await self._disconnect(path)
            raise TransportClosed("that phone has no Quick Share service over Bluetooth")
        # (20-byte packets: phones' Quick Share GATT servers keep the minimum MTU)
        sock = WeaveSocket(_WriteLink(self.bus, write), is_client=True, max_packet=MIN_PACKET, log=self._log)

        def value(p, signal, args):
            iface, props, _ = args
            if iface == GATT_CHAR and "Value" in props:
                asyncio.ensure_future(sock.feed(_bytes(props["Value"])))
        vsub = self.bus.subscribe(BLUEZ, PROPS, "PropertiesChanged", notify, value)
        try:
            await self.bus.call(BLUEZ, notify, GATT_CHAR, "StartNotify")
            await sock.connect()
        except (DBusError, WeaveError) as e:
            self.bus.unsubscribe(vsub)
            await self._disconnect(path)
            raise TransportClosed(f"the Bluetooth connection didn't start ({e})") from e
        bus = self.bus
        outer = self

        class _Messages:
            async def send(self, m):
                await sock.send(m)

            async def recv(self, timeout=None):
                return await sock.recv(timeout)

            async def close(self):
                await sock.close()
                bus.unsubscribe(vsub)
                try:
                    await bus.call(BLUEZ, notify, GATT_CHAR, "StopNotify")
                except DBusError:
                    pass
                await outer._disconnect(path)

        transport = BleSocketTransport(_Messages(), is_client=True, peer=path, log=self._log)
        await transport.start()
        return transport

    async def _disconnect(self, path):
        try:
            await self.bus.call(BLUEZ, path, DEVICE, "Disconnect")
        except DBusError:
            pass

    async def close(self):
        await self.stop_scan()
        await self.unserve()
        for s in self._subs:
            self.bus.unsubscribe(s)
        self._subs = []


# ── L2CAP channels (sockets: BlueZ has no D-Bus API for LE L2CAP) ─────────
AF_BLUETOOTH = 31
BTPROTO_L2CAP = 0
BDADDR_LE_PUBLIC, BDADDR_LE_RANDOM = 1, 2


class _SockaddrL2(ctypes.Structure):
    _fields_ = [("family", ctypes.c_ushort), ("psm", ctypes.c_ushort), ("bdaddr", ctypes.c_ubyte * 6),
                ("cid", ctypes.c_ushort), ("bdaddr_type", ctypes.c_ubyte)]


def sockaddr_l2(address, address_type, psm):
    """struct sockaddr_l2 for an LE address "AA:BB:…" (bdaddr is little-endian)."""
    parts = [int(x, 16) for x in address.split(":")]
    if len(parts) != 6 or not 0 < psm < 0x10000:
        raise ValueError("bad Bluetooth address or PSM")
    sa = _SockaddrL2()
    sa.family = AF_BLUETOOTH
    sa.psm = int.from_bytes(psm.to_bytes(2, "little"), sys.byteorder)  # (__le16)
    sa.bdaddr = (ctypes.c_ubyte * 6)(*reversed(parts))
    sa.cid = 0
    sa.bdaddr_type = BDADDR_LE_PUBLIC if address_type == "public" else BDADDR_LE_RANDOM
    return sa


class _L2capLink:
    def __init__(self, fd, loop):
        self.fd, self.loop = fd, loop

    async def send(self, data):
        view = memoryview(data)
        while view:
            try:
                n = os.write(self.fd, view)
                view = view[n:]
            except BlockingIOError:
                fut = self.loop.create_future()
                self.loop.add_writer(self.fd, lambda: fut.done() or fut.set_result(None))
                try:
                    await fut
                finally:
                    self.loop.remove_writer(self.fd)
            except OSError as e:
                raise TransportClosed(f"L2CAP write failed ({e})") from e


async def connect_l2cap(address, address_type, psm, timeout=10, log=None):
    """A BleSocketTransport over an LE L2CAP channel to a phone's PSM."""
    libc = ctypes.CDLL(ctypes.util.find_library("c") or "libc.so.6", use_errno=True)
    fd = libc.socket(AF_BLUETOOTH, socket.SOCK_SEQPACKET | socket.SOCK_NONBLOCK | socket.SOCK_CLOEXEC, BTPROTO_L2CAP)
    if fd < 0:
        raise OSError(ctypes.get_errno(), "no Bluetooth L2CAP sockets")
    loop = asyncio.get_running_loop()
    try:
        sa = sockaddr_l2(address, address_type, psm)
        if libc.connect(fd, ctypes.byref(sa), ctypes.sizeof(sa)) != 0:
            err = ctypes.get_errno()
            if err not in (errno.EINPROGRESS, errno.EAGAIN):
                raise OSError(err, os.strerror(err))
            fut = loop.create_future()
            loop.add_writer(fd, lambda: fut.done() or fut.set_result(None))
            try:
                await asyncio.wait_for(fut, timeout)
            finally:
                loop.remove_writer(fd)
            opt = ctypes.c_int(0)
            size = ctypes.c_uint(ctypes.sizeof(opt))
            libc.getsockopt(fd, socket.SOL_SOCKET, socket.SO_ERROR, ctypes.byref(opt), ctypes.byref(size))
            if opt.value:
                raise OSError(opt.value, os.strerror(opt.value))
    except BaseException:
        os.close(fd)
        raise
    messages = L2capMessages(_L2capLink(fd, loop))

    def readable():
        try:
            data = os.read(fd, 65536)
        except BlockingIOError:
            return
        except OSError:
            data = b""
        messages.feed(data)
        if not data:
            loop.remove_reader(fd)
    loop.add_reader(fd, readable)
    closing = messages.close

    async def close():
        await closing()
        loop.remove_reader(fd)
        try:
            os.close(fd)
        except OSError:
            pass
    messages.close = close
    try:
        await messages.request_data_connection()
    except BaseException:
        await close()
        raise
    transport = BleSocketTransport(messages, is_client=True, acks=True, peer=address, log=log, medium="ble")
    await transport.start()
    return transport
