"""A stand-in phone's Bluetooth LE side and bandwidth upgrade, for the tests.

Like quickshare_peer.py, written from the protocol's side (google/nearby
internal/weave/packet.cc and base_socket.cc, mediums/ble/ble_packet.cc,
proto/mediums/ble_frames.proto, connections/implementation/bwu_manager.cc)
rather than through the engine's weave.py and connection.py, so the two
check each other. The phone's end of a weave link gives quickshare_peer.Peer
the reader/writer it expects, so the same Peer runs over Bluetooth.
"""
import asyncio
import struct

import quickshare_peer as qp

HASH = bytes.fromhex("fc9f5e")
BWU = qp.offline.BandwidthUpgradeNegotiationFrame


class Link:
    """One direction of an in-memory GATT link: packets queue up and are
    fed to the other side in order, as BlueZ would deliver them."""

    def __init__(self):
        self.queue = asyncio.Queue()
        self.sizes = []
        self.target = None
        self.pump = None

    async def send(self, packet):
        self.sizes.append(len(packet))
        self.queue.put_nowait(bytes(packet))

    def connect(self, feed):
        async def run():
            while True:
                p = await self.queue.get()
                await feed(p)
        self.pump = asyncio.ensure_future(run())

    def stop(self):
        if self.pump:
            self.pump.cancel()


def proto_varint(n):
    out = b""
    while True:
        b, n = n & 0x7F, n >> 7
        out += bytes([b | (0x80 if n else 0)])
        if not n:
            return out


def socket_control(kind, extra=b""):
    """[00 00 00][SocketControlFrame{type, <frame>{service_id_hash, …}}]."""
    inner = b"\x0a\x03" + HASH + extra
    field = {1: 0x12, 2: 0x1a, 3: 0x22}[kind]
    return b"\x00\x00\x00" + b"\x08" + bytes([kind]) + bytes([field, len(inner)]) + inner


class PhoneWeave:
    """The phone's end of a weave link (central when the phone sends, the
    peripheral when it receives). reader/writer: the [length][frame] stream
    for quickshare_peer.Peer."""

    def __init__(self, out_link, central, packet_size=20):
        self.out = out_link
        self.central = central
        self.wanted = packet_size
        self.packet_size = packet_size
        self.tx = 0
        self.rx = 0
        self.parts = None
        self.connected = asyncio.Event()
        self.reader = asyncio.StreamReader()
        self.controls = []  # control frames the computer sent (raw)
        self.errors = []
        phone = self

        class Writer:
            def write(self, data):
                asyncio.ensure_future(phone.send_message(HASH + bytes(data)))

            async def drain(self):
                await asyncio.sleep(0)

            def close(self):
                pass
        self.writer = Writer()
        self._lock = asyncio.Lock()

    async def _packet(self, header, payload):
        h = header | (self.tx << 4)
        self.tx = (self.tx + 1) % 8
        await self.out.send(bytes([h]) + payload)

    async def connect(self):
        """Central: ConnectionRequest(1, 1, size), then the INTRODUCTION."""
        async with self._lock:
            await self._packet(0x80, struct.pack(">HHH", 1, 1, self.wanted))
        await asyncio.wait_for(self.connected.wait(), 5)
        await self.send_message(socket_control(1, b"\x10\x02"))

    async def send_message(self, msg):
        async with self._lock:
            room = self.packet_size - 1
            parts = [msg[i:i + room] for i in range(0, len(msg), room)] or [b""]
            for i, part in enumerate(parts):
                await self._packet((0x08 if i == 0 else 0) | (0x04 if i == len(parts) - 1 else 0), part)

    async def feed(self, packet):
        h = packet[0]
        counter = (h >> 4) & 7
        if counter != self.rx:
            self.errors.append(f"counter {counter}, expected {self.rx}")
        self.rx = (self.rx + 1) % 8
        if len(packet) > self.packet_size and self.connected.is_set():
            self.errors.append(f"packet of {len(packet)} bytes, over {self.packet_size}")
        if h & 0x80:
            cmd = h & 0x0F
            if cmd == 1 and self.central:  # confirm: version, size
                version, size = struct.unpack(">HH", packet[1:5])
                assert version == 1 and size <= self.wanted
                self.packet_size = size
                self.connected.set()
            elif cmd == 0 and not self.central:  # request from the computer
                lo, hi, size = struct.unpack(">HHH", packet[1:7])
                assert lo <= 1 <= hi
                self.packet_size = min(size or self.wanted, self.wanted)
                async with self._lock:
                    await self._packet(0x81, struct.pack(">HH", 1, self.packet_size))
                self.connected.set()
            elif cmd == 2:
                self.reader.feed_eof()
            return
        if h & 0x08:
            self.parts = b""
        self.parts += packet[1:]
        if h & 0x04:
            msg, self.parts = self.parts, None
            if msg[:3] == b"\x00\x00\x00":
                self.controls.append(msg[3:])
            elif msg[:3] == HASH:
                self.reader.feed_data(msg[3:])
            else:
                self.errors.append("message without a service hash")


def pair(engine_central, packet_size=20):
    """(engine link, phone link, PhoneWeave): wire them with
    engine_sock.feed / phone.feed afterwards."""
    to_phone, to_engine = Link(), Link()
    phone = PhoneWeave(to_engine, central=not engine_central, packet_size=packet_size)
    to_phone.connect(phone.feed)
    return to_phone, to_engine, phone


# ── Bandwidth upgrade, the phone's side ──────────────────────────────────
def bwu(event, **fields):
    f = qp.offline.OfflineFrame(version=qp.offline.OfflineFrame.V1)
    f.v1.type = qp.V1.BANDWIDTH_UPGRADE_NEGOTIATION
    f.v1.bandwidth_upgrade_negotiation.event_type = event
    for k, v in fields.items():
        getattr(f.v1.bandwidth_upgrade_negotiation, k).CopyFrom(v)
    return f


def plain_disconnection():
    f = qp.offline.OfflineFrame(version=qp.offline.OfflineFrame.V1)
    f.v1.type = qp.V1.DISCONNECTION
    return f.SerializeToString()


async def read_raw(reader):
    (n,) = struct.unpack(">I", await reader.readexactly(4))
    return await reader.readexactly(n)


async def write_raw(writer, data):
    writer.write(struct.pack(">I", len(data)) + data)
    await writer.drain()


async def drain_and_switch(p, new_r, new_w, seen, plain=False):
    """After the intro/ack: LAST_WRITE on the old channel, answer the
    computer's LAST_WRITE, stop at its SAFE_TO_CLOSE, say goodbye in the
    clear, carry on over the new channel (same keys, same counters).
    `seen` collects the other frames that arrived on the old channel."""
    await p.send(bwu(BWU.LAST_WRITE_TO_PRIOR_CHANNEL))
    while True:
        f = await p.recv()
        if f.v1.type == qp.V1.BANDWIDTH_UPGRADE_NEGOTIATION:
            e = f.v1.bandwidth_upgrade_negotiation.event_type
            if e == BWU.LAST_WRITE_TO_PRIOR_CHANNEL:
                await p.send(bwu(BWU.SAFE_TO_CLOSE_PRIOR_CHANNEL))
                continue
            if e == BWU.SAFE_TO_CLOSE_PRIOR_CHANNEL:
                break
        seen.append(f)
    old_r, old_w = p.r, p.w
    await write_raw(old_w, plain_disconnection())
    goodbye = qp.offline.OfflineFrame()
    goodbye.ParseFromString(await asyncio.wait_for(read_raw(old_r), 5))  # in the clear
    assert goodbye.v1.type == qp.V1.DISCONNECTION
    p.r, p.w = new_r, new_w
    p.plain = plain  # (as Nearby: the new channel without its encryption, when the offer said so)


async def respond_to_offer(p, offer, seen):
    """The phone was offered a path: connect, introduce, switch."""
    info = offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info
    if info.medium == BWU.UpgradePathInfo.WIFI_LAN:
        host = ".".join(str(b) for b in info.wifi_lan_socket.ip_address)
        port = info.wifi_lan_socket.wifi_port
    elif info.medium == BWU.UpgradePathInfo.WIFI_DIRECT:
        host, port = info.wifi_direct_credentials.gateway, info.wifi_direct_credentials.port
    else:
        host, port = info.wifi_hotspot_credentials.gateway, info.wifi_hotspot_credentials.port
    r, w = await asyncio.open_connection(host, port)
    intro = BWU.ClientIntroduction(endpoint_id="PHON")
    await write_raw(w, bwu(BWU.CLIENT_INTRODUCTION, client_introduction=intro).SerializeToString())
    if info.supports_client_introduction_ack:
        ack = qp.offline.OfflineFrame()
        ack.ParseFromString(await read_raw(r))
        assert ack.v1.bandwidth_upgrade_negotiation.event_type == BWU.CLIENT_INTRODUCTION_ACK
    await drain_and_switch(p, r, w, seen)
    return info


async def offer_and_switch(p, info, server_queue, seen):
    """The phone offers a path (it hosts): wait for the computer, ack, switch."""
    info.supports_client_introduction_ack = True
    await p.send(bwu(BWU.UPGRADE_PATH_AVAILABLE, upgrade_path_info=info))
    r, w = await asyncio.wait_for(server_queue.get(), 10)
    intro = qp.offline.OfflineFrame()
    intro.ParseFromString(await read_raw(r))
    b = intro.v1.bandwidth_upgrade_negotiation
    assert b.event_type == BWU.CLIENT_INTRODUCTION
    await write_raw(w, bwu(BWU.CLIENT_INTRODUCTION_ACK,
                           client_introduction_ack=BWU.ClientIntroductionAck()).SerializeToString())
    plain = info.supports_disabling_encryption
    assert b.client_introduction.supports_disabling_encryption == plain
    await drain_and_switch(p, r, w, seen, plain=plain)
    return b.client_introduction.endpoint_id
