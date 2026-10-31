#!/usr/bin/env python3
"""Chromecast and Google TV (services/Cast.qml): Google Cast, spoken here
directly (CASTV2: length-prefixed protobuf messages over TLS on port 8009),
so nothing extra needs installing.

  cast.py discover [SECONDS]   JSON: [{"id", "name", "model", "host", "port"}]
                               (Avahi: _googlecast._tcp)
  cast.py session              a cast, controlled on stdin, one JSON command
                               per line, reporting on stdout, one JSON status
                               per line:
      {"cmd": "cast", "host", "port", "name", "url" | "file", "title",
       "start": seconds}       start playing on the device
      {"cmd": "play"} {"cmd": "pause"} {"cmd": "seek", "t": seconds}
      {"cmd": "volume", "v": 0..1} {"cmd": "stop"} (ends it)
    status: {"state": "connecting" | "loading" | "buffering" | "playing" |
             "paused" | "idle" | "ended" | "error", "time", "duration",
             "volume", "error"}

A local file is served to the device from this computer, over the local
network only, for as long as the cast lasts: one file, at a random path, on
port 53320 (Settings -> Sharing -> Allow through firewall opens it). A web
page (a YouTube video) goes through yt-dlp, when it's installed, to the video
itself; a direct media link goes as it is.
"""
import http.server
import json
import mimetypes
import os
import re
import secrets
import select
import shutil
import socket
import ssl
import struct
import subprocess
import sys
import threading
import time
import urllib.parse

PORT = 8009
FILE_PORT = 53320
MEDIA_APP = "CC1AD845"  # Google's Default Media Receiver
NS_CONN = "urn:x-cast:com.google.cast.tp.connection"
NS_HEART = "urn:x-cast:com.google.cast.tp.heartbeat"
NS_RECV = "urn:x-cast:com.google.cast.receiver"
NS_MEDIA = "urn:x-cast:com.google.cast.media"
SENDER = "sender-irice"
MEDIA_EXT = re.compile(r"\.(mp4|m4v|webm|mkv|mov|mp3|m4a|aac|ogg|oga|opus|flac|wav|jpe?g|png|gif|webp|m3u8|mpd)(\?|$)", re.I)


# ── CastMessage (protobuf, by hand: six fields) ─────────────────────────
def _varint(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        out.append(b | (0x80 if n else 0))
        if not n:
            return bytes(out)


def _field_str(num, s):
    data = s.encode()
    return _varint(num << 3 | 2) + _varint(len(data)) + data


def encode(source, dest, namespace, payload):
    """A CastMessage with a JSON (string) payload, length-prefixed."""
    body = (_varint(1 << 3 | 0) + _varint(0)  # protocol_version CASTV2_1_0
            + _field_str(2, source) + _field_str(3, dest) + _field_str(4, namespace)
            + _varint(5 << 3 | 0) + _varint(0)  # payload_type STRING
            + _field_str(6, json.dumps(payload, separators=(",", ":"))))
    return struct.pack(">I", len(body)) + body


def decode(body):
    """A CastMessage's fields: {"source", "dest", "namespace", "payload"}."""
    i, out = 0, {}
    names = {2: "source", 3: "dest", 4: "namespace", 6: "payload"}
    while i < len(body):
        key, i = _read_varint(body, i)
        num, wire = key >> 3, key & 7
        if wire == 0:
            _, i = _read_varint(body, i)
        elif wire == 2:
            n, i = _read_varint(body, i)
            if num in names:
                out[names[num]] = body[i:i + n].decode(errors="replace")
            i += n
        else:
            raise ValueError("unexpected wire type")
    return out


def _read_varint(b, i):
    shift = n = 0
    while True:
        c = b[i]
        i += 1
        n |= (c & 0x7F) << shift
        if not c & 0x80:
            return n, i
        shift += 7


# ── Finding devices ─────────────────────────────────────────────────────
def parse_avahi(text):
    """avahi-browse -p -r output: the resolved Cast devices, IPv4 first."""
    found = {}
    for line in text.splitlines():
        f = line.split(";")
        if len(f) < 10 or f[0] != "=":
            continue
        txt = dict(re.findall(r'"([^=]+)=([^"]*)"', f[9]))
        dev_id = txt.get("id") or f[3]
        dev = {"id": dev_id, "name": txt.get("fn") or f[3], "model": txt.get("md", ""),
               "host": f[7], "port": int(f[8] or PORT)}
        if dev_id not in found or (":" in found[dev_id]["host"] and ":" not in dev["host"]):
            found[dev_id] = dev
    return sorted(found.values(), key=lambda d: d["name"].lower())


def discover(seconds=3):
    if not shutil.which("avahi-browse"):
        return []
    try:
        r = subprocess.run(["avahi-browse", "-p", "-r", "-t", "_googlecast._tcp"],
                           capture_output=True, text=True, timeout=seconds + 5)
    except subprocess.TimeoutExpired as e:
        return parse_avahi(e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or ""))
    return parse_avahi(r.stdout)


# ── What the device plays ───────────────────────────────────────────────
def local_ip(host):
    """This computer's address on the way to the device."""
    with socket.socket(socket.AF_INET6 if ":" in host else socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.connect((host, 9))
        return s.getsockname()[0]


class OneFile(http.server.ThreadingHTTPServer):
    """Serves one file, at /<token>/<name>, with byte ranges (seeking)."""
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, path, port=FILE_PORT, bind=""):
        self.path_ = path
        self.token = secrets.token_urlsafe(16)
        self.route = "/" + self.token + "/" + urllib.parse.quote(os.path.basename(path))
        super().__init__((bind, port), _FileHandler)


class _FileHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_HEAD(self):
        self._send(head=True)

    def do_GET(self):
        self._send(head=False)

    def _send(self, head):
        srv = self.server
        if urllib.parse.unquote(self.path.split("?")[0]) != urllib.parse.unquote(srv.route):
            self.send_error(404)
            return
        size = os.path.getsize(srv.path_)
        start, end = 0, size - 1
        m = re.match(r"bytes=(\d*)-(\d*)", self.headers.get("Range", ""))
        if m and (m.group(1) or m.group(2)):
            if m.group(1):
                start = int(m.group(1))
                end = int(m.group(2)) if m.group(2) else size - 1
            else:
                start = max(0, size - int(m.group(2)))
            if start >= size:
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return
            end = min(end, size - 1)
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        else:
            self.send_response(200)
        self.send_header("Content-Type", mimetypes.guess_type(srv.path_)[0] or "application/octet-stream")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(end - start + 1))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        if head:
            return
        with open(srv.path_, "rb") as f:
            f.seek(start)
            left = end - start + 1
            try:
                while left > 0:
                    chunk = f.read(min(1 << 16, left))
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    left -= len(chunk)
            except (BrokenPipeError, ConnectionResetError):
                pass


def resolve(url, run=subprocess.run):
    """(media URL, content type) for a link: as it is when it's media, else
    through yt-dlp (a video page)."""
    if MEDIA_EXT.search(urllib.parse.urlparse(url).path):
        return url, mimetypes.guess_type(urllib.parse.urlparse(url).path)[0] or "video/mp4"
    if not shutil.which("yt-dlp") and run is subprocess.run:
        raise CastError("Casting web videos needs yt-dlp. Install it, then try again.")
    r = run(["yt-dlp", "-g", "--no-playlist", "-f", "best[ext=mp4][protocol^=http]/best[protocol^=http]/best", url],
            capture_output=True, text=True, timeout=60)
    media = (r.stdout or "").strip().splitlines()
    if r.returncode != 0 or not media:
        raise CastError("Couldn't find a video in that page to cast.")
    return media[0], "video/mp4"


class CastError(Exception):
    pass


# ── A cast session ──────────────────────────────────────────────────────
class Session:
    def __init__(self, emit, connect=None):
        self.emit = emit
        self._connect = connect or self._tls
        self.sock = None
        self.buf = b""
        self.req = 0
        self.transport = None
        self.session_id = None
        self.media_session = None
        self.server = None
        self.status = {"state": "idle", "time": 0, "duration": 0, "volume": 1, "error": ""}
        self.last_ping = 0
        self.lock = threading.Lock()

    @staticmethod
    def _tls(host, port):
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE  # (Cast devices use their own self-signed certificate)
        raw = socket.create_connection((host, port), timeout=10)
        return ctx.wrap_socket(raw, server_hostname=host)

    def send(self, dest, ns, payload):
        with self.lock:
            self.sock.sendall(encode(SENDER, dest, ns, payload))

    def request(self, dest, ns, payload):
        self.req += 1
        payload = dict(payload, requestId=self.req)
        self.send(dest, ns, payload)
        return self.req

    def report(self, **kw):
        changed = False
        for k, v in kw.items():
            if self.status.get(k) != v:
                self.status[k] = v
                changed = True
        if changed:
            self.emit(dict(self.status))

    def read(self, timeout):
        """The next message, or None after `timeout` seconds."""
        end = time.monotonic() + timeout
        while True:
            if len(self.buf) >= 4:
                n = struct.unpack(">I", self.buf[:4])[0]
                if len(self.buf) >= 4 + n:
                    body, self.buf = self.buf[4:4 + n], self.buf[4 + n:]
                    msg = decode(body)
                    try:
                        msg["data"] = json.loads(msg.get("payload") or "{}")
                    except ValueError:
                        msg["data"] = {}
                    return msg
            left = max(0.0, end - time.monotonic())
            if not (self.sock.pending() if hasattr(self.sock, "pending") else 0):
                r, _, _ = select.select([self.sock], [], [], left)
                if not r:
                    return None
            chunk = self.sock.recv(65536)
            if not chunk:
                raise CastError("The device closed the connection.")
            self.buf += chunk

    def handle(self, msg):
        ns, data = msg.get("namespace"), msg["data"]
        t = data.get("type")
        if ns == NS_HEART and t == "PING":
            self.send(msg.get("source", "receiver-0"), NS_HEART, {"type": "PONG"})
        elif ns == NS_CONN and t == "CLOSE" and msg.get("source") == self.transport:
            self.report(state="ended")
            raise SystemExit(0)
        elif ns == NS_RECV and t == "RECEIVER_STATUS":
            st = data.get("status", {})
            vol = st.get("volume", {})
            if "level" in vol:
                self.report(volume=round(float(vol["level"]), 3))
            apps = [a for a in st.get("applications", []) if a.get("appId") == MEDIA_APP]
            if apps:
                self.transport = apps[0].get("transportId")
                self.session_id = apps[0].get("sessionId")
            elif self.transport and "applications" in st:
                # (another app took over the TV, or it was stopped there)
                self.report(state="ended")
                raise SystemExit(0)
        elif ns == NS_MEDIA and t == "MEDIA_STATUS":
            for s in data.get("status", []):
                self.media_session = s.get("mediaSessionId", self.media_session)
                state = {"PLAYING": "playing", "PAUSED": "paused", "BUFFERING": "buffering",
                         "IDLE": "idle"}.get(s.get("playerState"), self.status["state"])
                if s.get("playerState") == "IDLE" and s.get("idleReason") == "FINISHED":
                    state = "ended"
                elif s.get("playerState") == "IDLE" and s.get("idleReason") == "ERROR":
                    self.report(state="error", error="The device couldn't play it. "
                                + ("Check that Settings -> Sharing -> Allow through firewall has been used." if self.server else ""))
                    continue
                dur = (s.get("media") or {}).get("duration")
                self.report(state=state, time=round(float(s.get("currentTime", 0)), 1),
                            **({"duration": round(float(dur), 1)} if dur else {}))
        elif ns == NS_MEDIA and t in ("LOAD_FAILED", "LOAD_CANCELLED", "INVALID_REQUEST"):
            self.report(state="error", error="The device refused it (" + t.lower().replace("_", " ") + ").")

    def wait_for(self, pred, timeout):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            msg = self.read(end - time.monotonic())
            if msg is None:
                break
            self.handle(msg)
            if pred(msg):
                return msg
        return None

    def start(self, cmd):
        host, port = cmd["host"], int(cmd.get("port") or PORT)
        self.report(state="connecting")
        if cmd.get("file"):
            path = cmd["file"]
            if not os.path.isfile(path):
                raise CastError("That file isn't there any more.")
            self.server = OneFile(path, port=int(cmd.get("filePort") or FILE_PORT))
            threading.Thread(target=self.server.serve_forever, daemon=True).start()
            ip = cmd.get("selfHost") or local_ip(host)
            url = f"http://{'[' + ip + ']' if ':' in ip else ip}:{self.server.server_address[1]}{self.server.route}"
            ctype = mimetypes.guess_type(path)[0] or "video/mp4"
        else:
            url, ctype = resolve(cmd["url"])
        self.sock = self._connect(host, port)
        self.send("receiver-0", NS_CONN, {"type": "CONNECT"})
        self.request("receiver-0", NS_RECV, {"type": "LAUNCH", "appId": MEDIA_APP})
        if not self.wait_for(lambda m: self.transport is not None, 20):
            raise CastError("The device didn't start its player.")
        self.send(self.transport, NS_CONN, {"type": "CONNECT"})
        self.report(state="loading")
        media = {"contentId": url, "contentUrl": url, "streamType": "BUFFERED", "contentType": ctype,
                 "metadata": {"metadataType": 0, "title": cmd.get("title") or os.path.basename(urllib.parse.urlparse(url).path)}}
        self.request(self.transport, NS_MEDIA, {"type": "LOAD", "sessionId": self.session_id, "media": media,
                                                "autoplay": True, "currentTime": float(cmd.get("start") or 0)})

    def command(self, cmd):
        c = cmd.get("cmd")
        if c == "stop":
            if self.session_id:
                self.request("receiver-0", NS_RECV, {"type": "STOP", "sessionId": self.session_id})
            self.report(state="ended")
            raise SystemExit(0)
        if c == "volume":
            self.request("receiver-0", NS_RECV, {"type": "SET_VOLUME", "volume": {"level": max(0.0, min(1.0, float(cmd.get("v", 1))))}})
            return
        if self.media_session is None:
            return
        base = {"mediaSessionId": self.media_session}
        if c == "play":
            self.request(self.transport, NS_MEDIA, dict(base, type="PLAY"))
        elif c == "pause":
            self.request(self.transport, NS_MEDIA, dict(base, type="PAUSE"))
        elif c == "seek":
            self.request(self.transport, NS_MEDIA, dict(base, type="SEEK", currentTime=float(cmd.get("t", 0))))

    def run(self, fd):
        """Commands from file descriptor `fd` (JSON lines), until stopped."""
        pending = b""

        def lines():
            nonlocal pending
            chunk = os.read(fd, 65536)
            if not chunk:
                return None
            pending += chunk
            out = []
            while b"\n" in pending:
                line, pending = pending.split(b"\n", 1)
                if line.strip():
                    out.append(line.decode(errors="replace"))
            return out
        try:
            first = None
            while first is None:
                got = lines()
                if got is None:
                    raise CastError("Nothing to cast.")
                if got:
                    first = json.loads(got[0])
                    rest = got[1:]
            if first.get("cmd") != "cast":
                raise CastError("Nothing to cast.")
            self.start(first)
            for line in rest:
                self.command(json.loads(line))
            last_status = 0
            while True:
                r, _, _ = select.select([fd, self.sock], [], [], 1.0)
                if fd in r:
                    got = lines()
                    if got is None:
                        self.command({"cmd": "stop"})
                    for line in got:
                        try:
                            self.command(json.loads(line))
                        except ValueError:
                            pass
                # every message there is, buffered or waiting
                while True:
                    msg = self.read(0 if self.sock not in r else 0.2)
                    r = []
                    if not msg:
                        break
                    self.handle(msg)
                now = time.monotonic()
                if now - self.last_ping > 5:
                    self.send("receiver-0", NS_HEART, {"type": "PING"})
                    self.last_ping = now
                if self.transport and self.media_session is not None and now - last_status > 1:
                    self.request(self.transport, NS_MEDIA, {"type": "GET_STATUS", "mediaSessionId": self.media_session})
                    last_status = now
        except SystemExit:
            return 0
        except CastError as e:
            self.report(state="error", error=str(e))
            return 1
        except (OSError, ssl.SSLError) as e:
            self.report(state="error", error="Lost the connection to the device." if self.sock else f"Couldn't reach the device ({e.strerror or e}).")
            return 1
        finally:
            if self.server:
                self.server.shutdown()
                self.server.server_close()
            if self.sock:
                try:
                    self.sock.close()
                except OSError:
                    pass


def main(argv):
    if argv[:1] == ["discover"]:
        print(json.dumps(discover(float(argv[1]) if len(argv) > 1 else 3)))
        return 0
    if argv[:1] == ["session"]:
        def emit(s):
            sys.stdout.write(json.dumps(s) + "\n")
            sys.stdout.flush()
        return Session(emit).run(sys.stdin.fileno())
    print(json.dumps({"error": "usage: cast.py discover [SECONDS] | session"}))
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
