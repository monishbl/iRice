"""Quick Share protocol pieces: UKEY2, the encrypted channel, the PIN,
endpoint names, frames. (Transfers end to end: test_quickshare_transfer.py.)"""
import hashlib
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quickshare_peer as peer  # noqa: E402  (puts the engine on the path)

from nearby import endpoint, offline, secure, sharing_pb, ukey, ukey2  # noqa: E402
from nearby.files import free_name, safe_name  # noqa: E402


class Ukey2(unittest.TestCase):
    def handshake(self):
        c, s = ukey2.Client(), ukey2.Server()
        s.finish(c.finish(s.init(c.client_init())))
        return c, s

    def test_both_sides_derive_the_same_keys_and_pin(self):
        c, s = self.handshake()
        self.assertEqual(c.keys.auth_string, s.keys.auth_string)
        self.assertEqual(c.keys.next_secret, s.keys.next_secret)
        self.assertRegex(c.keys.pin, r"^\d{4}$")
        self.assertEqual(c.keys.pin, s.keys.pin)

    def test_keys_differ_between_handshakes(self):
        a, _ = self.handshake()
        b, _ = self.handshake()
        self.assertNotEqual(a.keys.next_secret, b.keys.next_secret)

    def test_a_client_that_breaks_its_commitment_is_refused(self):
        c, s = ukey2.Client(), ukey2.Server()
        server_init = s.init(c.client_init())
        other = ukey2.Client()
        other.finish(server_init)  # a ClientFinished with a different key
        with self.assertRaises(ukey2.HandshakeError):
            s.finish(other._finish)

    def test_unsupported_offers_are_refused(self):
        init = ukey.Ukey2ClientInit(version=1, random=b"\x00" * 32, next_protocol="SOMETHING_ELSE")
        c = init.cipher_commitments.add()
        c.handshake_cipher, c.commitment = ukey.P256_SHA512, b"x" * 64
        raw = ukey.Ukey2Message(message_type=ukey.Ukey2Message.CLIENT_INIT, message_data=init.SerializeToString()).SerializeToString()
        with self.assertRaises(ukey2.HandshakeError):
            ukey2.Server().init(raw)
        with self.assertRaises(ukey2.HandshakeError):
            ukey2.Server().init(b"\xff\xff garbage")

    def test_public_keys_round_trip_in_java_form(self):
        from cryptography.hazmat.primitives.asymmetric import ec
        for _ in range(20):  # some coordinates have the top bit set: sign byte
            k = ec.generate_private_key(ec.SECP256R1()).public_key()
            raw = ukey2.encode_public_key(k)
            self.assertEqual(ukey2.decode_public_key(raw).public_numbers(), k.public_numbers())
            self.assertEqual(peer.peer_key(raw).public_numbers(), k.public_numbers())

    def test_pin_matches_the_java_arithmetic(self):
        for auth in (bytes(range(32)), bytes(range(255, 223, -1)), b"\x80" * 32, hashlib.sha256(b"x").digest()):
            self.assertEqual(ukey2.pin_from_auth(auth), peer.pin(auth))

    def test_salts_are_the_published_constants(self):
        self.assertEqual(secure.D2D_SALT.hex(), "82aa55a0d397f88346ca1cee8d3909b95f13fa7deb1d4ab38376b8256da85510")
        self.assertEqual(secure.SECUREMESSAGE_SALT.hex(), "bf9d2a53c63616d75db0a7165b91c1ef73e537f2427405fa23610a4be657642e")


class Channel(unittest.TestCase):
    def setUp(self):
        c, s = ukey2.Client(), ukey2.Server()
        s.finish(c.finish(s.init(c.client_init())))
        self.client = secure.SecureChannel(c.keys.next_secret, is_client=True)
        self.server = secure.SecureChannel(s.keys.next_secret, is_client=False)

    def test_messages_round_trip_both_ways(self):
        for n in (0, 1, 15, 16, 17, 70000):
            msg = bytes(range(256)) * (n // 256) + bytes(n % 256)
            self.assertEqual(self.server.decrypt(self.client.encrypt(msg)), msg)
            self.assertEqual(self.client.decrypt(self.server.encrypt(msg)), msg)

    def test_tampering_is_caught(self):
        sm = ukey2.securemessage.SecureMessage()
        sm.ParseFromString(self.client.encrypt(b"hello"))
        body = bytearray(sm.header_and_body)
        body[-1] ^= 1
        sm.header_and_body = bytes(body)
        with self.assertRaises(secure.ChannelError):
            self.server.decrypt(sm.SerializeToString())

    def test_replays_and_reordering_are_caught(self):
        first, second = self.client.encrypt(b"1"), self.client.encrypt(b"2")
        with self.assertRaises(secure.ChannelError):
            self.server.decrypt(second)  # skipped one
        fresh_c, fresh_s = Channel._pair()
        m = fresh_c.encrypt(b"x")
        fresh_s.decrypt(m)
        with self.assertRaises(secure.ChannelError):
            fresh_s.decrypt(m)  # the same one twice

    def test_a_direction_cant_decrypt_its_own_messages(self):
        with self.assertRaises(secure.ChannelError):
            self.client.decrypt(self.client.encrypt(b"mine"))

    @staticmethod
    def _pair():
        c, s = ukey2.Client(), ukey2.Server()
        s.finish(c.finish(s.init(c.client_init())))
        return secure.SecureChannel(c.keys.next_secret, True), secure.SecureChannel(s.keys.next_secret, False)


class Endpoint(unittest.TestCase):
    def test_service_type_is_nearby_sharings(self):
        self.assertEqual(endpoint.MDNS_TYPE, "_FC9F5ED42C8A._tcp")
        self.assertEqual(endpoint.SERVICE_HASH, bytes.fromhex("fc9f5e"))

    def test_service_name_layout(self):
        name = endpoint.service_name("AB12")
        raw = endpoint.unb64(name)
        self.assertEqual(raw, b"\x23AB12\xfc\x9f\x5e\x00\x00")
        self.assertNotIn("=", name)
        self.assertEqual(endpoint.parse_service_name(name), "AB12")
        self.assertIsNone(endpoint.parse_service_name(endpoint.b64(b"\x23AB12\x00\x00\x00\x00\x00")))  # other service
        self.assertIsNone(endpoint.parse_service_name("not base64 !!"))

    def test_endpoint_info_layout(self):
        info = endpoint.endpoint_info("Monish’s laptop", endpoint.LAPTOP)
        self.assertEqual(info[0], 3 << 1)  # visible, laptop
        self.assertEqual(info[17], len("Monish’s laptop".encode()))
        self.assertEqual(endpoint.parse_endpoint_info(info), {"type": "laptop", "hidden": False, "name": "Monish’s laptop"})
        hidden = bytes([0x10 | (1 << 1)]) + bytes(16)
        self.assertEqual(endpoint.parse_endpoint_info(hidden), {"type": "phone", "hidden": True, "name": None})
        self.assertIsNone(endpoint.parse_endpoint_info(b"short"))

    def test_names_are_clipped_and_cleaned(self):
        info = endpoint.endpoint_info("é" * 300)
        self.assertLessEqual(info[17], 255)
        endpoint.parse_endpoint_info(info)["name"].encode()  # still valid UTF-8
        evil = bytes([2]) + bytes(16) + bytes([8]) + b"a\x1b[2Jb\nc"
        self.assertEqual(endpoint.parse_endpoint_info(evil)["name"], "a[2Jbc")


class Frames(unittest.TestCase):
    def test_offline_and_sharing_frames_round_trip(self):
        f = offline.OfflineFrame(version=offline.OfflineFrame.V1)
        f.v1.type = offline.V1Frame.CONNECTION_REQUEST
        f.v1.connection_request.endpoint_id = "AB12"
        f.v1.connection_request.mediums.append(offline.ConnectionRequestFrame.WIFI_LAN)
        g = offline.OfflineFrame()
        g.ParseFromString(f.SerializeToString())
        self.assertEqual(g, f)
        s = sharing_pb.Frame(version=sharing_pb.Frame.V1)
        s.v1.type = sharing_pb.V1Frame.INTRODUCTION
        m = s.v1.introduction.file_metadata.add()
        m.name, m.size, m.payload_id = "a.jpg", 10, 1 << 60
        t = sharing_pb.Frame()
        t.ParseFromString(s.SerializeToString())
        self.assertEqual(t.v1.introduction.file_metadata[0].payload_id, 1 << 60)


class FileNames(unittest.TestCase):
    def test_untrusted_names_stay_in_the_folder(self):
        for evil, want in (("../../.bashrc", "bashrc"), ("/etc/passwd", "passwd"), ("..", "file"), (".", "file"),
                           ("a\\..\\b.txt", "b.txt"), ("x\x00y\n.txt", "xy.txt"), ("", "file"), ("   ", "file")):
            self.assertEqual(safe_name(evil), want, evil)

    def test_long_names_keep_their_extension(self):
        n = safe_name("a" * 400 + ".jpeg")
        self.assertTrue(n.endswith(".jpeg"))
        self.assertLessEqual(len(n.encode()), 200)

    def test_free_names(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(free_name(d, "a.txt"), "a.txt")
            Path(d, "a.txt").touch()
            Path(d, "a (2).txt").touch()
            self.assertEqual(free_name(d, "a.txt"), "a (3).txt")
            Path(d, "noext").touch()
            self.assertEqual(free_name(d, "noext"), "noext (2)")


if __name__ == "__main__":
    unittest.main()
