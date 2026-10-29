"""A log of what Quick Share does, for testing with a phone: on with
IRICE_QS_DEBUG=1 or the file $XDG_RUNTIME_DIR/irice-quickshare.debug.

It goes to $XDG_RUNTIME_DIR/irice-quickshare.log (private to you, gone at
logout), one line per event: the time, the connection, and what happened.
Frames are logged by kind and size only: never keys, PINs, passwords, file
contents or names of files (the UKEY2 exchange and the encrypted frames are
described, not dumped). Off, every call is a no-op.
"""
import os
import time

_dir = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
_path = None
# (or `touch $XDG_RUNTIME_DIR/irice-quickshare.debug`: the shell starts the
# helper, so its environment is the shell's; the next start picks it up)
if os.environ.get("IRICE_QS_DEBUG") == "1" or os.path.exists(os.path.join(_dir, "irice-quickshare.debug")):
    _path = os.path.join(_dir, "irice-quickshare.log")


def enabled():
    return _path is not None


def log(where, what):
    if _path is None:
        return
    try:
        fd = os.open(_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
        with os.fdopen(fd, "a") as f:
            f.write(f"{time.strftime('%H:%M:%S')}.{int(time.time() * 1000) % 1000:03d} {where}: {what}\n")
    except OSError:
        pass


def frame_name(frame):
    """An OfflineFrame described without its contents: "PAYLOAD_TRANSFER
    data 16384 bytes", "BANDWIDTH_UPGRADE_NEGOTIATION UPGRADE_PATH_AVAILABLE
    WIFI_DIRECT"…"""
    from . import offline
    v1 = frame.v1
    try:
        kind = offline.V1Frame.FrameType.Name(v1.type)
    except ValueError:
        kind = str(v1.type)
    if v1.type == offline.V1Frame.PAYLOAD_TRANSFER:
        p = v1.payload_transfer
        if p.packet_type == offline.PayloadTransferFrame.DATA:
            return f"{kind} {offline.PayloadTransferFrame.PayloadHeader.PayloadType.Name(p.payload_header.type)} " \
                   f"{len(p.payload_chunk.body)} bytes at {p.payload_chunk.offset}{' last' if p.payload_chunk.flags & 1 else ''}"
        return f"{kind} control {p.control_message.event}"
    if v1.type == offline.V1Frame.BANDWIDTH_UPGRADE_NEGOTIATION:
        b = v1.bandwidth_upgrade_negotiation
        name = offline.BandwidthUpgradeNegotiationFrame.EventType.Name(b.event_type)
        if b.HasField("upgrade_path_info"):
            name += " " + offline.BandwidthUpgradeNegotiationFrame.UpgradePathInfo.Medium.Name(b.upgrade_path_info.medium)
        return f"{kind} {name}"
    if v1.type == offline.V1Frame.CONNECTION_REQUEST:
        r = v1.connection_request
        mediums = ",".join(offline.ConnectionRequestFrame.Medium.Name(m) for m in r.mediums)
        return f"{kind} mediums [{mediums}]"
    if v1.type == offline.V1Frame.KEEP_ALIVE:
        return f"{kind}{' ack' if v1.keep_alive.ack else ''}"
    return kind
