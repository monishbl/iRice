"""The encrypted channel after UKEY2 (D2D messages inside SecureMessage).

Each direction has its own keys, derived from UKEY2's next secret: the
client encrypts with the "client" keys and the server with the "server"
ones. A message is a DeviceToDeviceMessage (the payload and a sequence
number counting from 1), encrypted with AES-256-CBC under a fresh IV and
signed with HMAC-SHA256 over header and body. A wrong signature, or a
sequence number out of order (a replayed or dropped message), ends the
connection. Constants follow google/ukey2 (d2d and securemessage).
"""
import hashlib
import hmac
import os

from cryptography.hazmat.primitives import padding
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

from . import d2d, securegcm, securemessage
from .ukey2 import hkdf

D2D_SALT = hashlib.sha256(b"D2D").digest()
SECUREMESSAGE_SALT = hashlib.sha256(b"SecureMessage").digest()


class ChannelError(Exception):
    """A message that doesn't check out: tampered, replayed or garbled."""


class _Keys:
    def __init__(self, d2d_key):
        self.enc = hkdf(d2d_key, SECUREMESSAGE_SALT, b"ENC:2")
        self.mac = hkdf(d2d_key, SECUREMESSAGE_SALT, b"SIG:1")


class SecureChannel:
    """Encrypts what we send and checks and decrypts what we receive."""

    def __init__(self, next_secret, is_client):
        client = _Keys(hkdf(next_secret, D2D_SALT, b"client"))
        server = _Keys(hkdf(next_secret, D2D_SALT, b"server"))
        self._send, self._recv = (client, server) if is_client else (server, client)
        self._sent = 0
        self._received = 0

    def encrypt(self, plain):
        self._sent += 1
        message = d2d.DeviceToDeviceMessage(message=plain, sequence_number=self._sent).SerializeToString()
        iv = os.urandom(16)
        padder = padding.PKCS7(128).padder()
        enc = Cipher(algorithms.AES(self._send.enc), modes.CBC(iv)).encryptor()
        body = enc.update(padder.update(message) + padder.finalize()) + enc.finalize()
        meta = securegcm.GcmMetadata(type=securegcm.DEVICE_TO_DEVICE_MESSAGE, version=1)
        hb = securemessage.HeaderAndBody(body=body)
        hb.header.signature_scheme = securemessage.HMAC_SHA256
        hb.header.encryption_scheme = securemessage.AES_256_CBC
        hb.header.iv = iv
        hb.header.public_metadata = meta.SerializeToString()
        raw = hb.SerializeToString()
        sig = hmac.new(self._send.mac, raw, hashlib.sha256).digest()
        return securemessage.SecureMessage(header_and_body=raw, signature=sig).SerializeToString()

    def decrypt(self, data):
        sm = securemessage.SecureMessage()
        try:
            sm.ParseFromString(data)
        except Exception as e:
            raise ChannelError("unreadable secure message") from e
        expected = hmac.new(self._recv.mac, sm.header_and_body, hashlib.sha256).digest()
        if not hmac.compare_digest(expected, sm.signature):
            raise ChannelError("bad signature")
        hb = securemessage.HeaderAndBody()
        try:
            hb.ParseFromString(sm.header_and_body)
        except Exception as e:
            raise ChannelError("unreadable header") from e
        if hb.header.encryption_scheme != securemessage.AES_256_CBC or len(hb.header.iv) != 16:
            raise ChannelError("unexpected encryption")
        dec = Cipher(algorithms.AES(self._recv.enc), modes.CBC(hb.header.iv)).decryptor()
        unpadder = padding.PKCS7(128).unpadder()
        try:
            plain = unpadder.update(dec.update(hb.body) + dec.finalize()) + unpadder.finalize()
        except ValueError as e:
            raise ChannelError("bad padding") from e
        msg = d2d.DeviceToDeviceMessage()
        try:
            msg.ParseFromString(plain)
        except Exception as e:
            raise ChannelError("unreadable message") from e
        if msg.sequence_number != self._received + 1:
            raise ChannelError(f"message {msg.sequence_number} out of order (expected {self._received + 1})")
        self._received += 1
        return msg.message
