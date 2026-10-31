"""cast.py: Google Cast spoken by hand. The messages (protobuf by hand), the
devices Avahi finds, links to media, and a whole cast against a stand-in
Chromecast (TLS, CASTV2): a local file served to it and fetched back with a
byte range, then play, pause, seek, volume and stop."""
import importlib.util
import json
import os
import select
import socket
import ssl
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.request
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/cast.py"
spec = importlib.util.spec_from_file_location("cast", SCRIPT)
cast = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cast)


class Messages(unittest.TestCase):
    def test_round_trip(self):
        raw = cast.encode("sender-0", "receiver-0", cast.NS_RECV, {"type": "LAUNCH", "appId": "CC1AD845", "requestId": 1})
        n = struct.unpack(">I", raw[:4])[0]
        self.assertEqual(n, len(raw) - 4)
        msg = cast.decode(raw[4:])
        self.assertEqual(msg["source"], "sender-0")
        self.assertEqual(msg["dest"], "receiver-0")
        self.assertEqual(msg["namespace"], cast.NS_RECV)
        self.assertEqual(json.loads(msg["payload"])["appId"], "CC1AD845")

    def test_long_payload(self):
        # (lengths over 127 take more than one varint byte)
        raw = cast.encode("a", "b", "c", {"x": "y" * 5000})
        self.assertEqual(json.loads(cast.decode(raw[4:])["payload"])["x"], "y" * 5000)


class Discovery(unittest.TestCase):
    def test_avahi(self):
        out = "\n".join([
            '+;wlan0;IPv4;Google-TV-abc;_googlecast._tcp;local',
            '=;wlan0;IPv6;Google-TV-abc;_googlecast._tcp;local;abc.local;fe80::1;8009;"id=abc" "md=Chromecast" "fn=Living Room TV"',
            '=;wlan0;IPv4;Google-TV-abc;_googlecast._tcp;local;abc.local;192.168.1.20;8009;"id=abc" "md=Chromecast" "fn=Living Room TV"',
            '=;wlan0;IPv4;Nest-def;_googlecast._tcp;local;def.local;192.168.1.21;8009;"id=def" "md=Google Nest Mini" "fn=Kitchen speaker"',
        ])
        devs = cast.parse_avahi(out)
        self.assertEqual([d["name"] for d in devs], ["Kitchen speaker", "Living Room TV"])
        self.assertEqual(devs[1]["host"], "192.168.1.20")  # (IPv4 over IPv6)
        self.assertEqual(devs[1]["model"], "Chromecast")


class Links(unittest.TestCase):
    def test_media_as_is(self):
        self.assertEqual(cast.resolve("https://example.com/a/movie.mp4?x=1")[0], "https://example.com/a/movie.mp4?x=1")

    def test_page_through_ytdlp(self):
        seen = {}

        def run(cmd, **kw):
            seen["cmd"] = cmd
            return subprocess.CompletedProcess(cmd, 0, "https://media.example/v.mp4\n", "")
        url, ctype = cast.resolve("https://www.youtube.com/watch?v=abc", run=run)
        self.assertEqual(url, "https://media.example/v.mp4")
        self.assertEqual(seen["cmd"][0], "yt-dlp")

    def test_no_video(self):
        run = lambda cmd, **kw: subprocess.CompletedProcess(cmd, 1, "", "ERROR")
        with self.assertRaises(cast.CastError):
            cast.resolve("https://example.com/page", run=run)


# ── A stand-in Chromecast ───────────────────────────────────────────────
class FakeCast(threading.Thread):
    def __init__(self, certdir):
        super().__init__(daemon=True)
        self.ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.ctx.load_cert_chain(os.path.join(certdir, "c.pem"), os.path.join(certdir, "k.pem"))
        self.lsock = socket.socket()
        self.lsock.bind(("127.0.0.1", 0))
        self.lsock.listen(1)
        self.port = self.lsock.getsockname()[1]
        self.got = []  # (namespace, payload)
        self.fetched = None
        self.state = "BUFFERING"
        self.time = 0.0
        self.volume = 1.0

    def send(self, conn, src, ns, payload):
        conn.sendall(cast.encode(src, "sender-irice", ns, payload))

    def run(self):
        conn, _ = self.lsock.accept()
        conn = self.ctx.wrap_socket(conn, server_side=True)
        buf = b""
        while True:
            try:
                chunk = conn.recv(65536)
            except OSError:
                return
            if not chunk:
                return
            buf += chunk
            while len(buf) >= 4 and len(buf) >= 4 + struct.unpack(">I", buf[:4])[0]:
                n = struct.unpack(">I", buf[:4])[0]
                msg, buf = cast.decode(buf[4:4 + n]), buf[4 + n:]
                data = json.loads(msg["payload"])
                self.got.append((msg["namespace"], data))
                self.answer(conn, msg, data)

    def media_status(self):
        return {"type": "MEDIA_STATUS", "status": [{"mediaSessionId": 7, "playerState": self.state,
                                                    "currentTime": self.time, "media": {"duration": 100.0}}]}

    def answer(self, conn, msg, data):
        t = data.get("type")
        if t == "LAUNCH" or t == "SET_VOLUME":
            if t == "SET_VOLUME":
                self.volume = data["volume"]["level"]
            self.send(conn, "receiver-0", cast.NS_RECV, {"type": "RECEIVER_STATUS", "requestId": data["requestId"], "status": {
                "volume": {"level": self.volume}, "applications": [{"appId": "CC1AD845", "transportId": "web-1", "sessionId": "s-1"}]}})
        elif t == "LOAD":
            # fetch it as a Chromecast would: the end of the file, by range
            req = urllib.request.Request(data["media"]["contentId"], headers={"Range": "bytes=4-"})
            with urllib.request.urlopen(req, timeout=5) as r:
                self.fetched = (r.status, r.headers.get("Content-Range"), r.read())
            self.state = "PLAYING"
            self.send(conn, "web-1", cast.NS_MEDIA, self.media_status())
        elif t in ("PLAY", "PAUSE", "SEEK", "GET_STATUS") and msg["namespace"] == cast.NS_MEDIA:
            self.state = {"PLAY": "PLAYING", "PAUSE": "PAUSED"}.get(t, self.state)
            if t == "SEEK":
                self.time = data["currentTime"]
            self.send(conn, "web-1", cast.NS_MEDIA, self.media_status())
        elif t == "PING":
            self.send(conn, "receiver-0", cast.NS_HEART, {"type": "PONG"})


class WholeCast(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        r = subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj", "/CN=cast",
                            "-keyout", os.path.join(cls.tmp.name, "k.pem"), "-out", os.path.join(cls.tmp.name, "c.pem")],
                           capture_output=True)
        if r.returncode != 0:
            raise unittest.SkipTest("openssl is needed for the stand-in Chromecast")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def status_until(self, proc, want, timeout=10):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            r, _, _ = select.select([proc.stdout], [], [], max(0, end - time.monotonic()))
            if not r:
                break
            line = proc.stdout.readline()
            if not line:
                break
            st = json.loads(line)
            self.seen.append(st)
            if want(st):
                return st
        self.fail(f"no status like that: {self.seen[-3:]}")

    def test_file(self):
        fake = FakeCast(self.tmp.name)
        fake.start()
        media = os.path.join(self.tmp.name, "Holiday clip.mp4")
        Path(media).write_bytes(b"0123456789abcdef")
        proc = subprocess.Popen([sys.executable, "-I", str(SCRIPT), "session"], stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, text=True, bufsize=1)
        self.seen = []
        try:
            say = lambda o: (proc.stdin.write(json.dumps(o) + "\n"), proc.stdin.flush())
            say({"cmd": "cast", "host": "127.0.0.1", "port": fake.port, "file": media, "title": "Holiday clip",
                 "selfHost": "127.0.0.1", "filePort": 0})
            st = self.status_until(proc, lambda s: s["state"] == "playing")
            self.assertEqual(st["duration"], 100.0)
            # the file went to the device: by range, the bytes from 4 on
            self.assertEqual(fake.fetched, (206, "bytes 4-15/16", b"456789abcdef"))
            load = next(d for ns, d in fake.got if d.get("type") == "LOAD")
            self.assertEqual(load["media"]["metadata"]["title"], "Holiday clip")
            self.assertEqual(load["media"]["contentType"], "video/mp4")
            self.assertIn("/Holiday%20clip.mp4", load["media"]["contentId"])

            say({"cmd": "pause"})
            self.status_until(proc, lambda s: s["state"] == "paused")
            say({"cmd": "seek", "t": 42})
            self.status_until(proc, lambda s: s["time"] == 42.0)
            say({"cmd": "volume", "v": 0.25})
            self.status_until(proc, lambda s: s["volume"] == 0.25)
            say({"cmd": "stop"})
            self.status_until(proc, lambda s: s["state"] == "ended")
            self.assertEqual(proc.wait(timeout=5), 0)
            self.assertTrue(any(d.get("type") == "STOP" and d.get("sessionId") == "s-1" for ns, d in fake.got))
            # (and the file isn't served any more)
            url = load["media"]["contentId"]
            with self.assertRaises(OSError):
                urllib.request.urlopen(url, timeout=2)
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            proc.stdin.close()
            proc.stdout.close()
            fake.lsock.close()

    def test_unreachable(self):
        s = socket.socket()
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.close()  # (nothing listens there)
        proc = subprocess.run([sys.executable, "-I", str(SCRIPT), "session"], capture_output=True, text=True, timeout=20,
                              input=json.dumps({"cmd": "cast", "host": "127.0.0.1", "port": port, "url": "http://x/a.mp4"}) + "\n")
        last = json.loads(proc.stdout.strip().splitlines()[-1])
        self.assertEqual(last["state"], "error")
        self.assertIn("reach", last["error"])
        self.assertEqual(proc.returncode, 1)


if __name__ == "__main__":
    unittest.main()
