"""How frames travel: a byte stream, each frame prefixed with its length.

Every medium Nearby Connections uses (a TCP socket on the local network, a
Wi-Fi Direct or hotspot socket, a Bluetooth socket) carries the same frames
the same way: a 4-byte big-endian length, then the frame. A Transport wraps
one such stream; the protocol above it never knows which medium it is:
StreamTransport is TCP (the local network, or a Wi-Fi Direct group or
hotspot after an upgrade), weave.BleSocketTransport is Bluetooth LE.
"""
import asyncio
import struct

# Frames are protobufs of a chunk at most (Nearby sends files in chunks of
# up to 512 KiB, plus encryption); anything far bigger is garbage or abuse
MAX_FRAME = 8 * 1024 * 1024


class TransportClosed(Exception):
    """The other side went away (or sent something that isn't a frame)."""


class Transport:
    """One connection's byte stream. Subclasses provide the medium."""

    medium = "unknown"

    async def read_frame(self, timeout=None):
        raise NotImplementedError

    async def write_frame(self, data):
        raise NotImplementedError

    async def close(self):
        raise NotImplementedError

    @property
    def peer(self):
        return ""


class StreamTransport(Transport):
    """A transport over asyncio streams: TCP today, any socket-like medium later."""

    medium = "wifi_lan"

    def __init__(self, reader, writer, medium=None):
        self._reader = reader
        self._writer = writer
        self._write_lock = asyncio.Lock()
        if medium:
            self.medium = medium  # (an upgrade's "wifi_direct", "wifi_hotspot")

    @classmethod
    async def connect(cls, host, port, timeout=10, medium=None, local_address=None):
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(host, port, local_addr=(local_address, 0) if local_address else None), timeout)
        return cls(reader, writer, medium)

    @property
    def peer(self):
        info = self._writer.get_extra_info("peername")
        return f"{info[0]}:{info[1]}" if info else ""

    async def read_frame(self, timeout=None):
        try:
            head = await asyncio.wait_for(self._reader.readexactly(4), timeout)
            (size,) = struct.unpack(">I", head)
            if size > MAX_FRAME:
                raise TransportClosed(f"frame of {size} bytes is too big")
            return await asyncio.wait_for(self._reader.readexactly(size), timeout)
        except (asyncio.IncompleteReadError, ConnectionError, OSError) as e:
            raise TransportClosed(str(e) or "connection closed") from e

    async def write_frame(self, data):
        if len(data) > MAX_FRAME:
            raise ValueError("frame too big")
        async with self._write_lock:
            try:
                self._writer.write(struct.pack(">I", len(data)) + data)
                await self._writer.drain()
            except (ConnectionError, OSError) as e:
                raise TransportClosed(str(e) or "connection closed") from e

    async def close(self):
        try:
            self._writer.close()
            await asyncio.wait_for(self._writer.wait_closed(), 2)
        except (asyncio.TimeoutError, ConnectionError, OSError):
            pass
