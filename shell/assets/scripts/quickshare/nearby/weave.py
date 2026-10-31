"""Quick Share's Bluetooth LE sockets: frames over GATT ("weave") or L2CAP.

Over Bluetooth LE, Nearby Connections carries the same [length][frame]
stream as over TCP (transport.py), inside two more layers:

  weave        (google/nearby internal/weave: packet.cc, packetizer.cc,
               base_socket.cc, sockets/{client,server}_socket.cc) turns
               messages into GATT writes and notifications of at most the
               negotiated packet size. Every packet starts with one byte:
                 control 1 CCC NNNN   counter (3 bits), command
                 data    0 CCC F L 00 counter, first / last packet of a message
               Each direction counts its packets 0…7 and around, control
               packets included. The central writes a ConnectionRequest
               (command 0: min and max version 1, its max packet size, 2
               bytes each), the peripheral answers ConnectionConfirm
               (command 1: version, the chosen packet size). Error (2) ends it.
  BLE socket   (connections/implementation/mediums/ble/ble_socket.cc,
               ble_packet.cc, proto/mediums/ble_frames.proto) prefixes each
               message with the service id hash, FC 9F 5E for data, or
               00 00 00 for a SocketControlFrame protobuf: INTRODUCTION
               (the central sends it first), DISCONNECTION, and
               PACKET_ACKNOWLEDGEMENT (L2CAP only: one per frame received).

L2CAP channels (mediums/ble/ble_l2cap_packet.cc) skip weave: every message
is [4-byte length][payload]; the central asks 0x03 (data connection), the
peripheral answers 0x17 (ready), then the BLE socket layer as above.

Everything here works on "packets in, packets out" (a link object with an
async send(packet)), so the same code runs over BlueZ (bluez.py), an L2CAP
socket, or an in-memory pair in the tests.
"""
import asyncio
import struct

from .endpoint import SERVICE_HASH
from .transport import MAX_FRAME, Transport, TransportClosed

CONTROL = 0x80
FIRST = 0x08
LAST = 0x04
REQUEST, CONFIRM, ERROR = 0, 1, 2
VERSION = 1
MIN_PACKET = 20  # what an ATT MTU of 23 leaves; phones' own GATT servers keep it
MAX_PACKET = 512
HANDSHAKE_TIMEOUT = 15
CONTROL_PREFIX = b"\x00\x00\x00"

# SocketControlFrame types (ble_frames.proto)
INTRODUCTION, DISCONNECTION, PACKET_ACKNOWLEDGEMENT = 1, 2, 3

# L2CAP commands (ble_l2cap_packet.h)
L2CAP_REQUEST_ADVERTISEMENT = 0x01
L2CAP_ADVERTISEMENT_FINISHED = 0x02
L2CAP_ADVERTISEMENT = 0x15
L2CAP_REQUEST_DATA = 0x03
L2CAP_NOT_FOUND = 0x16
L2CAP_READY = 0x17
L2CAP_FAILED = 0x18


class WeaveError(Exception):
    """The other side broke the weave or BLE socket protocol."""


# ── Weave packets ────────────────────────────────────────────────────────
def control_packet(command, payload=b""):
    return bytes([CONTROL | (command & 0x0F)]) + payload


def data_packet(first, last, payload):
    return bytes([(FIRST if first else 0) | (LAST if last else 0)]) + payload


def with_counter(packet, counter):
    return bytes([packet[0] | ((counter & 7) << 4)]) + packet[1:]


def connection_request(max_packet, extra=b""):
    if len(extra) > 13:
        raise ValueError("a connection request carries at most 13 bytes of extra data")
    return control_packet(REQUEST, struct.pack(">HHH", VERSION, VERSION, max_packet) + extra)


def connection_confirm(packet_size, extra=b""):
    if len(extra) > 15:
        raise ValueError("a connection confirm carries at most 15 bytes of extra data")
    return control_packet(CONFIRM, struct.pack(">HH", VERSION, packet_size) + extra)


def parse_packet(packet):
    """{"control", "counter", "command" | "first"/"last", "payload"}."""
    if not packet:
        raise WeaveError("empty packet")
    h = packet[0]
    out = {"control": bool(h & CONTROL), "counter": (h >> 4) & 7, "payload": bytes(packet[1:])}
    if out["control"]:
        out["command"] = h & 0x0F
    else:
        out["first"] = bool(h & FIRST)
        out["last"] = bool(h & LAST)
    return out


def split_message(message, packet_size):
    """A message as data packets (without counters) of at most packet_size."""
    room = packet_size - 1
    if room < 1:
        raise ValueError("packet size too small")
    if not message:
        return [data_packet(True, True, b"")]
    parts = [message[i:i + room] for i in range(0, len(message), room)]
    return [data_packet(i == 0, i == len(parts) - 1, p) for i, p in enumerate(parts)]


class WeaveSocket:
    """One end of a weave link: handshake, counters, (de)fragmenting.

    link.send(packet) transmits one packet (a GATT write or notification) and
    may be awaited for pacing; incoming packets are handed to feed(). The
    central (is_client) connects; the peripheral confirms."""

    def __init__(self, link, is_client, max_packet=MIN_PACKET, log=None):
        self.link = link
        self.is_client = is_client
        self.max_packet = max(MIN_PACKET, min(MAX_PACKET, max_packet))
        self.packet_size = MIN_PACKET
        self.connected = asyncio.Event()
        self.closed = False
        self.error = None
        self._tx = 0
        self._rx = 0
        self._parts = None
        self._messages = asyncio.Queue()
        self._lock = asyncio.Lock()
        self._log = log or (lambda *a: None)

    async def _transmit(self, packet):
        counter = self._tx
        self._tx = (self._tx + 1) & 7
        await self.link.send(with_counter(packet, counter))

    async def connect(self, timeout=HANDSHAKE_TIMEOUT):
        """Central: ConnectionRequest, then wait for the confirm."""
        async with self._lock:
            await self._transmit(connection_request(self.max_packet))
        try:
            await asyncio.wait_for(self.connected.wait(), timeout)
        except asyncio.TimeoutError as e:
            raise WeaveError("the other device didn't answer the Bluetooth connection") from e
        if self.closed:
            raise WeaveError(self.error or "the Bluetooth connection was refused")

    async def feed(self, packet):
        """One packet from the other side."""
        if self.closed:
            return
        try:
            p = parse_packet(packet)
        except WeaveError as e:
            await self._fail(str(e))
            return
        expected = self._rx
        self._rx = (self._rx + 1) & 7
        if p["control"]:
            # (base_socket.cc: a control packet is handled even when its
            # counter is off; a data packet out of turn ends the link)
            await self._control(p)
            return
        if p["counter"] != expected:
            await self._fail(f"Bluetooth packet {p['counter']} arrived out of turn (expected {expected})")
            return
        if not self.connected.is_set():
            await self._fail("data before the Bluetooth handshake")
            return
        if p["first"]:
            if self._parts is not None:
                await self._fail("a new message started before the last one ended")
                return
            self._parts = bytearray()
        elif self._parts is None:
            await self._fail("a message continued that never started")
            return
        self._parts += p["payload"]
        if len(self._parts) > MAX_FRAME + 64:
            await self._fail("message too big")
            return
        if p["last"]:
            msg, self._parts = bytes(self._parts), None
            self._messages.put_nowait(msg)

    async def _control(self, p):
        cmd, body = p["command"], p["payload"]
        if cmd == ERROR:
            self._log("weave: the other side ended the link")
            self._end("the other device closed the Bluetooth connection")
            return
        if not self.is_client and cmd == REQUEST and not self.connected.is_set():
            if len(body) < 6:
                await self._fail("short connection request")
                return
            lo, hi, size = struct.unpack(">HHH", body[:6])
            if lo > VERSION or hi < VERSION:
                await self._fail(f"unsupported weave versions {lo}-{hi}")
                return
            self.packet_size = self.max_packet if size == 0 else max(MIN_PACKET, min(self.max_packet, size))
            async with self._lock:
                await self._transmit(connection_confirm(self.packet_size))
            self.connected.set()
            self._log(f"weave: connected as peripheral, {self.packet_size}-byte packets")
            if len(body) > 6:
                self._messages.put_nowait(bytes(body[6:]))
            return
        if self.is_client and cmd == CONFIRM and not self.connected.is_set():
            if len(body) < 4:
                await self._fail("short connection confirm")
                return
            version, size = struct.unpack(">HH", body[:4])
            if version != VERSION or size > self.max_packet or size < MIN_PACKET:
                await self._fail(f"unusable connection confirm (version {version}, {size}-byte packets)")
                return
            self.packet_size = size
            self.connected.set()
            self._log(f"weave: connected as central, {size}-byte packets")
            if len(body) > 4:
                self._messages.put_nowait(bytes(body[4:]))
            return
        # A repeated or misplaced request/confirm: the other side restarted
        await self._fail("unexpected Bluetooth control packet")

    async def _fail(self, why):
        self._log(f"weave: {why}")
        if not self.closed:
            try:
                async with self._lock:
                    await self._transmit(control_packet(ERROR))
            except Exception:  # noqa: BLE001 (the link may be gone already)
                pass
        self._end(why)

    def _end(self, why):
        if self.closed:
            return
        self.closed = True
        self.error = why
        self.connected.set()  # (wakes a waiting connect())
        self._messages.put_nowait(None)

    async def send(self, message):
        if self.closed:
            raise TransportClosed(self.error or "Bluetooth connection closed")
        async with self._lock:
            for packet in split_message(message, self.packet_size):
                if self.closed:
                    raise TransportClosed(self.error or "Bluetooth connection closed")
                await self._transmit(packet)

    async def recv(self, timeout=None):
        msg = await asyncio.wait_for(self._messages.get(), timeout)
        if msg is None:
            self._messages.put_nowait(None)  # (stays closed for later readers)
            raise TransportClosed(self.error or "Bluetooth connection closed")
        return msg

    async def close(self):
        if self.closed:
            return
        try:
            async with self._lock:
                await self._transmit(control_packet(ERROR))
        except Exception:  # noqa: BLE001
            pass
        self._end("closed")


# ── L2CAP messages ───────────────────────────────────────────────────────
class L2capMessages:
    """[4-byte length][payload] messages over a byte link (an L2CAP channel).

    link.send(bytes) writes; received bytes go to feed(), in whatever pieces
    they arrive."""

    def __init__(self, link):
        self.link = link
        self._buf = bytearray()
        self._messages = asyncio.Queue()
        self.closed = False

    def feed(self, data):
        if self.closed:
            return
        if not data:
            self.end()
            return
        self._buf += data
        while len(self._buf) >= 4:
            (n,) = struct.unpack(">I", self._buf[:4])
            if n > MAX_FRAME + 64:
                self.end()
                return
            if len(self._buf) < 4 + n:
                break
            self._messages.put_nowait(bytes(self._buf[4:4 + n]))
            del self._buf[:4 + n]

    def end(self):
        if not self.closed:
            self.closed = True
            self._messages.put_nowait(None)

    async def send(self, message):
        if self.closed:
            raise TransportClosed("Bluetooth connection closed")
        await self.link.send(struct.pack(">I", len(message)) + message)

    async def recv(self, timeout=None):
        msg = await asyncio.wait_for(self._messages.get(), timeout)
        if msg is None:
            self._messages.put_nowait(None)
            raise TransportClosed("Bluetooth connection closed")
        return msg

    async def request_data_connection(self, timeout=HANDSHAKE_TIMEOUT):
        """Central: ask for the data connection, wait for "ready"."""
        await self.send(bytes([L2CAP_REQUEST_DATA]))
        try:
            answer = await self.recv(timeout)
        except asyncio.TimeoutError as e:
            raise WeaveError("the other device didn't answer over L2CAP") from e
        if answer[:1] != bytes([L2CAP_READY]):
            raise WeaveError({L2CAP_NOT_FOUND: "Quick Share isn't listening on that channel",
                              L2CAP_FAILED: "the other device refused the L2CAP connection"}.get(
                answer[0] if answer else -1, "unexpected L2CAP answer"))

    async def close(self):
        self.end()


# ── BLE socket control frames (hand-encoded: three tiny protobufs) ───────
def _varint(n):
    out = bytearray()
    n &= 0xFFFFFFFFFFFFFFFF
    while True:
        b = n & 0x7F
        n >>= 7
        out.append(b | (0x80 if n else 0))
        if not n:
            return bytes(out)


def _read_varint(raw, i):
    shift = n = 0
    while True:
        if i >= len(raw) or shift > 63:
            raise WeaveError("bad control frame")
        b = raw[i]
        i += 1
        n |= (b & 0x7F) << shift
        shift += 7
        if not b & 0x80:
            return n, i


def _fields(raw):
    """(field number, wire type, value) of a protobuf message."""
    i = 0
    while i < len(raw):
        key, i = _read_varint(raw, i)
        num, wt = key >> 3, key & 7
        if wt == 0:
            v, i = _read_varint(raw, i)
        elif wt == 2:
            n, i = _read_varint(raw, i)
            if i + n > len(raw):
                raise WeaveError("bad control frame")
            v, i = raw[i:i + n], i + n
        elif wt == 5:
            v, i = raw[i:i + 4], i + 4
        elif wt == 1:
            v, i = raw[i:i + 8], i + 8
        else:
            raise WeaveError("bad control frame")
        yield num, wt, v


def control_frame(kind, received_size=None, service_hash=SERVICE_HASH):
    """[00 00 00][SocketControlFrame] (ble_packet.cc CreateControl…Packet)."""
    body = b"\x0a" + _varint(len(service_hash)) + service_hash
    if kind == INTRODUCTION:
        body += b"\x10" + _varint(2)  # socket_version V2
    elif kind == PACKET_ACKNOWLEDGEMENT:
        body += b"\x10" + _varint(received_size or 0)
    field = {INTRODUCTION: 2, DISCONNECTION: 3, PACKET_ACKNOWLEDGEMENT: 4}[kind]
    frame = b"\x08" + _varint(kind) + bytes([(field << 3) | 2]) + _varint(len(body)) + body
    return CONTROL_PREFIX + frame


def parse_control_frame(raw):
    """{"type", "service_hash", "socket_version", "received_size"} of a
    SocketControlFrame (without the 00 00 00 prefix)."""
    out = {"type": 0, "service_hash": b"", "socket_version": 0, "received_size": 0}
    for num, wt, v in _fields(raw):
        if num == 1 and wt == 0:
            out["type"] = v
        elif num in (2, 3, 4) and wt == 2:
            for n2, w2, v2 in _fields(v):
                if n2 == 1 and w2 == 2:
                    out["service_hash"] = bytes(v2)
                elif n2 == 2 and w2 == 0:
                    out["socket_version" if num == 2 else "received_size"] = v2
    return out


# ── The transport ────────────────────────────────────────────────────────
class BleSocketTransport(Transport):
    """Nearby frames over BLE socket messages (from a WeaveSocket or
    L2capMessages): the Transport the rest of the engine sees.

    Data messages carry the [length][frame] stream after the service hash;
    a frame may span messages or share one (older Android writes the length
    and the frame separately), so they're joined into one stream."""

    medium = "ble"

    def __init__(self, messages, *, is_client, acks=False, peer="", log=None, medium=None):
        self._messages = messages
        self.is_client = is_client
        self.acks = acks  # PACKET_ACKNOWLEDGEMENT per frame (L2CAP)
        self._peer = peer
        self._buf = bytearray()
        self._closed = False
        self._log = log or (lambda *a: None)
        self._write_lock = asyncio.Lock()
        if medium:
            self.medium = medium
        # Frames go out in pieces this size at most (connection.py): a BLE
        # link moves a few KB/s to tens of KB/s, and phones choke on 512 KB
        # messages over L2CAP
        self.max_chunk = 16 * 1024

    @property
    def peer(self):
        return self._peer

    async def start(self):
        """The central introduces itself before anything else."""
        if self.is_client:
            await self._messages.send(control_frame(INTRODUCTION))

    async def _fill(self, need, timeout):
        while len(self._buf) < need:
            msg = await self._messages.recv(timeout)
            if msg[:3] == CONTROL_PREFIX:
                c = parse_control_frame(msg[3:])
                if c["type"] == DISCONNECTION:
                    self._log("ble: the other side disconnected")
                    raise TransportClosed("the other device disconnected")
                if c["service_hash"] and c["service_hash"] != SERVICE_HASH:
                    raise TransportClosed("a Bluetooth connection for another Nearby service")
                continue  # INTRODUCTION, PACKET_ACKNOWLEDGEMENT
            if msg[:3] == SERVICE_HASH:
                self._buf += msg[3:]
            else:
                raise TransportClosed("a Bluetooth message for another Nearby service")

    async def read_frame(self, timeout=None):
        if self._closed:
            raise TransportClosed("connection closed")
        try:
            await self._fill(4, timeout)
            (size,) = struct.unpack(">I", self._buf[:4])
            if size > MAX_FRAME:
                raise TransportClosed(f"frame of {size} bytes is too big")
            await self._fill(4 + size, timeout)
        except WeaveError as e:
            raise TransportClosed(str(e)) from e
        frame = bytes(self._buf[4:4 + size])
        del self._buf[:4 + size]
        if self.acks:
            try:
                await self._messages.send(control_frame(PACKET_ACKNOWLEDGEMENT, received_size=size))
            except TransportClosed:
                pass
        return frame

    async def write_frame(self, data):
        if len(data) > MAX_FRAME:
            raise ValueError("frame too big")
        if self._closed:
            raise TransportClosed("connection closed")
        async with self._write_lock:
            await self._messages.send(SERVICE_HASH + struct.pack(">I", len(data)) + data)

    async def close(self):
        if self._closed:
            return
        self._closed = True
        try:
            await asyncio.wait_for(self._messages.send(control_frame(DISCONNECTION)), 2)
        except (TransportClosed, asyncio.TimeoutError, OSError):
            pass
        await self._messages.close()
