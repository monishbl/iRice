"""Nearby Connections over one Transport.

A connection starts in the clear: the connecting side (the client) sends a
ConnectionRequest with its endpoint info, then UKEY2 runs (ukey2.py), then
both sides send a ConnectionResponse (accept). From there every frame is
encrypted (secure.py). Encrypted frames are OfflineFrames:
  PAYLOAD_TRANSFER  data, in chunks; Nearby Sharing's own messages are small
                    BYTES payloads, files are FILE payloads
  KEEP_ALIVE        sent every few seconds both ways; silence means gone
  DISCONNECTION     the other side is done
A payload arrives as chunks with offsets; a chunk flagged LAST_CHUNK (with
no body) ends it. Files go straight to a sink (sharing.py decides which
payloads are welcome and where they land); anything unexpected ends the
connection instead of being stored.

Bandwidth upgrade (BANDWIDTH_UPGRADE_NEGOTIATION, google/nearby
connections/implementation/bwu_manager.cc): a connection that started over
Bluetooth moves to a faster medium mid-way, keeping its keys and sequence
numbers. One side (the initiator) offers a path (UPGRADE_PATH_AVAILABLE:
its address on the network, or a Wi-Fi Direct group / hotspot to join);
the other connects to it and introduces itself in the clear
(CLIENT_INTRODUCTION, answered by CLIENT_INTRODUCTION_ACK). Then both:
  1. send from now on over the new channel, held back until 3;
  2. say LAST_WRITE_TO_PRIOR_CHANNEL on the old one, keep reading it, and
     answer the other's LAST_WRITE with SAFE_TO_CLOSE_PRIOR_CHANNEL;
  3. on the other's SAFE_TO_CLOSE: a DISCONNECTION in the clear on the old
     channel, close it, read and write on the new one.
Frames arriving on the old channel meanwhile (payload chunks…) are handled
as usual. A side that can't use the path answers UPGRADE_FAILURE, and the
connection simply stays where it is.
"""
import asyncio
import os
import random

from . import debuglog, offline, ukey2
from .endpoint import parse_endpoint_info
from .secure import ChannelError, SecureChannel
from .transport import TransportClosed

V1 = offline.V1Frame
PT = offline.PayloadTransferFrame
LAST_CHUNK = PT.PayloadChunk.LAST_CHUNK
CHUNK = 512 * 1024  # what Nearby itself sends files in
MAX_BYTES_PAYLOAD = 1024 * 1024  # control messages and shared text are small
HANDSHAKE_TIMEOUT = 15
KEEP_ALIVE = 10  # seconds between our keep-alives
IDLE_TIMEOUT = 45  # no frame at all for this long: the other side is gone
BWU = offline.BandwidthUpgradeNegotiationFrame
PATH = BWU.UpgradePathInfo
UPGRADE_INTRO_TIMEOUT = 10  # the new channel's introduction, once connected
UPGRADE_DRAIN_TIMEOUT = 20  # LAST_WRITE … SAFE_TO_CLOSE on the old channel


class ProtocolError(Exception):
    """The other device sent something the protocol doesn't allow."""


def new_payload_id():
    # Positive 63-bit, as Java's random longs used for payload ids usually are
    return random.SystemRandom().getrandbits(62) + 1


def _frame(kind, **fields):
    f = offline.OfflineFrame(version=offline.OfflineFrame.V1)
    f.v1.type = kind
    for name, value in fields.items():
        getattr(f.v1, name).CopyFrom(value)
    return f


def bwu_frame(event, **fields):
    b = BWU(event_type=event)
    for name, value in fields.items():
        getattr(b, name).CopyFrom(value)
    return _frame(V1.BANDWIDTH_UPGRADE_NEGOTIATION, bandwidth_upgrade_negotiation=b)


def _parse_offline(raw):
    f = offline.OfflineFrame()
    try:
        f.ParseFromString(raw)
    except Exception as e:
        raise ProtocolError("unreadable frame") from e
    return f


class Connection:
    """One Nearby Connections link: handshake, then frames and payloads.

    Callbacks (set by the layer above):
      on_bytes(payload_id, data)   a complete BYTES payload
      on_file_progress(payload_id, done, total)
      on_file_done(payload_id)
      on_payload_cancelled(payload_id)
      on_upgrade_offer(path_info)  async → a connected Transport for the
                                   path the other side offers, or None
      on_upgraded(medium)          the connection moved ("wifi_direct"…)
    Files are written through sinks registered with expect_file()."""

    def __init__(self, transport, *, is_client, endpoint_id, endpoint_info, device_name=""):
        self.transport = transport  # where frames go (the new channel, mid-upgrade)
        self._reader = transport  # where they come from (the old one until it's closed)
        self.is_client = is_client
        self.endpoint_id = endpoint_id
        self.endpoint_info = endpoint_info
        self.device_name = device_name
        self.remote_id = ""
        self.remote = None  # parsed endpoint info of the other side
        self.pin = ""
        self._channel = None
        self._incoming = {}  # payload id -> bytearray (BYTES) or FileSink (FILE)
        self._sinks = {}  # payload id -> FileSink, expected files
        self._closed = False
        self._keepalive = None
        # Upgrades: what the client lists in its request (the mediums it can
        # move to, and its Wi-Fi details), what the other side listed
        self.mediums = [offline.ConnectionRequestFrame.WIFI_LAN]
        self.medium_metadata = None
        self.remote_mediums = []
        self.remote_metadata = None
        self._send_lock = asyncio.Lock()
        self._resumed = asyncio.Event()  # cleared while an upgrade holds sends back
        self._resumed.set()
        self._old = None  # the prior channel, while draining
        self._peer_last_write = False
        self._plain = set()  # channels without Nearby's encryption (see _seal)
        self._upgrading = False
        self._upgrade_failed = asyncio.Event()
        self._upgraded = asyncio.Event()
        self._tasks = set()
        self.tag = "connection"
        self.on_upgrade_offer = None
        self.on_upgraded = lambda medium: None
        self.on_bytes = lambda pid, data: None
        self.on_file_progress = lambda pid, done, total: None
        self.on_file_done = lambda pid: None
        self.on_payload_cancelled = lambda pid: None

    # ── Handshake ────────────────────────────────────────────────────────
    async def handshake(self):
        try:
            if self.is_client:
                await self._handshake_client()
            else:
                await self._handshake_server()
        except asyncio.TimeoutError as e:
            raise ProtocolError("the other device stopped answering while connecting") from e
        self._keepalive = asyncio.ensure_future(self._keepalive_loop())

    async def _read_plain(self):
        return await self.transport.read_frame(HANDSHAKE_TIMEOUT)

    def _log(self, what):
        debuglog.log(self.tag, what)

    async def _handshake_server(self):
        req = _parse_offline(await self._read_plain())
        if req.v1.type != V1.CONNECTION_REQUEST:
            raise ProtocolError("expected a connection request")
        r = req.v1.connection_request
        self._log_frame("←", req, self.transport)
        self.remote_id = r.endpoint_id
        self.remote_mediums = list(r.mediums)
        if r.HasField("medium_metadata"):
            self.remote_metadata = r.medium_metadata
        self.remote = parse_endpoint_info(r.endpoint_info) or {"type": "unknown", "hidden": True, "name": None}
        if not self.remote.get("name") and r.endpoint_name:
            self.remote["name"] = r.endpoint_name
        server = ukey2.Server()
        await self.transport.write_frame(server.init(await self._read_plain()))
        server.finish(await self._read_plain())
        self.pin = server.keys.pin
        # The client's ConnectionResponse comes in the clear, then ours
        resp = _parse_offline(await self._read_plain())
        if resp.v1.type != V1.CONNECTION_RESPONSE:
            raise ProtocolError("expected a connection response")
        if resp.v1.connection_response.response == offline.ConnectionResponseFrame.REJECT:
            raise ProtocolError("the other device rejected the connection")
        await self.transport.write_frame(self._our_response().SerializeToString())
        self._channel = SecureChannel(server.keys.next_secret, is_client=False)
        self._log(f"connected (UKEY2 as server) [{self.transport.medium}]")

    async def _handshake_client(self):
        req = offline.ConnectionRequestFrame(endpoint_id=self.endpoint_id, endpoint_name=self.device_name,
                                             endpoint_info=self.endpoint_info, nonce=random.SystemRandom().getrandbits(31),
                                             keep_alive_interval_millis=KEEP_ALIVE * 1000,
                                             keep_alive_timeout_millis=IDLE_TIMEOUT * 1000)
        req.mediums.extend(self.mediums)
        if self.medium_metadata is not None:
            req.medium_metadata.CopyFrom(self.medium_metadata)
        request = _frame(V1.CONNECTION_REQUEST, connection_request=req)
        self._log_frame("→", request, self.transport)
        await self.transport.write_frame(request.SerializeToString())
        client = ukey2.Client()
        await self.transport.write_frame(client.client_init())
        await self.transport.write_frame(client.finish(await self._read_plain()))
        self.pin = client.keys.pin
        await self.transport.write_frame(self._our_response().SerializeToString())
        resp = _parse_offline(await self._read_plain())
        if resp.v1.type != V1.CONNECTION_RESPONSE:
            raise ProtocolError("expected a connection response")
        if resp.v1.connection_response.response == offline.ConnectionResponseFrame.REJECT:
            raise ProtocolError("the other device rejected the connection")
        self._channel = SecureChannel(client.keys.next_secret, is_client=True)
        self._log(f"connected (UKEY2 as client) [{self.transport.medium}]")

    def _our_response(self):
        resp = offline.ConnectionResponseFrame(status=0, response=offline.ConnectionResponseFrame.ACCEPT,
                                               keep_alive_timeout_millis=IDLE_TIMEOUT * 1000)
        resp.os_info.type = offline.OsInfo.LINUX
        return _frame(V1.CONNECTION_RESPONSE, connection_response=resp)

    # ── Encrypted frames ─────────────────────────────────────────────────
    async def send(self, frame):
        if self._closed:
            raise TransportClosed("connection closed")
        while True:
            await self._resumed.wait()  # (an upgrade switching channels)
            async with self._send_lock:
                if not self._resumed.is_set():
                    continue
                if self._closed:
                    raise TransportClosed("connection closed")
                self._log_frame("→", frame, self.transport)
                await self.transport.write_frame(self._seal(self.transport, frame.SerializeToString()))
                return

    async def _send_on(self, transport, frame):
        """A frame on a given channel (the old one, mid-upgrade)."""
        async with self._send_lock:
            self._log_frame("→", frame, transport)
            await transport.write_frame(self._seal(transport, frame.SerializeToString()))

    # A channel a bandwidth upgrade opened without Nearby's own encryption
    # (the link has its own: Wi-Fi Direct and hotspots are WPA2) carries
    # plain OfflineFrames; every other channel, encrypted ones (bwu_manager.cc
    # RunUpgradeProtocol's enable_encryption)
    def _seal(self, transport, data):
        return data if transport in self._plain else self._channel.encrypt(data)

    def _open(self, transport, raw):
        return raw if transport in self._plain else self._channel.decrypt(raw)

    def _log_frame(self, arrow, frame, transport):
        if debuglog.enabled():
            self._log(f"{arrow} {debuglog.frame_name(frame)} [{transport.medium}]")

    async def receive(self):
        """The next frame that isn't handled here (keep-alives, payload
        chunks and upgrades are), or None when the other side disconnected."""
        while True:
            try:
                raw = await self._reader.read_frame(IDLE_TIMEOUT)
            except asyncio.TimeoutError as e:
                raise ProtocolError("the other device stopped responding") from e
            try:
                frame = _parse_offline(self._open(self._reader, raw))
            except ChannelError as e:
                raise ProtocolError(f"a message didn't check out ({e})") from e
            if debuglog.enabled():
                self._log(f"← {debuglog.frame_name(frame)} [{self._reader.medium}]")
            kind = frame.v1.type
            if kind == V1.KEEP_ALIVE:
                if not frame.v1.keep_alive.ack:
                    ack = offline.KeepAliveFrame(ack=True, seq_num=frame.v1.keep_alive.seq_num)
                    self._spawn(self._quietly(self.send(_frame(V1.KEEP_ALIVE, keep_alive=ack))))
                continue
            if kind == V1.DISCONNECTION:
                return None
            if kind == V1.PAYLOAD_TRANSFER:
                await self._payload(frame.v1.payload_transfer)
                continue
            if kind == V1.BANDWIDTH_UPGRADE_NEGOTIATION:
                await self._bwu(frame.v1.bandwidth_upgrade_negotiation)
                continue
            return frame

    def _spawn(self, coro):
        t = asyncio.ensure_future(coro)
        self._tasks.add(t)
        t.add_done_callback(self._tasks.discard)
        return t

    @staticmethod
    async def _quietly(coro):
        try:
            await coro
        except (TransportClosed, OSError):
            pass

    async def run(self):
        """Handles frames until the other side disconnects."""
        while await self.receive() is not None:
            pass  # (nothing else is expected on a Quick Share connection)

    async def _keepalive_loop(self):
        seq = 0
        try:
            while not self._closed:
                await asyncio.sleep(KEEP_ALIVE)
                seq += 1
                await self.send(_frame(V1.KEEP_ALIVE, keep_alive=offline.KeepAliveFrame(ack=False, seq_num=seq)))
        except (TransportClosed, asyncio.CancelledError):
            pass

    # ── Bandwidth upgrade ────────────────────────────────────────────────
    async def _bwu(self, b):
        event = b.event_type
        if event == BWU.UPGRADE_PATH_AVAILABLE:
            if self._upgrading or self._old is not None:
                return  # (one at a time; Nearby closes everything here, we just stay)
            self._upgrading = True
            self._spawn(self._respond(b.upgrade_path_info))
        elif event == BWU.LAST_WRITE_TO_PRIOR_CHANNEL:
            if self._old is not None:
                await self._send_on(self._old, bwu_frame(BWU.SAFE_TO_CLOSE_PRIOR_CHANNEL))
            else:
                self._peer_last_write = True  # (it overtook our switch: answered there)
        elif event == BWU.SAFE_TO_CLOSE_PRIOR_CHANNEL:
            if self._old is not None:
                await self._close_prior()
        elif event == BWU.UPGRADE_FAILURE:
            self._log("the other device couldn't use the upgrade path")
            self._upgrade_failed.set()
        # (CLIENT_INTRODUCTION/_ACK only travel in the clear on a new channel)

    async def _respond(self, info):
        """We were offered a path: connect, introduce ourselves, switch."""
        new = None
        try:
            if self.on_upgrade_offer is not None:
                new = await self.on_upgrade_offer(info)
            if new is None:
                raise ProtocolError("no way to use that path")
            # The offer says whether the new channel goes without Nearby's
            # encryption; Quick Share (point to point) always goes along
            # (bwu_manager.cc: enable_encryption = !supports_disabling ||
            # !local_supports_disabling), and says so in the introduction.
            # A phone that offers it doesn't encrypt there, whatever we say
            plain = bool(info.supports_disabling_encryption)
            intro = BWU.ClientIntroduction(endpoint_id=self.endpoint_id, supports_disabling_encryption=plain)
            if plain:
                self._plain.add(new)
            await new.write_frame(bwu_frame(BWU.CLIENT_INTRODUCTION, client_introduction=intro).SerializeToString())
            self._log(f"→ CLIENT_INTRODUCTION [{new.medium}]")
            if info.supports_client_introduction_ack:
                ack = _parse_offline(await new.read_frame(UPGRADE_INTRO_TIMEOUT))
                if ack.v1.type != V1.BANDWIDTH_UPGRADE_NEGOTIATION or \
                        ack.v1.bandwidth_upgrade_negotiation.event_type != BWU.CLIENT_INTRODUCTION_ACK:
                    raise ProtocolError("the new channel didn't acknowledge us")
                self._log(f"← CLIENT_INTRODUCTION_ACK [{new.medium}]")
            await self._switch(new)
        except asyncio.CancelledError:
            if new is not None and self._old is None:
                await new.close()
            raise
        except (ProtocolError, TransportClosed, OSError, asyncio.TimeoutError, ValueError) as e:
            self._log(f"upgrade failed: {e}")
            if new is not None:
                await new.close()
            self._upgrading = False
            failure = PATH(medium=info.medium)
            try:
                await self.send(bwu_frame(BWU.UPGRADE_FAILURE, upgrade_path_info=failure))
            except (TransportClosed, OSError):
                pass

    async def offer_upgrade(self, info, accept, timeout=30):
        """We offer a path (info: UpgradePathInfo); accept(timeout) waits for
        the other side's connection on it (a Transport). True once the
        connection has moved; False if the other side couldn't (it stays)."""
        if self._upgrading or self._closed:
            return False
        self._upgrading = True
        self._upgrade_failed.clear()
        info.supports_client_introduction_ack = True
        info.supports_disabling_encryption = False
        new = None
        try:
            await self.send(bwu_frame(BWU.UPGRADE_PATH_AVAILABLE, upgrade_path_info=info))
            arriving = asyncio.ensure_future(accept(timeout))
            failed = asyncio.ensure_future(self._upgrade_failed.wait())
            try:
                done, _ = await asyncio.wait({arriving, failed}, timeout=timeout, return_when=asyncio.FIRST_COMPLETED)
            finally:
                failed.cancel()
            if arriving not in done:
                arriving.cancel()
                return False
            new = arriving.result()
            if new is None:
                return False
            intro = _parse_offline(await new.read_frame(UPGRADE_INTRO_TIMEOUT))
            if intro.v1.type != V1.BANDWIDTH_UPGRADE_NEGOTIATION or \
                    intro.v1.bandwidth_upgrade_negotiation.event_type != BWU.CLIENT_INTRODUCTION:
                raise ProtocolError("the new channel didn't start with an introduction")
            who = intro.v1.bandwidth_upgrade_negotiation.client_introduction.endpoint_id
            if who != self.remote_id:
                raise ProtocolError("someone else connected to the upgrade path")
            self._log(f"← CLIENT_INTRODUCTION [{new.medium}]")
            if intro.v1.bandwidth_upgrade_negotiation.client_introduction.supports_disabling_encryption:
                self._plain.add(new)  # (it asked for it: Nearby's hosts go along)
            await new.write_frame(bwu_frame(BWU.CLIENT_INTRODUCTION_ACK,
                                            client_introduction_ack=BWU.ClientIntroductionAck()).SerializeToString())
            self._log(f"→ CLIENT_INTRODUCTION_ACK [{new.medium}]")
            await self._switch(new)
            new = None
            await asyncio.wait_for(self._upgraded.wait(), UPGRADE_DRAIN_TIMEOUT)
            return not self._closed
        except (ProtocolError, TransportClosed, OSError, asyncio.TimeoutError, ValueError) as e:
            self._log(f"upgrade failed: {e}")
            if new is not None:
                await new.close()
            return False
        finally:
            if not self._upgraded.is_set() and self._old is None:
                self._upgrading = False

    async def _switch(self, new):
        """Sends go to the new channel (held back); LAST_WRITE on the old one."""
        async with self._send_lock:
            self._resumed.clear()
            self._old, self.transport = self.transport, new
        self._log(f"switching {self._old.medium} → {new.medium}")
        await self._send_on(self._old, bwu_frame(BWU.LAST_WRITE_TO_PRIOR_CHANNEL))
        if self._peer_last_write:
            self._peer_last_write = False
            await self._send_on(self._old, bwu_frame(BWU.SAFE_TO_CLOSE_PRIOR_CHANNEL))
        self._spawn(self._drain_deadline())

    async def _drain_deadline(self):
        await asyncio.sleep(UPGRADE_DRAIN_TIMEOUT)
        if self._old is not None and not self._closed:
            self._log("the old channel never closed: giving up")
            await self.close(say_goodbye=False)

    async def _close_prior(self):
        """The other side is done with the old channel: say so in the clear,
        close it, carry on over the new one."""
        old, self._old = self._old, None
        bye = _frame(V1.DISCONNECTION, disconnection=offline.DisconnectionFrame(
            request_safe_to_disconnect=False, ack_safe_to_disconnect=False))
        try:
            await old.write_frame(bye.SerializeToString())
            await asyncio.wait_for(old.read_frame(), 2)  # (its own goodbye)
        except (TransportClosed, OSError, asyncio.TimeoutError):
            pass
        await old.close()
        self._reader = self.transport
        self._upgrading = False
        self._upgraded.set()
        self._resumed.set()
        self._log(f"upgraded to {self.transport.medium}")
        try:
            self.on_upgraded(self.transport.medium)
        except Exception:  # noqa: BLE001 (a UI callback mustn't break the link)
            pass

    @property
    def medium(self):
        return self.transport.medium

    # ── Payloads in ──────────────────────────────────────────────────────
    def expect_file(self, payload_id, sink):
        """Accepts the FILE payload with this id into the sink (sharing.py)."""
        self._sinks[payload_id] = sink

    async def _payload(self, p):
        h = p.payload_header
        pid = h.id
        if p.packet_type == PT.CONTROL:
            if p.control_message.event in (PT.ControlMessage.PAYLOAD_CANCELED, PT.ControlMessage.PAYLOAD_ERROR):
                sink = self._incoming.pop(pid, None)
                if hasattr(sink, "fail"):
                    sink.fail()
                self.on_payload_cancelled(pid)
            return
        if p.packet_type != PT.DATA:
            return
        chunk = p.payload_chunk
        last = bool(chunk.flags & LAST_CHUNK)
        if h.type == PT.PayloadHeader.BYTES:
            if h.total_size > MAX_BYTES_PAYLOAD:
                raise ProtocolError("a message payload is too big")
            buf = self._incoming.setdefault(pid, bytearray())
            if chunk.offset != len(buf):
                raise ProtocolError("message payload chunks out of order")
            buf += chunk.body
            if len(buf) > MAX_BYTES_PAYLOAD:
                raise ProtocolError("a message payload is too big")
            if last:
                del self._incoming[pid]
                self.on_bytes(pid, bytes(buf))
        elif h.type == PT.PayloadHeader.FILE:
            sink = self._incoming.get(pid) or self._sinks.pop(pid, None)
            if sink is None:
                raise ProtocolError("a file arrived that wasn't announced or accepted")
            self._incoming[pid] = sink
            if h.total_size != sink.size:
                raise ProtocolError("a file is a different size than announced")
            if chunk.body:
                sink.write(chunk.offset, chunk.body)
                self.on_file_progress(pid, sink.written, sink.size)
            if last:
                del self._incoming[pid]
                sink.finish()
                self.on_file_done(pid)
        else:
            raise ProtocolError("unsupported payload type")

    # ── Payloads out ─────────────────────────────────────────────────────
    async def send_bytes(self, data, payload_id=None):
        pid = payload_id or new_payload_id()
        header = PT.PayloadHeader(id=pid, type=PT.PayloadHeader.BYTES, total_size=len(data))
        await self._chunk(header, 0, data, 0)
        await self._chunk(header, len(data), b"", LAST_CHUNK)
        return pid

    async def send_file(self, payload_id, path, size, name, progress=None, cancelled=None):
        """Streams a file; progress(done, size); stops early if cancelled()."""
        header = PT.PayloadHeader(id=payload_id, type=PT.PayloadHeader.FILE, total_size=size, file_name=name)
        done = 0
        with open(path, "rb") as f:
            while done < size:
                if cancelled and cancelled():
                    await self.cancel_payload(payload_id)
                    return False
                body = f.read(min(getattr(self.transport, "max_chunk", CHUNK), size - done))
                if not body:
                    raise OSError(f"{name} got shorter while it was being sent")
                await self._chunk(header, done, body, 0)
                done += len(body)
                if progress:
                    progress(done, size)
        await self._chunk(header, size, b"", LAST_CHUNK)
        return True

    async def _chunk(self, header, offset, body, flags):
        p = PT(packet_type=PT.DATA)
        p.payload_header.CopyFrom(header)
        p.payload_chunk.flags = flags
        p.payload_chunk.offset = offset
        p.payload_chunk.body = body
        await self.send(_frame(V1.PAYLOAD_TRANSFER, payload_transfer=p))

    async def cancel_payload(self, payload_id):
        p = PT(packet_type=PT.CONTROL)
        p.payload_header.id = payload_id
        p.control_message.event = PT.ControlMessage.PAYLOAD_CANCELED
        try:
            await self.send(_frame(V1.PAYLOAD_TRANSFER, payload_transfer=p))
        except TransportClosed:
            pass

    # ── Ending ───────────────────────────────────────────────────────────
    async def close(self, say_goodbye=True):
        if self._closed:
            return
        if say_goodbye and self._channel is not None:
            try:
                await self.send(_frame(V1.DISCONNECTION, disconnection=offline.DisconnectionFrame()))
            except (TransportClosed, OSError):
                pass
        self._closed = True
        self._resumed.set()  # (wakes sends held back by an upgrade: they fail)
        if self._keepalive:
            self._keepalive.cancel()
        for t in list(self._tasks):
            if t is not asyncio.current_task():
                t.cancel()
        for sink in list(self._incoming.values()) + list(self._sinks.values()):
            if hasattr(sink, "fail"):
                sink.fail()
        self._incoming.clear()
        self._sinks.clear()
        if self._old is not None:
            await self._old.close()
            self._old = None
        await self.transport.close()


def file_size(path):
    return os.stat(path).st_size
