"""Quick Share over Bluetooth LE, piece by piece, without Bluetooth.

The advertisement bytes against google/nearby's layout (the source files
and lines are named in each test), weave packets and the BLE socket layer
(fragments, counters, control packets), BlueZ through a fake bus (what is
exported and called, never the real D-Bus), and the root Wi-Fi helper's
pure parts (checks, channel choice, D-Bus arguments, DHCP packets).
"""
import asyncio
import hashlib
import importlib.util
import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quickshare_ble_peer as bp  # noqa: E402
import quickshare_peer as qp  # noqa: E402

from nearby import ble_format, bluez, endpoint, weave, wifi  # noqa: E402
from nearby.gbus import DBusError, _intro_xml, _split  # noqa: E402
from nearby.transport import TransportClosed  # noqa: E402

spec = importlib.util.spec_from_file_location("qs_wifi_helper", qp.ENGINE / "qs-wifi-helper.py")
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


class Advertisement(unittest.TestCase):
    def test_connections_advertisement_layout(self):
        # connections/implementation/ble_advertisement.cc, operator ByteArray
        # (lines 206-251): version|pcp, service hash, endpoint id, info size,
        # info, 6-byte BT MAC (zeros when unset), UWB size, extra field
        info = endpoint.endpoint_info("Desk", endpoint.LAPTOP)
        raw = ble_format.connections_advertisement("AB12", info)
        self.assertEqual(raw[0], 0x23)  # version 1 << 5 | PCP 3 (point to point)
        self.assertEqual(raw[1:4], bytes.fromhex("fc9f5e"))
        self.assertEqual(raw[4:8], b"AB12")
        self.assertEqual(raw[8], len(info))
        self.assertEqual(raw[9:9 + len(info)], info)
        self.assertEqual(raw[9 + len(info):], b"\x00" * 6 + b"\x00" + b"\x00")

    def test_medium_advertisement_layout(self):
        # mediums/ble/ble_advertisement.cc operator ByteArray (regular):
        # version 2 << 5 | socket version 2 << 2 = 0x48, hash, uint32 size,
        # data, device token; then BleExtraFields (PSM bit 0x01 + uint16)
        data = b"x" * 40
        raw = ble_format.medium_advertisement(data, b"\x12\x34", psm=0x0081)
        self.assertEqual(raw[0], 0x48)
        self.assertEqual(raw[1:4], hashlib.sha256(b"NearbySharing").digest()[:3])
        self.assertEqual(struct.unpack(">I", raw[4:8])[0], 40)
        self.assertEqual(raw[8:48], data)
        self.assertEqual(raw[48:50], b"\x12\x34")
        self.assertEqual(raw[50:], b"\x01\x00\x81")
        # no PSM: no extra fields at all
        self.assertEqual(len(ble_format.medium_advertisement(data, b"\x12\x34")), 50)

    def test_matches_the_published_capture(self):
        # The receiver advertisement documented in open-quickshare's
        # BLE_RECEIVER_DISCOVERY.md §4.1: 48 | fc9f5e | 00 00 00 NN | 23 fc9f5e
        # <id> <info size> <info: 0x06 = laptop, visible> <mac+extra> | token
        info = bytes([0x06]) + bytes(16) + bytes([4]) + b"Desk"
        raw = ble_format.advertisement("WXYZ", info, b"\xaa\xbb")
        self.assertEqual(raw[:4], bytes.fromhex("48fc9f5e"))
        n = struct.unpack(">I", raw[4:8])[0]
        self.assertEqual(raw[8:12], bytes.fromhex("23fc9f5e"))
        self.assertEqual(raw[12:16], b"WXYZ")
        self.assertEqual(raw[8 + n:], b"\xaa\xbb")
        parsed = ble_format.parse_service_data(raw)
        self.assertEqual((parsed["endpoint_id"], parsed["endpoint_info"]), ("WXYZ", info))
        self.assertEqual(endpoint.parse_endpoint_info(parsed["endpoint_info"])["name"], "Desk")

    def test_parses_a_phone_advertisement_with_psm(self):
        info = bytes([0x02]) + bytes(16) + bytes([7]) + b"Pixel 9"
        raw = ble_format.advertisement("PH0N", info, b"\x01\x02", psm=161)
        parsed = ble_format.parse_service_data(raw)
        self.assertEqual(parsed["psm"], 161)
        self.assertEqual(endpoint.parse_endpoint_info(parsed["endpoint_info"])["type"], "phone")

    def test_fits_the_controller(self):
        info = endpoint.endpoint_info("Wanderer’s laptop", endpoint.LAPTOP)  # (a multi-byte character)
        full = ble_format.advertisement("AB12", info, b"\0\1")
        self.assertEqual(ble_format.fitting_advertisement("AB12", info, b"\0\1", 251), full)
        cut = ble_format.fitting_advertisement("AB12", info, b"\0\1", len(full) - 3)
        self.assertLessEqual(len(cut), len(full) - 3)
        name = endpoint.parse_endpoint_info(ble_format.parse_service_data(cut)["endpoint_info"])["name"]
        self.assertTrue("Wanderer’s laptop".startswith(name) and len(name) < len("Wanderer’s laptop"))
        # a legacy-only controller (31 bytes): no extended advertisement at all
        self.assertIsNone(ble_format.fitting_advertisement("AB12", info, b"\0\1", 31 - ble_format.AD_OVERHEAD))

    def test_rejects_what_isnt_quick_share(self):
        for raw in (b"", b"\x48", bytes(17), b"\x48\x01\x02\x03" + struct.pack(">I", 2) + b"zz",
                    ble_format.header([("NearbySharing", b"x")], True)):
            with self.assertRaises(ble_format.AdvertisementError):
                ble_format.parse_service_data(raw)

    def test_fast_advertisement_round_trip(self):
        # fast: version byte with bit 1, one size byte, no service hash
        conn = bytes([0x23]) + b"FAST" + bytes([17]) + bytes(17)
        raw = bytes([0x48 | 0x02, len(conn)]) + conn + b"\x00\x01"
        parsed = ble_format.parse_service_data(raw)
        self.assertTrue(parsed["fast"])
        self.assertEqual(parsed["endpoint_id"], "FAST")

    def test_header_layout_bloom_and_hash_chain(self):
        # mediums/ble/ble_advertisement_header.cc operator ByteArray: version 2
        # << 5 | extended << 4 | slots; bloom (10); hash (4); psm (2).
        # ble.cc CreateAdvertisementHeader (lines 1395-1418): the filter holds a
        # random 128-byte "service id" and each slot's service id; the hash is
        # SHA-256(dummy)[:4], then SHA-256(hash + slot body)[:4] per slot
        body = b"advertisement bytes"
        dummy = bytes(range(128))
        raw = ble_format.header([("NearbySharing", body)], extended=True, psm=0x80, dummy_service_id=dummy)
        self.assertEqual(len(raw), 17)
        self.assertEqual(raw[0], (2 << 5) | 0x10 | 1)
        chain = hashlib.sha256(hashlib.sha256(dummy).digest()[:4] + body).digest()[:4]
        self.assertEqual(raw[11:15], chain)
        self.assertEqual(raw[15:], b"\x00\x80")
        self.assertTrue(ble_format.header_offers(raw))
        self.assertTrue(ble_format.BloomFilter(raw[1:11]).may_contain(dummy))
        h = ble_format.parse_header(raw)
        self.assertEqual((h["extended"], h["slots"], h["psm"]), (True, 1, 0x80))

    def test_murmur3_known_values(self):
        # MurmurHash3_x64_128 reference outputs (seed 0)
        self.assertEqual(ble_format.murmur3_x64_128(b""), (0, 0))
        self.assertEqual(ble_format.murmur3_x64_128(b"hello"), (0xCBD8A7B341BD9B02, 0x5B1E906A48AE1D19))
        h1, h2 = ble_format.murmur3_x64_128(b"The quick brown fox jumps over the lazy dog")
        self.assertEqual(struct.pack("<QQ", h1, h2).hex(), "6c1b07bc7bbc4be347939ac4a93c437a")

    def test_bloom_bit_order_and_positions(self):
        # bloom_filter.cc: hash64 = the low half; hash_i = lo + i * hi (int32),
        # ~x when negative; bit n is bit n % 8 of byte n // 8 (constructor)
        h1, _ = ble_format.murmur3_x64_128(b"NearbySharing")
        lo, hi = h1 & 0xFFFFFFFF, h1 >> 32
        expected = set()
        for i in range(1, 6):
            v = (lo + i * hi) & 0xFFFFFFFF
            v = v - (1 << 32) if v & 0x80000000 else v
            if v < 0:
                v = ~v
            expected.add(v % 80)
        f = ble_format.BloomFilter()
        f.add(b"NearbySharing")
        bits = {n for n in range(80) if f.to_bytes()[n // 8] >> (n % 8) & 1}
        self.assertEqual(bits, expected)

    def test_gatt_uuids(self):
        # mediums/ble/ble_utils.cc GenerateAdvertisementUuid: msb 0x3000,
        # lsb 0x8000000000000000 | slot; the copresence service 0xFEF3
        self.assertEqual(ble_format.SLOT_UUID.format(0), "00000000-0000-3000-8000-000000000000")
        self.assertEqual(ble_format.COPRESENCE_UUID, "0000fef3-0000-1000-8000-00805f9b34fb")


class WeavePackets(unittest.TestCase):
    def test_headers(self):
        # internal/weave/packet.cc: control 0b1000_0000 | command; data first
        # 0b1000, last 0b0100; counter in bits 4-6
        # (the bytes a Pixel exchanged, open-quickshare BLE_RECEIVER_DISCOVERY.md §4.3)
        self.assertEqual(weave.connection_request(509), bytes.fromhex("80 0001 0001 01fd"))
        self.assertEqual(weave.connection_confirm(509), bytes.fromhex("81 0001 01fd"))
        self.assertEqual(weave.with_counter(weave.data_packet(True, True, b"x"), 5), bytes([0x5C]) + b"x")
        p = weave.parse_packet(bytes([0xA2]))
        self.assertEqual((p["control"], p["counter"], p["command"]), (True, 2, weave.ERROR))

    def test_split_message(self):
        parts = weave.split_message(bytes(range(50)), 20)
        self.assertEqual([len(p) for p in parts], [20, 20, 13])
        self.assertEqual(parts[0][0], 0x08)
        self.assertEqual(parts[1][0], 0x00)
        self.assertEqual(parts[2][0], 0x04)
        self.assertEqual(b"".join(p[1:] for p in parts), bytes(range(50)))
        self.assertEqual(weave.split_message(b"", 20), [bytes([0x0C])])

    def test_control_frames_match_ble_frames_proto(self):
        # The INTRODUCTION a phone sends (quickshare-direct DEV_NOTES):
        # 00 00 00 | 08 01 12 07 0a 03 fc9f5e 10 02
        self.assertEqual(weave.control_frame(weave.INTRODUCTION), bytes.fromhex("000000080112070a03fc9f5e1002"))
        self.assertEqual(weave.control_frame(weave.INTRODUCTION), bp.socket_control(1, b"\x10\x02"))
        ack = weave.control_frame(weave.PACKET_ACKNOWLEDGEMENT, received_size=300)
        c = weave.parse_control_frame(ack[3:])
        self.assertEqual((c["type"], c["received_size"], c["service_hash"]), (3, 300, bytes.fromhex("fc9f5e")))
        c = weave.parse_control_frame(weave.control_frame(weave.DISCONNECTION)[3:])
        self.assertEqual(c["type"], weave.DISCONNECTION)


class WeaveLink(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        # The computer as peripheral, the stand-in phone as central
        self.to_phone, self.to_engine, self.phone = bp.pair(engine_central=False, packet_size=40)
        self.sock = weave.WeaveSocket(self.to_phone, is_client=False, max_packet=200)
        self.to_engine.connect(self.sock.feed)
        self.transport = weave.BleSocketTransport(self.sock, is_client=False)

    async def asyncTearDown(self):
        self.to_phone.stop()
        self.to_engine.stop()

    async def test_handshake_fragments_and_counters(self):
        await self.phone.connect()
        self.assertEqual(self.sock.packet_size, 40)  # the smaller of both
        frames = [bytes([i]) * n for i, n in enumerate((1, 39, 40, 41, 300, 5000))]
        for f in frames:
            self.phone.writer.write(struct.pack(">I", len(f)) + f)
        for f in frames:
            self.assertEqual(await self.transport.read_frame(5), f)
        for f in frames:
            await self.transport.write_frame(f)
        for f in frames:
            n = struct.unpack(">I", await self.phone.reader.readexactly(4))[0]
            self.assertEqual(await self.phone.reader.readexactly(n), f)
        self.assertEqual(self.phone.errors, [])  # counters in turn (wrapping past 7), packets ≤ 40
        self.assertTrue(all(s <= 40 for s in self.to_phone.sizes))

    async def test_frames_split_across_messages(self):
        # Older Android writes the length and the frame as two messages
        await self.phone.connect()
        await self.phone.send_message(bp.HASH + struct.pack(">I", 3))
        await self.phone.send_message(bp.HASH + b"abc")
        self.assertEqual(await self.transport.read_frame(5), b"abc")

    async def test_data_out_of_turn_ends_the_link(self):
        await self.phone.connect()
        self.phone.tx = (self.phone.tx + 3) % 8  # a packet "lost"
        await self.phone.send_message(bp.HASH + struct.pack(">I", 1) + b"x")
        with self.assertRaises(TransportClosed):
            await self.transport.read_frame(5)
        await asyncio.sleep(0.05)
        self.assertTrue(self.sock.closed)

    async def test_error_packet_and_disconnection(self):
        await self.phone.connect()
        await self.phone.send_message(weave.control_frame(weave.DISCONNECTION))
        with self.assertRaises(TransportClosed):
            await self.transport.read_frame(5)

    async def test_wrong_versions_are_refused(self):
        async with self.phone._lock:
            await self.phone._packet(0x80, struct.pack(">HHH", 2, 3, 100))
        await asyncio.sleep(0.05)
        self.assertTrue(self.sock.closed)

    async def test_central_side(self):
        to_phone, to_engine, phone = bp.pair(engine_central=True, packet_size=20)
        sock = weave.WeaveSocket(to_phone, is_client=True, max_packet=20)
        to_engine.connect(sock.feed)
        t = weave.BleSocketTransport(sock, is_client=True)
        await sock.connect()
        await t.start()
        await t.write_frame(b"hello")
        n = struct.unpack(">I", await phone.reader.readexactly(4))[0]
        self.assertEqual(await phone.reader.readexactly(n), b"hello")
        self.assertEqual(phone.controls[0], bytes.fromhex("080112070a03fc9f5e1002"))  # the introduction first
        await t.close()
        to_phone.stop()
        to_engine.stop()


class L2cap(unittest.IsolatedAsyncioTestCase):
    async def test_messages_handshake_and_acks(self):
        sent = []

        class Link:
            async def send(self, data):
                sent.append(data)
        m = weave.L2capMessages(Link())
        asking = asyncio.ensure_future(m.request_data_connection(2))
        await asyncio.sleep(0)
        self.assertEqual(sent[0], b"\x00\x00\x00\x01\x03")  # [length][0x03]
        m.feed(b"\x00\x00\x00")
        m.feed(b"\x01\x17")  # ready, in two pieces
        await asyncio.wait_for(asking, 2)
        t = weave.BleSocketTransport(m, is_client=True, acks=True)
        frame = bp.HASH + struct.pack(">I", 2) + b"ok"
        m.feed(struct.pack(">I", len(frame)) + frame)
        self.assertEqual(await t.read_frame(2), b"ok")
        ack = sent[-1][4:]
        self.assertEqual(weave.parse_control_frame(ack[3:])["received_size"], 2)

    async def test_refusal(self):
        class Link:
            async def send(self, data):
                pass
        m = weave.L2capMessages(Link())
        asking = asyncio.ensure_future(m.request_data_connection(2))
        m.feed(b"\x00\x00\x00\x01\x16")
        with self.assertRaises(weave.WeaveError):
            await asking

    def test_sockaddr(self):
        sa = bluez.sockaddr_l2("AA:BB:CC:DD:EE:FF", "random", 0x81)
        self.assertEqual(bytes(sa.bdaddr), bytes.fromhex("ffeeddccbbaa"))
        self.assertEqual(sa.bdaddr_type, bluez.BDADDR_LE_RANDOM)
        self.assertEqual(bytes(sa)[2:4], b"\x81\x00")  # __le16 psm


# ── BlueZ through a fake bus ─────────────────────────────────────────────
class FakeBus:
    """Records calls; answers like BlueZ would; exported objects are kept
    so the test can call into them as BlueZ does."""

    def __init__(self, objects=None, refuse=()):
        self.objects = objects or {}
        self.refuse = set(refuse)
        self.calls = []
        self.exported = {}
        self.emitted = []
        self.subs = []
        self.props = {}

    async def call(self, service, path, iface, method, sig=None, args=(), timeout=0):
        self.calls.append((path, iface, method, args))
        if method in self.refuse:
            raise DBusError("org.bluez.Error.Failed", f"{method} refused")
        if method == "GetManagedObjects":
            return (self.objects,)
        return ()

    call_async = call

    async def get_property(self, service, path, iface, name):
        return self.props.get((path, name))

    async def set_property(self, service, path, iface, name, sig, value):
        self.calls.append((path, iface, "Set", (name, value)))

    def subscribe(self, sender, iface, signal, path, callback, arg0=None):
        h = {"iface": iface, "signal": signal, "path": path, "cb": callback}
        self.subs.append(h)
        return h

    def unsubscribe(self, h):
        if h in self.subs:
            self.subs.remove(h)

    async def export(self, path, interfaces, props, methods):
        self.exported[path] = (interfaces, props, methods)

    async def unexport(self, path):
        self.exported.pop(path, None)

    def emit(self, path, iface, signal, sig, args):
        self.emitted.append((path, signal, args))

    def signal(self, path, iface, signal, args):
        for h in list(self.subs):
            if h["signal"] == signal and (h["path"] in (None, path)):
                h["cb"](path, signal, args)


ADAPTER = {"/org/bluez/hci0": {bluez.ADAPTER: {"Powered": True, "Address": "00:11:22:33:44:55"}}}


class BlueZ(unittest.IsolatedAsyncioTestCase):
    async def test_advertises_both_ways_and_serves_gatt(self):
        bus = FakeBus(dict(ADAPTER))
        ble = bluez.Bluetooth(bus)
        self.assertTrue(await ble.start())
        adv = ble_format.advertisement("AB12", endpoint.endpoint_info("Desk"), b"\x00\x01")
        self.assertTrue(await ble.advertise(adv))
        methods = [c[2] for c in bus.calls]
        self.assertIn("RegisterApplication", methods)
        self.assertEqual(methods.count("RegisterAdvertisement"), 2)
        ext = bus.exported[bluez.ADV_EXTENDED][1](bluez.ADV)
        self.assertEqual(ext["ServiceData"][1][ble_format.COPRESENCE_UUID][1], adv)
        self.assertEqual(ext["Type"], ("s", "peripheral"))
        # discoverable and often, like a phone's own (a Pixel's sheet missed a slow one)
        self.assertEqual(ext["Discoverable"], ("b", True))
        self.assertEqual((ext["MinInterval"], ext["MaxInterval"]), (("u", 100), ("u", 150)))
        legacy = bus.exported[bluez.ADV_LEGACY][1](bluez.ADV)
        self.assertEqual(legacy["Discoverable"], ("b", True))
        header = legacy["ServiceData"][1][ble_format.COPRESENCE_UUID][1]
        self.assertTrue(ble_format.is_header(header) and header[0] & 0x10)  # says the extended one is out
        # slot 0 serves the full advertisement
        (value,) = await bus.exported[bluez.SLOT_PATH][2][(bluez.GATT_CHAR, "ReadValue")](({"offset": 0},), ":1.5")
        self.assertEqual(value, adv)
        (tree,) = await bus.exported[bluez.ROOT][2][(bluez.OM, "GetManagedObjects")]((), ":1.5")
        uuids = {i[bluez.GATT_CHAR]["UUID"][1] for p, i in tree.items() if bluez.GATT_CHAR in i}
        self.assertEqual(uuids, {ble_format.SLOT_UUID.format(0), ble_format.WEAVE_WRITE_UUID, ble_format.WEAVE_NOTIFY_UUID})
        await ble.unserve()
        self.assertNotIn(bluez.ROOT, bus.exported)
        self.assertIn("UnregisterApplication", [c[2] for c in bus.calls])

    async def test_extended_refused_falls_back_to_the_header(self):
        bus = FakeBus(dict(ADAPTER))

        async def call(service, path, iface, method, sig=None, args=(), timeout=0):
            bus.calls.append((path, iface, method, args))
            if method == "RegisterAdvertisement" and args[0] == bluez.ADV_EXTENDED:
                raise DBusError("org.bluez.Error.InvalidLength", "too long")
            return (bus.objects,) if method == "GetManagedObjects" else ()
        bus.call = call
        ble = bluez.Bluetooth(bus)
        await ble.start()
        self.assertTrue(await ble.advertise(ble_format.advertisement("AB12", endpoint.endpoint_info("D"), b"\0\0")))
        self.assertEqual(ble.advertising, [bluez.ADV_LEGACY])
        header = bus.exported[bluez.ADV_LEGACY][1](bluez.ADV)["ServiceData"][1][ble_format.COPRESENCE_UUID][1]
        self.assertFalse(header[0] & 0x10)

    async def test_powered_off_does_nothing_and_never_powers_on(self):
        bus = FakeBus({"/org/bluez/hci0": {bluez.ADAPTER: {"Powered": False}}})
        states = []
        ble = bluez.Bluetooth(bus, on_state=states.append)
        await ble.start()
        self.assertEqual(ble.state(), {"available": True, "powered": False})
        self.assertFalse(await ble.advertise(b"x"))
        self.assertFalse(await ble.scan(lambda *a: None, lambda *a: None))
        self.assertFalse(any(c[2] == "Set" for c in bus.calls))  # nothing powered on by itself
        await ble.power_on()  # (only the user's button calls this)
        self.assertIn(("/org/bluez/hci0", bluez.ADAPTER, "Set", ("Powered", True)), bus.calls)
        bus.signal("/org/bluez/hci0", bluez.PROPS, "PropertiesChanged", (bluez.ADAPTER, {"Powered": True}, []))
        self.assertEqual(states[-1], {"available": True, "powered": True})

    async def test_a_phone_connects_over_gatt(self):
        bus = FakeBus(dict(ADAPTER))
        sessions = []
        ble = bluez.Bluetooth(bus, on_session=sessions.append)
        await ble.start()
        await ble.advertise(ble_format.advertisement("AB12", endpoint.endpoint_info("D"), b"\0\0"))
        start = bus.exported[bluez.NOTIFY_PATH][2][(bluez.GATT_CHAR, "StartNotify")]
        write = bus.exported[bluez.WRITE_PATH][2][(bluez.GATT_CHAR, "WriteValue")]
        await start((), ":1.9")
        dev = "/org/bluez/hci0/dev_AA"
        await write((list(weave.connection_request(509)), {"device": dev, "mtu": 247}), ":1.9")
        self.assertEqual(len(sessions), 1)
        # the confirm went out as a notification: our packet size is MTU - 3
        path, signal, args = bus.emitted[-1]
        self.assertEqual((path, signal), (bluez.NOTIFY_PATH, "PropertiesChanged"))
        confirm = args[1]["Value"][1]
        self.assertEqual(confirm, weave.with_counter(weave.connection_confirm(244), 0))
        # another phone meanwhile: busy
        with self.assertRaises(DBusError):
            await write((list(weave.with_counter(weave.data_packet(True, True, b"x"), 0)),
                         {"device": "/org/bluez/hci0/dev_BB"}), ":1.10")
        # the phone goes: its session ends
        bus.signal(dev, bluez.PROPS, "PropertiesChanged", (bluez.DEVICE, {"Connected": False}, []))
        with self.assertRaises(TransportClosed):
            await sessions[0].read_frame(1)

    async def test_a_new_request_starts_afresh(self):
        # (a phone that gave up on a link connects again; the old one is dead)
        bus = FakeBus(dict(ADAPTER))
        sessions, ended = [], []
        ble = bluez.Bluetooth(bus, on_session=sessions.append)
        ble.on_session_end = lambda: ended.append(True)
        await ble.start()
        await ble.advertise(ble_format.advertisement("AB12", endpoint.endpoint_info("D"), b"\0\0"))
        await bus.exported[bluez.NOTIFY_PATH][2][(bluez.GATT_CHAR, "StartNotify")]((), ":1.9")
        write = bus.exported[bluez.WRITE_PATH][2][(bluez.GATT_CHAR, "WriteValue")]
        for dev in ("/org/bluez/hci0/dev_AA", "/org/bluez/hci0/dev_BB"):
            await write((list(weave.connection_request(509)), {"device": dev, "mtu": 247}), ":1.9")
        self.assertEqual(len(sessions), 2)
        self.assertEqual(ended, [True])
        self.assertEqual(ble.session[0], "/org/bluez/hci0/dev_BB")
        with self.assertRaises(TransportClosed):
            await sessions[0].read_frame(1)

    async def test_stray_packets_outside_a_connection_are_left_alone(self):
        # (seen on a real phone: one more control packet after the transfer moved to Wi-Fi)
        bus = FakeBus(dict(ADAPTER))
        sessions = []
        ble = bluez.Bluetooth(bus, on_session=sessions.append)
        await ble.start()
        await ble.advertise(ble_format.advertisement("AB12", endpoint.endpoint_info("D"), b"\0\0"))
        write = bus.exported[bluez.WRITE_PATH][2][(bluez.GATT_CHAR, "WriteValue")]
        await write(([0xE2], {"device": "/org/bluez/hci0/dev_AA"}), ":1.9")
        await write((list(weave.with_counter(weave.data_packet(True, True, b"x"), 3)), {"device": "/d"}), ":1.9")
        self.assertEqual((sessions, ble.session), ([], None))

    async def gatt(self, bus):
        ble = bluez.Bluetooth(bus)
        await ble.start()
        await ble.advertise(ble_format.advertisement("AB12", endpoint.endpoint_info("D"), b"\0\0"))
        m = lambda path, name: bus.exported[path][2][(bluez.GATT_CHAR, name)]  # noqa: E731
        return ble, m(bluez.WRITE_PATH, "WriteValue"), m(bluez.NOTIFY_PATH, "StartNotify")

    async def test_a_write_before_subscribing_never_blocks(self):
        # (a real phone dropped every attempt 2 s in: its first write waited
        # for a subscription BlueZ could only deliver after the write returned)
        bus = FakeBus(dict(ADAPTER))
        ble, write, start = await self.gatt(bus)
        flags = bus.exported[bluez.NOTIFY_PATH][1](bluez.GATT_CHAR)["Flags"][1]
        self.assertIn("indicate", flags)  # as Nearby's own peripherals
        dev = "/org/bluez/hci0/dev_AA"
        await asyncio.wait_for(write((list(weave.connection_request(0)), {"device": dev, "mtu": 517}), ":1.9"), 0.2)
        self.assertEqual([e for e in bus.emitted if e[0] == bluez.NOTIFY_PATH], [])  # held until it subscribes
        await start((), ":1.9")
        confirm = [e for e in bus.emitted if e[0] == bluez.NOTIFY_PATH][0][2][1]["Value"][1]
        self.assertEqual(confirm, weave.with_counter(weave.connection_confirm(512), 0))  # (0: we choose)

    async def test_a_phone_that_never_subscribes_is_let_go(self):
        bus = FakeBus(dict(ADAPTER))
        ble, write, start = await self.gatt(bus)
        old, bluez.SUBSCRIBE_WAIT = bluez.SUBSCRIBE_WAIT, 0.05
        try:
            await write((list(weave.connection_request(100)), {"device": "/d", "mtu": 247}), ":1.9")
            await asyncio.sleep(0.15)
        finally:
            bluez.SUBSCRIBE_WAIT = old
        self.assertIsNone(ble.session)
        self.assertEqual(ble.pending, [])

    async def test_packet_log_shows_control_bytes_only(self):
        from nearby import debuglog
        import os
        import tempfile
        bus = FakeBus(dict(ADAPTER))
        ble, write, start = await self.gatt(bus)
        with tempfile.TemporaryDirectory() as d:
            debuglog._path = os.path.join(d, "log")
            try:
                await start((), ":1.9")
                await write((list(weave.connection_request(185)), {"device": "/d", "mtu": 188}), ":1.9")
                await write((list(weave.with_counter(weave.data_packet(True, False, b"SECRETDATA"), 1)),
                             {"device": "/d"}), ":1.9")
                bus.signal("/d", bluez.PROPS, "PropertiesChanged", (bluez.DEVICE, {"Paired": True}, []))
                text = open(debuglog._path).read()
            finally:
                debuglog._path = None
        self.assertIn("← write (request, MTU 188) 0101: 7 bytes, control 80000100010" + "0b9", text)
        self.assertIn("→ indicate 0102: 5 bytes, control 81000100b9", text)
        self.assertIn("data #1 first", text)
        self.assertNotIn("SECRET", text)
        self.assertNotIn(b"SECRETDATA".hex(), text)
        self.assertIn("device d: {'Paired': True}", text)

    async def test_service_fits_the_advertisement_to_the_controller(self):
        from nearby.service import QuickShareService
        for longest, expect in ((31, [bluez.ADV_LEGACY]), (251, [bluez.ADV_EXTENDED, bluez.ADV_LEGACY])):
            bus = FakeBus(dict(ADAPTER))
            bus.props[("/org/bluez/hci0", "SupportedCapabilities")] = {"MaxAdvLen": longest}
            ble = bluez.Bluetooth(bus)
            await ble.start()
            svc = QuickShareService(lambda ev: None, None, name="Desk", ble=ble)
            svc.preferred_port = 0
            await svc.configure(visible=True)
            self.assertEqual([c[3][0] for c in bus.calls if c[2] == "RegisterAdvertisement"], expect)
            # slot 0 keeps the full advertisement either way
            self.assertEqual(ble_format.parse_service_data(ble.slot_value)["endpoint_id"], svc.endpoint_id)
            await svc.close()

    async def test_scan_is_le_only_and_finds_phones(self):
        bus = FakeBus(dict(ADAPTER))
        found, lost = [], []
        ble = bluez.Bluetooth(bus)
        await ble.start()
        self.assertTrue(await ble.scan(lambda k, a: found.append((k, a)), lost.append))
        flt = next(c for c in bus.calls if c[2] == "SetDiscoveryFilter")[3][0]
        self.assertEqual(flt["Transport"], ("s", "le"))
        # (no UUID filter: BlueZ would match it against service UUID lists, and
        # phones carry 0xFEF3 only as service data — a filtered scan finds none)
        self.assertNotIn("UUIDs", flt)
        info = bytes([0x02]) + bytes(16) + bytes([5]) + b"Pixel"
        adv = ble_format.advertisement("PH0N", info, b"\x01\x02", psm=128)
        dev = "/org/bluez/hci0/dev_C0"
        bus.props[(dev, "Address")] = "C0:00:00:00:00:01"
        bus.props[(dev, "AddressType")] = "random"
        bus.signal("/", bluez.OM, "InterfacesAdded",
                   (dev, {bluez.DEVICE: {"ServiceData": {ble_format.COPRESENCE_UUID: list(adv)}, "RSSI": -50}}))
        await asyncio.sleep(0.01)
        self.assertEqual(found[0][0], dev)
        self.assertEqual((found[0][1]["endpoint_id"], found[0][1]["psm"]), ("PH0N", 128))
        # nearby phones are told someone is sharing (they then show themselves)
        wake = next(c for c in bus.calls if c[2] == "RegisterAdvertisement" and c[3][0] == bluez.ADV_WAKE)
        self.assertTrue(wake)
        props = bluez.wake_properties()
        self.assertEqual(props["Type"], ("s", "broadcast"))
        self.assertEqual(props["ServiceData"][1][bluez.WAKE_UUID][1][:5], bytes([0xFC, 0x12, 0x8E, 0x01, 0x42]))
        await ble.stop_scan()
        self.assertEqual(lost, [dev])
        self.assertIn("StopDiscovery", [c[2] for c in bus.calls])
        self.assertIn(("UnregisterAdvertisement", bluez.ADV_WAKE),
                      [(c[2], c[3][0]) for c in bus.calls if c[2] == "UnregisterAdvertisement"])


# ── The root Wi-Fi helper's pure parts ───────────────────────────────────
IW_PHY = """
Wiphy phy0
	Band 1:
		Frequencies:
			* 2412.0 MHz [1] (22.0 dBm)
			* 2467.0 MHz [12] (22.0 dBm) (no IR)
	Band 2:
		Frequencies:
			* 5180.0 MHz [36] (22.0 dBm)
			* 5260 MHz [52] (22.0 dBm) (no IR, radar detection)
			* 5500 MHz [100] (22.0 dBm) (no IR, radar detection)
			* 5745 MHz [149] (22.0 dBm)
			* 5865 MHz [173] (disabled)
"""


class WifiHelper(unittest.TestCase):
    def test_channel_choice(self):
        chans = helper.parse_iw_phy(IW_PHY)
        self.assertEqual(chans, {2412: "ok", 2467: "no-ir", 5180: "ok", 5260: "radar", 5500: "radar", 5745: "ok",
                                 5865: "disabled"})
        # next to a connected station: its channel, even "no IR" (the connection lifts it)
        self.assertEqual(helper.go_freq(5745, 5180, chans), 5745)
        self.assertEqual(helper.go_freq(2467, 0, chans), 2467)
        self.assertEqual(helper.go_freq(5500, 0, chans), 5180)  # radar: a free channel instead
        self.assertEqual(helper.go_freq(0, 5745, chans), 5745)  # alone: the hint when it's free
        self.assertEqual(helper.go_freq(0, 2467, chans), 5180)
        self.assertEqual(helper.go_freq(0, 0, {}), 0)  # nothing known: wpa_supplicant picks
        # a hotspot shares the station's channel; refused up front (saying
        # why) where it can't start: radar, "no IR", 5 GHz on Intel
        self.assertEqual(helper.ap_freq(5745, 5180, chans), 5745)
        self.assertEqual(helper.ap_freq(2412, 0, chans, "iwlwifi"), 2412)
        self.assertEqual(helper.ap_freq(0, 0, chans), 5180)
        for station, driver, why in ((5500, "", "5500 MHz needs radar"), (2467, "", "2467 MHz: the rules"),
                                     (5180, "iwlwifi", "Intel Wi-Fi can't run a hotspot on 5 GHz")):
            with self.assertRaisesRegex(helper.Refused, why):
                helper.ap_freq(station, 0, chans, driver)

    def test_requests_are_checked(self):
        for bad in ("", "x" * 33, "a\nb", None, 5):
            with self.assertRaises(helper.Refused):
                helper.check_ssid(bad)
        for bad in ("short", "x" * 64, "pässword1", "tab\there1"):
            with self.assertRaises(helper.Refused):
                helper.check_passphrase(bad)
        for bad in (1, 3000, "5180", 10000):
            with self.assertRaises(helper.Refused):
                helper.check_freq(bad)
        self.assertEqual(helper.check_freq(5180), 5180)
        with self.assertRaises(helper.Refused):
            helper.check_ifname("wlan0; rm -rf /")

    def test_dbus_arguments(self):
        # Only the keys wpa_supplicant's GroupAdd handler takes (anything else:
        # InvalidArgs, as a real laptop answered to ht40/vht)
        self.assertEqual(helper.group_add_args(5180), {"persistent": ("b", False), "frequency": ("i", 5180)})
        self.assertEqual(helper.group_add_args(0), {"persistent": ("b", False)})
        ap = helper.ap_network("DIRECT-iR-ABCD", "secretpass", 2412)
        self.assertEqual((ap["mode"], ap["frequency"], ap["key_mgmt"]), (("u", 2), ("i", 2412), ("s", "WPA-PSK")))
        j = helper.join_network("DIRECT-xy", "secretpass", 5220)
        self.assertEqual((j["ssid"], j["scan_freq"]), (("s", "DIRECT-xy"), ("s", "5220")))
        self.assertNotIn("scan_freq", helper.join_network("DIRECT-xy", "secretpass", 0))
        mac = helper.random_mac()
        self.assertEqual(int(mac.split(":")[0], 16) & 3, 2)  # locally administered, unicast

    def test_dhcp_packets(self):
        mac = bytes.fromhex("020000000001")
        pkt = helper.dhcp_packet(1, 0x1234, mac, options=[(53, b"\x01")])
        p = helper.parse_dhcp(pkt)
        self.assertEqual((p["op"], p["xid"], p["type"], p["flags"], bytes(p["chaddr"])), (1, 0x1234, 1, 0x8000, mac))
        offer = helper.parse_dhcp(helper.dhcp_packet(2, 7, mac, yiaddr="192.168.49.10",
                                                     options=[(53, b"\x02"), (1, bytes([255, 255, 255, 0]))]))
        self.assertEqual((offer["yiaddr"], offer["type"]), ("192.168.49.10", 2))
        self.assertEqual(helper.prefix_len("255.255.255.0"), 24)
        self.assertIsNone(helper.parse_dhcp(b"short"))


class FakeWpa:
    """wpa_supplicant as start_group sees it: GroupAdd makes `script` happen
    (signals delivered while pump() runs, new interfaces appearing)."""

    def __init__(self, script):
        self.script = script
        self.subs = {}
        self.interfaces = ["/w/1"]
        self.groups = {}
        self.queue = []
        self.calls = []

    def subscribe(self, iface, signal, cb):
        self.subs[signal] = cb
        return signal

    def unsubscribe(self, sub):
        self.subs.pop(sub, None)

    def call(self, path, iface, method, sig=None, args=()):
        self.calls.append((path, method, args))
        if method == "GroupAdd" and "frequency" in args[0] and "freq-refused" in self.script:
            raise helper.Refused("wpa_supplicant: Did not receive correct message arguments")
        if method == "GroupAdd" and "all-refused" in self.script:
            raise helper.Refused("wpa_supplicant: Did not receive correct message arguments")
        if method == "GroupAdd":
            self.queue = list(self.script)
        return ()

    def get(self, path, iface, prop):
        if prop == "Interfaces":
            return list(self.interfaces)
        if prop == "Group":
            return self.groups.get(path, "/")
        raise helper.Refused("no such property")

    def pump(self, seconds):
        while self.queue:
            step = self.queue.pop(0)
            if step in ("freq-refused", "all-refused"):
                continue
            if step == "appear":
                self.interfaces.append("/w/2")
                self.groups["/w/2"] = "/w/2/Groups/YU"
            else:
                name, args = step
                if name in self.subs:
                    self.subs[name]("/w/1", args)  # (on the P2P device's object, not the group's)


class GroupStart(unittest.TestCase):
    STARTED = ("GroupStarted", ({"interface_object": "/w/2", "group_object": "/w/2/Groups/YU", "role": "GO"},))

    def test_group_started_signal(self):
        wpa = FakeWpa([self.STARTED])
        self.assertEqual(helper.start_group(wpa, "/w/1", 5745, timeout=1, settle=0), ("/w/2", "/w/2/Groups/YU"))
        add = [c for c in wpa.calls if c[1] == "GroupAdd"][0]
        self.assertEqual(add[2][0]["frequency"], ("i", 5745))
        self.assertEqual(wpa.subs, {})  # unsubscribed again

    def test_retries_on_a_channel_of_its_choosing(self):
        wpa = FakeWpa(["freq-refused", self.STARTED])
        helper.start_group(wpa, "/w/1", 5180, timeout=1, settle=0)
        adds = [c[2][0] for c in wpa.calls if c[1] == "GroupAdd"]
        self.assertEqual(adds, [{"persistent": ("b", False), "frequency": ("i", 5180)}, {"persistent": ("b", False)}])
        with self.assertRaisesRegex(helper.Refused, "on 5180 MHz or on a channel of its own choosing"):
            helper.start_group(FakeWpa(["all-refused"]), "/w/1", 5180, timeout=1, settle=0)

    def test_groupadd_gvariant_types(self):
        try:
            import gi
            gi.require_version("GLib", "2.0")
            from gi.repository import GLib
        except (ImportError, ValueError):
            self.skipTest("no python-gobject")
        v = helper.dbus_params(GLib, "a{sv}", (helper.group_add_args(5180),))
        self.assertEqual(v.get_type_string(), "(a{sv})")
        d = v.get_child_value(0)
        self.assertEqual(d.lookup_value("persistent", None).get_type_string(), "b")
        self.assertEqual(d.lookup_value("frequency", None).get_type_string(), "i")
        self.assertEqual(d.n_children(), 2)

    def test_a_group_that_ends_as_it_starts_is_reported_at_once(self):
        # (what a real laptop did: P2P-GROUP-STARTED, then AP-DISABLED)
        import time
        wpa = FakeWpa([self.STARTED, ("GroupFinished", ({"interface_object": "/w/2", "role": "GO"},))])
        t = time.monotonic()
        with self.assertRaisesRegex(helper.Refused, "stopped right after starting"):
            helper.start_group(wpa, "/w/1", 0, timeout=10, settle=0.05)
        self.assertLess(time.monotonic() - t, 2)

    def test_formation_failure_and_finished_end_the_wait(self):
        for event, why in ((("GroupFormationFailure", ("no channel",)), "didn't form"),
                           (("GroupFinished", ({},)), "ended as it started")):
            with self.assertRaisesRegex(helper.Refused, why):
                helper.start_group(FakeWpa([event]), "/w/1", 0, timeout=10, settle=0)

    def test_polling_finds_a_group_without_its_signal(self):
        self.assertEqual(helper.start_group(FakeWpa(["appear"]), "/w/1", 0, timeout=1, settle=0),
                         ("/w/2", "/w/2/Groups/YU"))

    def test_group_that_dies_at_once_falls_back_to_2_4_ghz(self):
        # (Intel next to a 5 GHz connection: the group ends 2 ms after it
        # starts, so its objects are gone when we read them)
        tried = []

        def start(wpa, dev, freq, anywhere=True, **_):
            tried.append((freq, anywhere))
            return (f"/w/{freq}", f"/w/{freq}/Groups/X")

        class Wpa:
            calls = []

            def get(self, path, iface, prop):
                if path.startswith("/w/5180"):
                    raise helper.Refused("Method \"Get\" doesn't exist")
                if path == helper.WPA_PATH:
                    return "info"
                return {"Ifname": "p2p-wlan0-1", "SSID": b"DIRECT-ab", "Passphrase": "pw", "Frequency": 2437}[prop]

            def set(self, *args):
                self.calls.append(("set",) + args[2:])

            def interface(self, name):
                return "/w/1"

            def call(self, *args):
                self.calls.append(args)

        old = (helper.start_group, helper.run, helper.have)
        helper.start_group, helper.run, helper.have = start, (lambda *cmd, check=True: ""), (lambda cmd: False)
        try:
            links = helper.Links()
            links.wpa = Wpa()
            links.station = lambda: "wlan0"
            links.station_freq = lambda sta: 5180
            links.usable = lambda: {5180: "no-ir", 2437: "ok", 2412: "ok", 2462: "ok"}
            links._netdevs = lambda: set()
            links._nm_off = lambda sta: None
            links._address = links._dhcp_server = lambda *a: None
            r = links.host_p2p(0)
        finally:
            helper.start_group, helper.run, helper.have = old
        self.assertEqual(tried, [(5180, True), (2437, False)])
        self.assertEqual((r["ssid"], r["freq"], r["iface"]), ("DIRECT-ab", 2437, "p2p-wlan0-1"))
        # wpa_supplicant's detailed log is on while the link lasts, and back after
        self.assertIn(("set", "DebugLevel", "s", "debug"), links.wpa.calls)
        links.teardown()
        self.assertEqual(links.wpa.calls[-1], ("set", "DebugLevel", "s", "info"))

    def test_networkmanager_kept_off_and_restored(self):
        import os
        import tempfile
        ran = []
        old = (helper.NM_CONF_DIR, helper.run, helper.have)
        with tempfile.TemporaryDirectory() as d:
            helper.NM_CONF_DIR = os.path.join(d, "conf.d")
            helper.run = lambda *cmd, check=True: ran.append(cmd) or ""
            helper.have = lambda cmd: True
            try:
                links = helper.Links()
                links._nm_off("wlan0")
                links._nm_off("wlan0")  # (once per set of links)
                path = os.path.join(helper.NM_CONF_DIR, helper.NM_CONF)
                text = open(path).read()
                self.assertIn("unmanaged-devices+=interface-name:p2p-dev-wlan0;interface-name:p2p-wlan0-*;"
                              "interface-name:p2p-grp-*;interface-name:qsc0;interface-name:qsap0", text)
                self.assertEqual(ran, [("nmcli", "general", "reload", "conf")])
                links.teardown()
                self.assertFalse(os.path.exists(path))
                self.assertEqual(ran[-1], ("nmcli", "general", "reload", "conf"))
                self.assertEqual(len(ran), 2)
            finally:
                helper.NM_CONF_DIR, helper.run, helper.have = old

    def test_leftover_group_interfaces_are_removed(self):
        ran = []
        old = helper.run
        helper.run = lambda *cmd, check=True: ran.append(cmd) or ""
        try:
            links = helper.Links()
            links._netdevs = lambda: {"lo", "wlan0", "p2p-dev-wlan0", "p2p-wlan0-0"}
            links._remove_new_groups({"lo", "wlan0", "p2p-dev-wlan0"})
        finally:
            helper.run = old
        self.assertEqual(ran, [("iw", "dev", "p2p-wlan0-0", "del")])
        self.assertEqual(helper.group_interfaces({"p2p-dev-wlan0", "p2p-grp-3", "qsc0"}), {"p2p-grp-3"})


class WifiPaths(unittest.TestCase):
    def test_network_parsing(self):
        ip = ('[{"ifname":"lo","addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8}]},'
              '{"ifname":"wlan0","addr_info":[{"family":"inet","local":"192.168.1.20","prefixlen":24}]}]')
        addrs = wifi.parse_ip_addr(ip)
        self.assertEqual([a[0] for a in addrs], ["wlan0"])
        self.assertEqual(wifi.same_network(addrs, bytes([192, 168, 1, 77])), bytes([192, 168, 1, 20]))
        self.assertIsNone(wifi.same_network(addrs, bytes([10, 0, 0, 2])))
        self.assertEqual(wifi.parse_iw_link("Connected to aa\n\tSSID: x\n\tfreq: 5220.0\n"), 5220)
        self.assertEqual(wifi.parse_iw_list("Band 2:\n * P2P-GO\n"), {"5ghz": True, "p2p_go": True})
        m = wifi.medium_metadata({"addresses": addrs, "station": "wlan0", "frequency": 5220, "5ghz": True})
        self.assertEqual((m.supports_5_ghz, m.ap_frequency, bytes(m.ip_address)), (True, 5220, bytes([192, 168, 1, 20])))


class PrivateBus(unittest.IsolatedAsyncioTestCase):
    """gbus.GioBus for real (GLib variants, exported objects, signals), on a
    private dbus-daemon of the test's own: never the system or session bus."""

    async def asyncSetUp(self):
        import shutil
        if not shutil.which("dbus-daemon"):
            self.skipTest("no dbus-daemon")
        try:
            from nearby.gbus import GioBus
        except ImportError:
            self.skipTest("no python-gobject")
        self.daemon = await asyncio.create_subprocess_exec(
            "dbus-daemon", "--session", "--nofork", "--nopidfile", "--print-address=1",
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        address = (await asyncio.wait_for(self.daemon.stdout.readline(), 5)).decode().strip()
        loop = asyncio.get_running_loop()
        self.server = GioBus(loop, address)
        self.client = GioBus(loop, address)

    async def asyncTearDown(self):
        try:
            await self.server.close()
            await self.client.close()
        finally:
            self.daemon.kill()
            await self.daemon.wait()

    async def test_export_call_property_signal(self):
        got = []

        async def write(args, sender):
            value, options = args
            got.append((bytes(value), options))
            return ()

        async def managed(args, sender):
            return (bluez.gatt_objects(b"slot"),)
        await self.server.export(bluez.WRITE_PATH, {bluez.GATT_CHAR: bluez.CHAR_IFACE},
                                 lambda iface: {"UUID": ("s", ble_format.WEAVE_WRITE_UUID), "Flags": ("as", ["write"])},
                                 {(bluez.GATT_CHAR, "WriteValue"): write})
        await self.server.export(bluez.ROOT, {bluez.OM: bluez.OM_IFACE}, lambda iface: {},
                                 {(bluez.OM, "GetManagedObjects"): managed})
        name = self.server.conn.get_unique_name()
        await self.client.call(name, bluez.WRITE_PATH, bluez.GATT_CHAR, "WriteValue", "aya{sv}",
                               (b"\x80\x00\x01", {"device": ("o", "/org/bluez/hci0/dev_X"), "mtu": ("q", 517)}))
        self.assertEqual(got, [(b"\x80\x00\x01", {"device": "/org/bluez/hci0/dev_X", "mtu": 517})])
        uuid = await self.client.get_property(name, bluez.WRITE_PATH, bluez.GATT_CHAR, "UUID")
        self.assertEqual(uuid, ble_format.WEAVE_WRITE_UUID)
        (tree,) = await self.client.call(name, bluez.ROOT, bluez.OM, "GetManagedObjects")
        self.assertEqual(tree[bluez.SERVICE_PATH][bluez.GATT_SERVICE]["UUID"], ble_format.COPRESENCE_UUID)
        with self.assertRaises(DBusError):
            await self.client.call(name, bluez.WRITE_PATH, bluez.GATT_CHAR, "ReadValue", "a{sv}", ({},))
        signals = asyncio.Queue()
        self.client.subscribe(None, bluez.PROPS, "PropertiesChanged", bluez.NOTIFY_PATH,
                              lambda path, sig, args: signals.put_nowait(args))
        await asyncio.sleep(0.1)
        self.server.emit(bluez.NOTIFY_PATH, bluez.PROPS, "PropertiesChanged", "sa{sv}as",
                         (bluez.GATT_CHAR, {"Value": ("ay", b"\x81\x00")}, []))
        iface, changed, _ = await asyncio.wait_for(signals.get(), 5)
        self.assertEqual((iface, bytes(changed["Value"])), (bluez.GATT_CHAR, b"\x81\x00"))


    async def test_a_call_answered_by_calling_back(self):
        # BlueZ reads our exported objects while it handles RegisterApplication:
        # the caller's GLib thread must stay free to answer
        async def managed(args, sender):
            return (bluez.gatt_objects(b"x"),)
        await self.client.export(bluez.ROOT, {bluez.OM: bluez.OM_IFACE}, lambda iface: {},
                                 {(bluez.OM, "GetManagedObjects"): managed})
        seen = []

        async def register(args, sender):
            (tree,) = await self.server.call(sender, args[0], bluez.OM, "GetManagedObjects")
            seen.append(tree)
            return ()
        iface = {"methods": {"RegisterApplication": ("oa{sv}", "")}}
        await self.server.export("/org/bluez/hci0", {bluez.GATT_MANAGER: iface}, lambda i: {},
                                 {(bluez.GATT_MANAGER, "RegisterApplication"): register})
        await asyncio.wait_for(self.client.call(self.server.conn.get_unique_name(), "/org/bluez/hci0",
                                                bluez.GATT_MANAGER, "RegisterApplication", "oa{sv}",
                                                (bluez.ROOT, {})), 5)
        self.assertIn(bluez.SERVICE_PATH, seen[0])


class GBus(unittest.TestCase):
    def test_signatures_and_introspection(self):
        self.assertEqual(_split("aya{sv}"), ["ay", "a{sv}"])
        self.assertEqual(_split("oa{sa{sv}}as"), ["o", "a{sa{sv}}", "as"])
        xml = _intro_xml({bluez.GATT_CHAR: bluez.CHAR_IFACE})
        self.assertIn('<method name="WriteValue"><arg name="a0" type="ay" direction="in"/>', xml)
        self.assertIn('<property name="Value" type="ay" access="read"/>', xml)


if __name__ == "__main__":
    unittest.main()
