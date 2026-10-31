"""Nearby Sharing: what Quick Share does over a Nearby connection.

Both sides first exchange PairedKeyEncryption and PairedKeyResult frames.
Those prove "this is one of my contacts' devices" with keys tied to a
Google account; iRice has none, so it sends random data and answers
UNABLE, as for any stranger with visibility set to Everyone. Then:
  sender    INTRODUCTION: the files (name, size, type, payload id), texts
            and Wi-Fi networks on offer
  receiver  asks you (showing the PIN both screens share), then RESPONSE:
            ACCEPT, or REJECT / NOT_ENOUGH_SPACE / UNSUPPORTED_ATTACHMENT_TYPE
  sender    one payload per item: files as FILE payloads, texts and Wi-Fi
            passwords as BYTES payloads with the announced ids
Either side can send CANCEL. Nearby Sharing's own frames travel as small
BYTES payloads (connection.py).
"""
import asyncio
import mimetypes
import os
import shutil

from . import sharing_enums, sharing_pb
from .connection import Connection, ProtocolError, new_payload_id
from .files import FileSink, safe_name
from .transport import TransportClosed

F = sharing_pb.V1Frame
RESP = sharing_pb.ConnectionResponseFrame
FILE_TYPES = {"image": sharing_pb.FileMetadata.IMAGE, "video": sharing_pb.FileMetadata.VIDEO,
              "audio": sharing_pb.FileMetadata.AUDIO, "text": sharing_pb.FileMetadata.DOCUMENT,
              "application": sharing_pb.FileMetadata.DOCUMENT}
TEXT_KINDS = {sharing_pb.TextMetadata.URL: "url", sharing_pb.TextMetadata.ADDRESS: "address",
              sharing_pb.TextMetadata.PHONE_NUMBER: "phone"}
DECISION_TIMEOUT = 60  # Android gives up waiting for an answer at about a minute
MAX_ITEMS = 1000


class Cancelled(Exception):
    """The other side cancelled, or we did."""


class Declined(Exception):
    """The receiver said no (reason in args[0])."""


def sharing_frame(kind, **fields):
    f = sharing_pb.Frame(version=sharing_pb.Frame.V1)
    f.v1.type = kind
    for name, value in fields.items():
        getattr(f.v1, name).CopyFrom(value)
    return f.SerializeToString()


def parse_sharing(data):
    f = sharing_pb.Frame()
    try:
        f.ParseFromString(data)
    except Exception as e:
        raise ProtocolError("unreadable Quick Share message") from e
    return f.v1


def file_type(name):
    mime = mimetypes.guess_type(name)[0] or "application/octet-stream"
    return mime, FILE_TYPES.get(mime.split("/")[0], sharing_pb.FileMetadata.UNKNOWN)


class Session:
    """What both directions share: the paired-key exchange and a frame queue."""

    def __init__(self, conn):
        self.conn = conn
        self.frames = asyncio.Queue()
        self.texts = {}  # payload id -> bytes, BYTES payloads we were told to expect
        self._expected_bytes = set()
        self.cancelled = False
        conn.on_bytes = self._bytes
        self._pump = None

    def _bytes(self, pid, data):
        if pid in self._expected_bytes:
            self.texts[pid] = data
            self._expected_bytes.discard(pid)
            self.frames.put_nowait(("payload", pid))
        else:
            self.frames.put_nowait(("frame", data))

    def start(self):
        self._pump = asyncio.ensure_future(self._run())

    async def _run(self):
        try:
            await self.conn.run()
            self.frames.put_nowait(("closed", None))
        except (ProtocolError, TransportClosed, OSError, ValueError) as e:
            self.frames.put_nowait(("error", e))

    async def next_frame(self, timeout=None):
        """The next Nearby Sharing frame; raises when the link ends."""
        while True:
            kind, value = await asyncio.wait_for(self.frames.get(), timeout)
            if kind == "frame":
                v1 = parse_sharing(value)
                if v1.type == F.CANCEL:
                    raise Cancelled("cancelled by the other device")
                return v1
            if kind == "closed":
                raise TransportClosed("the other device disconnected")
            if kind == "error":
                raise value
            # a text payload arriving: whoever waits on payloads checks .texts

    async def paired_keys(self):
        """Exchanges PairedKeyEncryption/Result, the stranger's way."""
        pke = sharing_pb.PairedKeyEncryptionFrame(secret_id_hash=os.urandom(6), signed_data=os.urandom(72))
        await self.conn.send_bytes(sharing_frame(F.PAIRED_KEY_ENCRYPTION, paired_key_encryption=pke))
        sent_result = got_result = False
        while not (sent_result and got_result):
            v1 = await self.next_frame(30)
            if v1.type == F.PAIRED_KEY_ENCRYPTION and not sent_result:
                res = sharing_pb.PairedKeyResultFrame(status=sharing_pb.PairedKeyResultFrame.UNABLE,
                                                      os_type=sharing_enums.UNKNOWN_OS_TYPE)
                await self.conn.send_bytes(sharing_frame(F.PAIRED_KEY_RESULT, paired_key_result=res))
                sent_result = True
            elif v1.type == F.PAIRED_KEY_RESULT:
                got_result = True
            else:
                raise ProtocolError(f"unexpected message {v1.type} before the introduction")

    async def cancel(self):
        self.cancelled = True
        try:
            await self.conn.send_bytes(sharing_frame(F.CANCEL))
        except (TransportClosed, OSError):
            pass

    async def close(self):
        if self._pump:
            self._pump.cancel()
        await self.conn.close()


class Receive(Session):
    """An incoming share. ask(offer) → True to accept, False to decline.

    offer: {"from", "type", "pin", "files": [{"name", "size", "mime"}],
            "texts": [{"kind", "title"}], "wifi": [ssid…], "total"}
    Returns {"paths": [...], "texts": [{"kind", "text"}], "wifi": [{"ssid", "password"}]}."""

    def __init__(self, conn, folder, ask, progress=None, max_total=None):
        super().__init__(conn)
        self.folder = folder
        self.ask = ask
        self.progress = progress or (lambda done, total: None)
        self.max_total = max_total
        self.total = 0  # bytes of files on offer, once introduced
        self.on_accepted = None  # called once we've said yes (the service upgrades a Bluetooth link then)

    async def run(self):
        self.start()
        await self.paired_keys()
        # (newer phones may send other frames first: skip a few)
        for _ in range(16):
            intro = await self.next_frame(30)
            if intro.type == F.INTRODUCTION:
                break
        else:
            raise ProtocolError("expected an introduction")
        i = intro.introduction
        files = list(i.file_metadata)
        texts = list(i.text_metadata)
        wifi = list(i.wifi_credentials_metadata)
        if not files and not texts and not wifi:
            await self._respond(RESP.UNSUPPORTED_ATTACHMENT_TYPE)
            raise Declined("only shares apps or streams, which iRice can't receive")
        if len(files) + len(texts) + len(wifi) > MAX_ITEMS or any(f.size < 0 for f in files):
            await self._respond(RESP.REJECT)
            raise Declined("too many items")
        total = sum(f.size for f in files)
        self.total = total  # (the service weighs a faster link by it)
        free = shutil.disk_usage(self._existing(self.folder)).free
        if total > free - 64 * 1024 * 1024 or (self.max_total and total > self.max_total):
            await self._respond(RESP.NOT_ENOUGH_SPACE)
            raise Declined("not enough space")
        offer = {"from": self.conn.remote.get("name") or "A nearby device", "type": self.conn.remote.get("type"),
                 "pin": self.conn.pin, "total": total,
                 "files": [{"name": safe_name(f.name), "size": f.size, "mime": f.mime_type} for f in files],
                 "texts": [{"kind": TEXT_KINDS.get(t.type, "text"), "title": t.text_title} for t in texts],
                 "wifi": [w.ssid for w in wifi]}
        try:
            accepted = await asyncio.wait_for(self.ask(offer), DECISION_TIMEOUT)
        except asyncio.TimeoutError:
            await self._respond(RESP.TIMED_OUT)
            raise Declined("no answer in time")
        if not accepted:
            await self._respond(RESP.REJECT)
            raise Declined("declined")

        sinks = {f.payload_id: FileSink(self.folder, f.name, f.size) for f in files}
        try:
            for pid, sink in sinks.items():
                self.conn.expect_file(pid, sink)
            self._expected_bytes = {t.payload_id for t in texts} | {w.payload_id for w in wifi}
            pending = set(sinks) | set(self._expected_bytes)
            done = {}
            self.conn.on_file_progress = lambda pid, d, t: self.progress(sum(s.written for s in sinks.values()), total)
            self.conn.on_file_done = lambda pid: (done.__setitem__(pid, True), self.frames.put_nowait(("payload", pid)))
            self.conn.on_payload_cancelled = lambda pid: self.frames.put_nowait(("error", Cancelled("the sender cancelled")))
            await self._respond(RESP.ACCEPT)
            if self.on_accepted:
                self.on_accepted()
            while pending:
                kind, value = await asyncio.wait_for(self.frames.get(), 120)
                if kind == "payload":
                    pending.discard(value)
                elif kind == "frame":
                    v1 = parse_sharing(value)
                    if v1.type == F.CANCEL:
                        raise Cancelled("cancelled by the other device")
                elif kind == "closed":
                    raise TransportClosed("the other device disconnected before sending everything")
                elif kind == "error":
                    raise value
        except BaseException:
            for s in sinks.values():
                if s.path is None:
                    s.fail()
            raise
        result_texts = []
        for t in texts:
            body = self.texts.get(t.payload_id, b"").decode("utf-8", "replace")
            result_texts.append({"kind": TEXT_KINDS.get(t.type, "text"), "text": body})
        result_wifi = []
        for w in wifi:
            cred = sharing_pb.WifiCredentials()
            try:
                cred.ParseFromString(self.texts.get(w.payload_id, b""))
            except Exception:
                pass
            result_wifi.append({"ssid": w.ssid, "password": cred.password, "hidden": cred.hidden_ssid})
        return {"paths": [s.path for s in sinks.values()], "texts": result_texts, "wifi": result_wifi}

    @staticmethod
    def _existing(path):
        while path and not os.path.exists(path):
            path = os.path.dirname(path)
        return path or "/"

    async def _respond(self, status):
        await self.conn.send_bytes(sharing_frame(F.RESPONSE, connection_response=RESP(status=status)))


class Send(Session):
    """An outgoing share of files. Raises Declined if the receiver says no."""

    def __init__(self, conn, paths, progress=None):
        super().__init__(conn)
        self.paths = paths
        self.progress = progress or (lambda done, total: None)
        self.before_files = None  # awaited after the receiver accepts (a Bluetooth link waits to move)

    async def run(self):
        items = []
        for p in self.paths:
            name = os.path.basename(p)
            mime, kind = file_type(name)
            items.append({"path": p, "name": name, "size": os.stat(p).st_size, "mime": mime, "type": kind,
                          "pid": new_payload_id()})
        total = sum(it["size"] for it in items)
        self.start()
        await self.paired_keys()
        intro = sharing_pb.IntroductionFrame()
        for it in items:
            m = intro.file_metadata.add()
            m.name, m.size, m.mime_type, m.type, m.payload_id = it["name"], it["size"], it["mime"], it["type"], it["pid"]
            m.id = new_payload_id()
        await self.conn.send_bytes(sharing_frame(F.INTRODUCTION, introduction=intro))
        # The receiver may take its time deciding (it's a person)
        while True:
            v1 = await self.next_frame(DECISION_TIMEOUT + 30)
            if v1.type == F.RESPONSE:
                break
        status = v1.connection_response.status
        if status != RESP.ACCEPT:
            reason = {RESP.REJECT: "declined", RESP.NOT_ENOUGH_SPACE: "not enough space on that device",
                      RESP.UNSUPPORTED_ATTACHMENT_TYPE: "that device can't receive these",
                      RESP.TIMED_OUT: "no answer in time"}.get(status, "declined")
            raise Declined(reason)
        if self.before_files:
            await self.before_files()
        sent = 0
        for it in items:
            base = sent
            ok = await self.conn.send_file(it["pid"], it["path"], it["size"], it["name"],
                                           progress=lambda d, s, base=base: self.progress(base + d, total),
                                           cancelled=lambda: self.cancelled)
            if not ok:
                raise Cancelled("cancelled")
            sent += it["size"]
        # The receiver hangs up once it has everything
        try:
            while True:
                await self.next_frame(30)
        except (TransportClosed, asyncio.TimeoutError):
            pass
        return {"paths": self.paths, "total": total}
