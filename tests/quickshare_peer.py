"""A stand-in for an Android phone, for iRice's Quick Share tests.

It speaks the protocol from the specification side, written directly
against the protobufs and `cryptography` rather than through the engine's
own Connection/Session code, so a mistake on one side doesn't simply cancel
out on the other: UKEY2 (as client and as server), the D2D channel, the
paired-key exchange, introductions and payload chunks.
"""
import asyncio
import hashlib
import hmac
import os
import struct
import sys
from pathlib import Path

from cryptography.hazmat.primitives import hashes, padding
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

ENGINE = Path(__file__).resolve().parent.parent / "shell/assets/scripts/quickshare"
sys.path.insert(0, str(ENGINE))

from nearby import d2d, offline, securegcm, securemessage, sharing_pb, ukey  # noqa: E402

V1 = offline.V1Frame
PT = offline.PayloadTransferFrame
SF = sharing_pb.V1Frame


def kdf(ikm, salt, info):
    return HKDF(algorithm=hashes.SHA256(), length=32, salt=salt, info=info).derive(ikm)


def pubkey(priv):
    n = priv.public_key().public_numbers()
    k = securemessage.GenericPublicKey(type=securemessage.EC_P256)
    # Java BigInteger style, including the sign byte
    for coord, field in ((n.x, "x"), (n.y, "y")):
        b = coord.to_bytes(32, "big")
        setattr(k.ec_p256_public_key, field, b"\x00" + b if b[0] & 0x80 else b)
    return k.SerializeToString()


def peer_key(raw):
    k = securemessage.GenericPublicKey()
    k.ParseFromString(raw)
    x = int.from_bytes(k.ec_p256_public_key.x, "big")
    y = int.from_bytes(k.ec_p256_public_key.y, "big")
    return ec.EllipticCurvePublicNumbers(x, y, ec.SECP256R1()).public_key()


def pin(auth):
    """The spec's 4-digit code, written out the Java way."""
    h, m = 0, 1
    for b in auth:
        s = b - 256 if b >= 128 else b
        v = h + s * m
        h = int(v - 9973 * int(v / 9973))  # Java % truncates toward zero
        v = m * 31
        m = int(v - 9973 * int(v / 9973))
    return "%04d" % abs(h)


class Peer:
    """One end of a connection, Android's way."""

    def __init__(self, reader, writer, name="Pixel 8"):
        self.r, self.w = reader, writer
        self.name = name
        self.sent = self.got = 0
        self.pin = None

    async def read(self):
        (n,) = struct.unpack(">I", await self.r.readexactly(4))
        return await self.r.readexactly(n)

    async def write(self, data):
        self.w.write(struct.pack(">I", len(data)) + data)
        await self.w.drain()

    def _keys(self, shared, m1, m2, is_client):
        dhs = hashlib.sha256(shared).digest()
        self.auth = kdf(dhs, b"UKEY2 v1 auth", m1 + m2)
        nxt = kdf(dhs, b"UKEY2 v1 next", m1 + m2)
        self.pin = pin(self.auth)
        d2d_salt = hashlib.sha256(b"D2D").digest()
        sm_salt = hashlib.sha256(b"SecureMessage").digest()
        mine = kdf(nxt, d2d_salt, b"client" if is_client else b"server")
        theirs = kdf(nxt, d2d_salt, b"server" if is_client else b"client")
        self.enc, self.mac = kdf(mine, sm_salt, b"ENC:2"), kdf(mine, sm_salt, b"SIG:1")
        self.dec, self.vmac = kdf(theirs, sm_salt, b"ENC:2"), kdf(theirs, sm_salt, b"SIG:1")

    # ── Handshakes ───────────────────────────────────────────────────────
    async def connect_as_sender(self, endpoint_id=b"PHON", mediums=(), metadata=None):
        info = bytes([1 << 1]) + os.urandom(16) + bytes([len(self.name.encode())]) + self.name.encode()
        f = offline.OfflineFrame(version=offline.OfflineFrame.V1)
        f.v1.type = V1.CONNECTION_REQUEST
        f.v1.connection_request.endpoint_id = endpoint_id.decode()
        f.v1.connection_request.endpoint_info = info
        f.v1.connection_request.mediums.extend(mediums)  # where it can move to (bandwidth upgrades)
        if metadata is not None:
            f.v1.connection_request.medium_metadata.CopyFrom(metadata)
        await self.write(f.SerializeToString())
        priv = ec.generate_private_key(ec.SECP256R1())
        fin = ukey.Ukey2Message(message_type=ukey.Ukey2Message.CLIENT_FINISH,
                                message_data=ukey.Ukey2ClientFinished(public_key=pubkey(priv)).SerializeToString()).SerializeToString()
        init = ukey.Ukey2ClientInit(version=1, random=os.urandom(32), next_protocol="AES_256_CBC-HMAC_SHA256")
        c = init.cipher_commitments.add()
        c.handshake_cipher = ukey.P256_SHA512
        c.commitment = hashlib.sha512(fin).digest()
        m1 = ukey.Ukey2Message(message_type=ukey.Ukey2Message.CLIENT_INIT, message_data=init.SerializeToString()).SerializeToString()
        await self.write(m1)
        m2 = await self.read()
        si = ukey.Ukey2Message()
        si.ParseFromString(m2)
        assert si.message_type == ukey.Ukey2Message.SERVER_INIT
        sinit = ukey.Ukey2ServerInit()
        sinit.ParseFromString(si.message_data)
        await self.write(fin)
        self._keys(priv.exchange(ec.ECDH(), peer_key(sinit.public_key)), m1, m2, is_client=True)
        await self.write(self._response())
        resp = offline.OfflineFrame()
        resp.ParseFromString(await self.read())
        assert resp.v1.type == V1.CONNECTION_RESPONSE and resp.v1.connection_response.response == offline.ConnectionResponseFrame.ACCEPT

    async def accept_as_receiver(self):
        req = offline.OfflineFrame()
        req.ParseFromString(await self.read())
        assert req.v1.type == V1.CONNECTION_REQUEST
        self.remote_info = req.v1.connection_request.endpoint_info
        self.remote_request = req.v1.connection_request
        m1 = await self.read()
        ci = ukey.Ukey2Message()
        ci.ParseFromString(m1)
        cinit = ukey.Ukey2ClientInit()
        cinit.ParseFromString(ci.message_data)
        priv = ec.generate_private_key(ec.SECP256R1())
        sinit = ukey.Ukey2ServerInit(version=1, random=os.urandom(32), handshake_cipher=ukey.P256_SHA512, public_key=pubkey(priv))
        m2 = ukey.Ukey2Message(message_type=ukey.Ukey2Message.SERVER_INIT, message_data=sinit.SerializeToString()).SerializeToString()
        await self.write(m2)
        fin_raw = await self.read()
        assert hashlib.sha512(fin_raw).digest() == cinit.cipher_commitments[0].commitment
        fin = ukey.Ukey2Message()
        fin.ParseFromString(fin_raw)
        cf = ukey.Ukey2ClientFinished()
        cf.ParseFromString(fin.message_data)
        self._keys(priv.exchange(ec.ECDH(), peer_key(cf.public_key)), m1, m2, is_client=False)
        resp = offline.OfflineFrame()
        resp.ParseFromString(await self.read())
        assert resp.v1.type == V1.CONNECTION_RESPONSE
        await self.write(self._response())

    def _response(self):
        f = offline.OfflineFrame(version=offline.OfflineFrame.V1)
        f.v1.type = V1.CONNECTION_RESPONSE
        f.v1.connection_response.response = offline.ConnectionResponseFrame.ACCEPT
        f.v1.connection_response.os_info.type = offline.OsInfo.ANDROID
        return f.SerializeToString()

    # ── Encrypted frames ─────────────────────────────────────────────────
    async def send(self, frame):
        self.sent += 1
        msg = d2d.DeviceToDeviceMessage(message=frame.SerializeToString(), sequence_number=self.sent).SerializeToString()
        iv = os.urandom(16)
        p = padding.PKCS7(128).padder()
        e = Cipher(algorithms.AES(self.enc), modes.CBC(iv)).encryptor()
        hb = securemessage.HeaderAndBody(body=e.update(p.update(msg) + p.finalize()) + e.finalize())
        hb.header.signature_scheme = securemessage.HMAC_SHA256
        hb.header.encryption_scheme = securemessage.AES_256_CBC
        hb.header.iv = iv
        hb.header.public_metadata = securegcm.GcmMetadata(type=securegcm.DEVICE_TO_DEVICE_MESSAGE, version=1).SerializeToString()
        raw = hb.SerializeToString()
        await self.write(securemessage.SecureMessage(header_and_body=raw, signature=hmac.new(self.mac, raw, hashlib.sha256).digest()).SerializeToString())

    async def recv(self):
        """The next offline frame that isn't a keep-alive."""
        while True:
            sm = securemessage.SecureMessage()
            sm.ParseFromString(await self.read())
            assert hmac.compare_digest(hmac.new(self.vmac, sm.header_and_body, hashlib.sha256).digest(), sm.signature)
            hb = securemessage.HeaderAndBody()
            hb.ParseFromString(sm.header_and_body)
            d = Cipher(algorithms.AES(self.dec), modes.CBC(hb.header.iv)).decryptor()
            u = padding.PKCS7(128).unpadder()
            m = d2d.DeviceToDeviceMessage()
            m.ParseFromString(u.update(d.update(hb.body) + d.finalize()) + u.finalize())
            self.got += 1
            assert m.sequence_number == self.got
            f = offline.OfflineFrame()
            f.ParseFromString(m.message)
            if f.v1.type != V1.KEEP_ALIVE:
                return f

    # ── Payloads and sharing frames ──────────────────────────────────────
    async def chunk(self, pid, kind, total, offset, body, last, name=""):
        f = offline.OfflineFrame(version=offline.OfflineFrame.V1)
        f.v1.type = V1.PAYLOAD_TRANSFER
        p = f.v1.payload_transfer
        p.packet_type = PT.DATA
        p.payload_header.id, p.payload_header.type, p.payload_header.total_size = pid, kind, total
        if name:
            p.payload_header.file_name = name
        p.payload_chunk.offset, p.payload_chunk.body, p.payload_chunk.flags = offset, body, 1 if last else 0
        await self.send(f)

    async def send_bytes(self, data, pid=None):
        pid = pid or int.from_bytes(os.urandom(7), "big")
        await self.chunk(pid, PT.PayloadHeader.BYTES, len(data), 0, data, False)
        await self.chunk(pid, PT.PayloadHeader.BYTES, len(data), len(data), b"", True)

    async def send_sharing(self, kind, **fields):
        f = sharing_pb.Frame(version=sharing_pb.Frame.V1)
        f.v1.type = kind
        for k, v in fields.items():
            getattr(f.v1, k).CopyFrom(v)
        await self.send_bytes(f.SerializeToString())

    async def recv_sharing(self):
        """The next Nearby Sharing frame (assembled from BYTES chunks)."""
        buf = {}
        while True:
            f = await self.recv()
            if f.v1.type == V1.DISCONNECTION:
                return None
            if f.v1.type != V1.PAYLOAD_TRANSFER:
                continue
            p = f.v1.payload_transfer
            if p.payload_header.type != PT.PayloadHeader.BYTES:
                return ("file", p)
            buf.setdefault(p.payload_header.id, b"")
            buf[p.payload_header.id] += p.payload_chunk.body
            if p.payload_chunk.flags & 1:
                s = sharing_pb.Frame()
                s.ParseFromString(buf.pop(p.payload_header.id))
                return s.v1

    async def paired_keys(self):
        await self.send_sharing(SF.PAIRED_KEY_ENCRYPTION, paired_key_encryption=sharing_pb.PairedKeyEncryptionFrame(
            secret_id_hash=os.urandom(6), signed_data=os.urandom(72)))
        got_pke = got_pkr = False
        while not (got_pke and got_pkr):
            v1 = await self.recv_sharing()
            if v1.type == SF.PAIRED_KEY_ENCRYPTION:
                got_pke = True
                await self.send_sharing(SF.PAIRED_KEY_RESULT, paired_key_result=sharing_pb.PairedKeyResultFrame(
                    status=sharing_pb.PairedKeyResultFrame.UNABLE))
            elif v1.type == SF.PAIRED_KEY_RESULT:
                got_pkr = True

    async def disconnect(self):
        f = offline.OfflineFrame(version=offline.OfflineFrame.V1)
        f.v1.type = V1.DISCONNECTION
        await self.send(f)
        self.w.close()


async def open_peer(port, name="Pixel 8"):
    r, w = await asyncio.open_connection("127.0.0.1", port)
    return Peer(r, w, name)
