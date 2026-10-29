"""UKEY2: the key exchange every Quick Share connection starts with.

The device that connects (the client, the sender) and the one it connects to
(the server, the receiver) agree on keys over P-256 ECDH. The client first
commits to its public key (a SHA-512 of its future ClientFinished message),
so it can't pick its key after seeing the server's. Both then derive:
  auth string   shown as the 4-digit PIN on both screens, to compare
  next secret   the root of the encrypted channel's keys (secure.py)
Messages and constants follow google/ukey2 (ukey.proto, securemessage.proto).
"""
import hashlib
import hmac
import os

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

from . import securemessage, ukey

VERSION = 1
NEXT_PROTOCOL = "AES_256_CBC-HMAC_SHA256"
P256_SHA512 = ukey.P256_SHA512


class HandshakeError(Exception):
    """The other side broke the protocol (the connection can't go on)."""


def hkdf(ikm, salt, info, length=32):
    return HKDF(algorithm=hashes.SHA256(), length=length, salt=salt, info=info).derive(ikm)


# ── Public keys, as GenericPublicKey ─────────────────────────────────────
# Coordinates are big-endian two's complement (Java's BigInteger), so a
# coordinate with its top bit set gets a leading zero byte. Reading accepts
# either form.
def _twos(n):
    b = n.to_bytes(32, "big")
    return b"\x00" + b if b[0] & 0x80 else b


def encode_public_key(public):
    nums = public.public_numbers()
    key = securemessage.GenericPublicKey(type=securemessage.EC_P256)
    key.ec_p256_public_key.x = _twos(nums.x)
    key.ec_p256_public_key.y = _twos(nums.y)
    return key.SerializeToString()


def decode_public_key(raw):
    key = securemessage.GenericPublicKey()
    try:
        key.ParseFromString(raw)
    except Exception as e:
        raise HandshakeError("unreadable public key") from e
    if key.type != securemessage.EC_P256 or not key.HasField("ec_p256_public_key"):
        raise HandshakeError("not a P-256 public key")
    x = int.from_bytes(key.ec_p256_public_key.x, "big")
    y = int.from_bytes(key.ec_p256_public_key.y, "big")
    try:
        return ec.EllipticCurvePublicNumbers(x, y, ec.SECP256R1()).public_key()
    except ValueError as e:  # not on the curve
        raise HandshakeError("invalid public key") from e


def _message(kind, body):
    return ukey.Ukey2Message(message_type=kind, message_data=body.SerializeToString()).SerializeToString()


def _parse(raw, kind, cls):
    msg = ukey.Ukey2Message()
    try:
        msg.ParseFromString(raw)
    except Exception as e:
        raise HandshakeError("unreadable UKEY2 message") from e
    if msg.message_type != kind:
        raise HandshakeError(f"expected UKEY2 message {kind}, got {msg.message_type}")
    body = cls()
    try:
        body.ParseFromString(msg.message_data)
    except Exception as e:
        raise HandshakeError("unreadable UKEY2 message body") from e
    return body


class Keys:
    """What a finished handshake gives: the PIN material and the channel root."""

    def __init__(self, shared, client_init, server_init):
        # DHS is SHA-256 of the ECDH secret; M1 || M2 (the two init messages
        # exactly as sent) ties the keys to this handshake
        dhs = hashlib.sha256(shared).digest()
        info = client_init + server_init
        self.auth_string = hkdf(dhs, b"UKEY2 v1 auth", info)
        self.next_secret = hkdf(dhs, b"UKEY2 v1 next", info)

    @property
    def pin(self):
        return pin_from_auth(self.auth_string)


def pin_from_auth(auth):
    """The 4-digit code both devices show, as Nearby Sharing computes it.

    Bytes are signed, and the arithmetic is Java's (remainders truncate
    toward zero), so -7 % 9973 is -7 here, not 9966."""
    def rem(a, m):
        r = abs(a) % m
        return -r if a < 0 else r
    h, mult = 0, 1
    for b in auth:
        s = b - 256 if b > 127 else b
        h = rem(h + s * mult, 9973)
        mult = rem(mult * 31, 9973)
    return f"{abs(h):04d}"


class Client:
    """The connecting side: client_init(), then finish(server_init)."""

    def __init__(self):
        self._key = ec.generate_private_key(ec.SECP256R1())
        finished = ukey.Ukey2ClientFinished(public_key=encode_public_key(self._key.public_key()))
        self._finish = _message(ukey.Ukey2Message.CLIENT_FINISH, finished)
        init = ukey.Ukey2ClientInit(version=VERSION, random=os.urandom(32), next_protocol=NEXT_PROTOCOL)
        c = init.cipher_commitments.add()
        c.handshake_cipher = P256_SHA512
        c.commitment = hashlib.sha512(self._finish).digest()
        self._init = _message(ukey.Ukey2Message.CLIENT_INIT, init)
        self.keys = None

    def client_init(self):
        return self._init

    def finish(self, server_init_raw):
        """Reads ServerInit; returns ClientFinished to send. Sets .keys."""
        server = _parse(server_init_raw, ukey.Ukey2Message.SERVER_INIT, ukey.Ukey2ServerInit)
        if server.version != VERSION:
            raise HandshakeError(f"unsupported UKEY2 version {server.version}")
        if server.handshake_cipher != P256_SHA512:
            raise HandshakeError("the other device chose a cipher we didn't offer")
        if len(server.random) != 32:
            raise HandshakeError("bad random in ServerInit")
        peer = decode_public_key(server.public_key)
        shared = self._key.exchange(ec.ECDH(), peer)
        self.keys = Keys(shared, self._init, server_init_raw)
        return self._finish


class Server:
    """The side connected to: init(client_init) → ServerInit, then finish()."""

    def __init__(self):
        self._key = ec.generate_private_key(ec.SECP256R1())
        self._client_init = None
        self._server_init = None
        self._commitment = None
        self.keys = None

    def init(self, client_init_raw):
        client = _parse(client_init_raw, ukey.Ukey2Message.CLIENT_INIT, ukey.Ukey2ClientInit)
        if client.version != VERSION:
            raise HandshakeError(f"unsupported UKEY2 version {client.version}")
        if len(client.random) != 32:
            raise HandshakeError("bad random in ClientInit")
        if client.next_protocol != NEXT_PROTOCOL:
            raise HandshakeError(f"unsupported next protocol {client.next_protocol!r}")
        commitment = next((c.commitment for c in client.cipher_commitments if c.handshake_cipher == P256_SHA512), None)
        if commitment is None:
            raise HandshakeError("the other device offers no cipher we support")
        self._client_init = client_init_raw
        self._commitment = commitment
        server = ukey.Ukey2ServerInit(version=VERSION, random=os.urandom(32), handshake_cipher=P256_SHA512,
                                      public_key=encode_public_key(self._key.public_key()))
        self._server_init = _message(ukey.Ukey2Message.SERVER_INIT, server)
        return self._server_init

    def finish(self, client_finish_raw):
        """Checks ClientFinished against the commitment; sets .keys."""
        if self._server_init is None:
            raise HandshakeError("ClientFinished before ClientInit")
        if not hmac.compare_digest(hashlib.sha512(client_finish_raw).digest(), self._commitment):
            raise HandshakeError("ClientFinished doesn't match the client's commitment")
        finished = _parse(client_finish_raw, ukey.Ukey2Message.CLIENT_FINISH, ukey.Ukey2ClientFinished)
        peer = decode_public_key(finished.public_key)
        shared = self._key.exchange(ec.ECDH(), peer)
        self.keys = Keys(shared, self._client_init, self._server_init)
