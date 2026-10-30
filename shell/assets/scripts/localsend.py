#!/usr/bin/env python3
"""LocalSend for iRice: send files to, and receive files from, nearby devices
running LocalSend (phones, other computers), like AirDrop. Speaks LocalSend
protocol v2: multicast discovery on 224.0.0.167:53317 and certificate-pinned HTTPS.

Talks to the shell over stdin/stdout, one JSON object per line.
  in:  {"cmd":"mode","mode":"off|everyone","alias":"..."}
       {"cmd":"scan"}                                  look for devices now
       {"cmd":"send","to":"<fingerprint>","files":["/path", ...]}
       {"cmd":"answer","session":"<id>","accept":true|false}
       {"cmd":"cancel","session":"<id>"}
  out: {"ev":"ready","port":53317,"receiving":true}
       {"ev":"peers","peers":[{"id","alias","type","model"}]}
       {"ev":"incoming","session","from","type","files":[{"name","size"}],"total"}
       {"ev":"progress","session","dir":"in|out","done","total","name"}
       {"ev":"finished","session","dir","paths":[...]}
       {"ev":"declined","session","to"}                 they turned down what we sent
       {"ev":"declined","session","dir":"in","from","expired"}  we didn't take theirs
       {"ev":"failed","session","message"} | {"ev":"error","message"}
Exits when stdin closes (the shell quit). Standard library only.
"""
import hashlib
import http.client
import http.server
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import tempfile
import secrets
import socket
import ssl
import struct
import sys
import threading
import time
import urllib.parse

GROUP, DISCOVERY = "224.0.0.167", 53317
PORT = int(os.environ.get("IRICE_LOCALSEND_PORT", DISCOVERY))  # (another port only for testing two on one machine)
API = "/api/localsend/v2"
FINGERPRINT = hashlib.sha256(secrets.token_bytes(32)).hexdigest()
DOWNLOADS = os.environ.get("XDG_DOWNLOAD_DIR") or os.path.join(os.path.expanduser("~"), "Downloads")

lock = threading.Lock()
state = {"mode": "off", "alias": socket.gethostname(), "server": False}
peers = {}      # fingerprint -> {alias, type, model, ip, port, protocol, seen}
sessions = {}   # incoming session id -> {...}
answers = {}    # session id -> threading.Event, with result in sessions
session_lock = threading.RLock()
peer_lock = threading.RLock()
TLS_CONTEXT = None
MAX_BODY = 1 << 20
MAX_SESSIONS = 8
DISCOVERY_SLOTS = threading.BoundedSemaphore(8)
active_outgoing = set()

cancelled = set()  # outgoing session ids to stop


def emit(ev, **kw):
    kw["ev"] = ev
    with lock:
        sys.stdout.write(json.dumps(kw) + "\n")
        sys.stdout.flush()


def info(announce=None):
    d = {"alias": state["alias"], "version": "2.1", "deviceModel": platform.system(), "deviceType": "desktop",
         "fingerprint": FINGERPRINT, "port": PORT, "protocol": "https", "download": False}
    if announce is not None:
        d["announce"] = announce
    return d


def publish_peers():
    now = time.time()
    with peer_lock:
        for k in [k for k, p in peers.items() if now - p["seen"] > 90]:
            del peers[k]
        # Peers with HTTPS off can't be sent to (connect() refuses them):
        # listing them would only offer a send that always fails
        snapshot = [{"id": k, "alias": p["alias"], "type": p["type"], "model": p["model"]} for k, p in peers.items() if p["protocol"] == "https"]
    emit("peers", peers=snapshot)


def remember(d, ip):
    if not isinstance(d, dict):
        return False
    fp = d.get("fingerprint")
    port = d.get("port", DISCOVERY)
    if (not isinstance(fp, str) or len(fp) > 256 or not fp or fp == FINGERPRINT
            or not isinstance(port, int) or isinstance(port, bool) or not 1 <= port <= 65535
            or d.get("protocol", "https") not in ("http", "https")):
        return False
    with peer_lock:
        if fp not in peers and len(peers) >= 512:
            return False
        new = fp not in peers
        peers[fp] = {"alias": str(d.get("alias", "Device"))[:200], "type": str(d.get("deviceType") or "mobile"),
                     "model": str(d.get("deviceModel") or ""), "fingerprint": fp,
                     "ip": ip, "port": port, "protocol": d.get("protocol") or "https", "seen": time.time()}
        return new


class PinnedHTTPSConnection(http.client.HTTPSConnection):
    def __init__(self, peer, timeout):
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE # self-signed, authenticated by discovery fingerprint below
        super().__init__(peer["ip"], peer["port"], timeout=timeout, context=ctx)
        self.fingerprint = peer.get("fingerprint", "").lower().replace(":", "")

    def connect(self):
        super().connect()
        digest = hashlib.sha256(self.sock.getpeercert(binary_form=True)).hexdigest()
        if not secrets.compare_digest(digest, self.fingerprint):
            self.close()
            raise ssl.SSLError("Peer certificate does not match the discovered fingerprint")


class UnencryptedPeer(ValueError):
    """A peer with HTTPS off; the message says how to fix it."""


def connect(p, timeout=10):
    if p["protocol"] != "https":
        raise UnencryptedPeer("%s has encryption off. Turn on HTTPS in LocalSend's settings on that device." % p.get("alias", "That device"))
    return PinnedHTTPSConnection(p, timeout)


def configure_tls(directory=None):
    global FINGERPRINT, TLS_CONTEXT
    directory = Path(directory or (os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"))) / "irice/localsend"
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(directory, 0o700)
    cert, key = directory / "certificate.pem", directory / "private.pem"
    if not cert.exists() or not key.exists():
        with tempfile.TemporaryDirectory(dir=directory) as tmp:
            c, k = Path(tmp) / 'cert', Path(tmp) / 'key'
            subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '3650',
                            '-subj', '/CN=iRice LocalSend', '-keyout', str(k), '-out', str(c)],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
            os.chmod(k, 0o600); os.chmod(c, 0o600)
            os.replace(k, key); os.replace(c, cert)
    os.chmod(key, 0o600)
    FINGERPRINT = hashlib.sha256(ssl.PEM_cert_to_DER_cert(cert.read_text())).hexdigest()
    TLS_CONTEXT = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    TLS_CONTEXT.minimum_version = ssl.TLSVersion.TLSv1_2
    TLS_CONTEXT.load_cert_chain(cert, key)


def cancel_incoming(sid, message="Cancelled"):
    with session_lock:
        s = sessions.pop(sid, None)
        if not s or s.get("cancelled"):
            return
        s["cancelled"] = True
        if sid in answers:
            answers[sid].set()
    emit("failed", session=sid, message=message)


# ── Discovery ────────────────────────────────────────────────────────────
def multicast_socket():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    if hasattr(socket, "SO_REUSEPORT"):
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEPORT, 1)
    s.bind(("", DISCOVERY))
    s.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, struct.pack("4sl", socket.inet_aton(GROUP), socket.INADDR_ANY))
    s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
    return s


def announce():
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
        s.sendto(json.dumps(info(True)).encode(), (GROUP, DISCOVERY))
        s.close()
    except OSError as e:
        emit("error", message="Couldn't look for devices: %s" % e.strerror)


def reply_to(peer):
    """Answer discovery with bounded concurrent work and response size."""
    c = None
    try:
        if state["mode"] == "off":
            return
        c = connect(peer, timeout=4)
        c.request("POST", API + "/register", json.dumps(info()), {"Content-Type": "application/json"})
        c.getresponse().read(MAX_BODY + 1)
    except Exception:
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP) as sock:
                sock.sendto(json.dumps(info(False)).encode(), (GROUP, DISCOVERY))
        except OSError:
            pass
    finally:
        if c: c.close()
        DISCOVERY_SLOTS.release()


def discovered(data, ip):
    """One discovery packet. Anyone on the network can send one, so nothing in
    it is trusted: only a peer remember() accepted (from this address) is
    answered, using what was stored, not the packet."""
    d = json.loads(data.decode())
    if not isinstance(d, dict):
        return
    fp = d.get("fingerprint")
    if not isinstance(fp, str) or not fp or fp == FINGERPRINT:
        return
    if remember(d, ip):
        publish_peers()
    with peer_lock:
        peer = dict(peers[fp]) if fp in peers and peers[fp]["ip"] == ip else None
    if d.get("announce") is True and peer and DISCOVERY_SLOTS.acquire(blocking=False):
        threading.Thread(target=reply_to, args=(peer,), daemon=True).start()


def listen():
    try:
        s = multicast_socket()
    except OSError as e:
        emit("error", message="Couldn't listen for nearby devices: %s" % e.strerror)
        return
    logged = set()
    while True:
        try:
            data, (ip, _) = s.recvfrom(65536)
        except OSError:
            continue
        # A bad packet is dropped, never the listener (noted once per kind)
        try:
            discovered(data, ip)
        except Exception as e:
            if type(e).__name__ not in logged:
                logged.add(type(e).__name__)
                print("localsend: dropped a discovery packet (%s)" % type(e).__name__, file=sys.stderr)


# ── Receiving ────────────────────────────────────────────────────────────
def unique(path):
    base, ext = os.path.splitext(path)
    n = 1
    while os.path.exists(path):
        path = "%s (%d)%s" % (base, n, ext)
        n += 1
    return path


class Handler(http.server.BaseHTTPRequestHandler):
    server_version = "iRice-LocalSend"

    def setup(self):
        self.request.settimeout(15)
        super().setup()

    def log_message(self, *a):
        pass

    def reply(self, code, body=None):
        raw = json.dumps(body).encode() if body is not None else b""
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n < 0 or n > MAX_BODY or self.headers.get("Transfer-Encoding"):
            raise ValueError("Invalid metadata size")
        data = self.rfile.read(n) if n else b""
        if len(data) != n:
            raise ValueError("Truncated metadata")
        return data

    def do_GET(self):
        if self.path.startswith(API + "/info"):
            return self.reply(200, info())
        self.reply(404)

    def do_POST(self):
        url = urllib.parse.urlparse(self.path)
        q = dict(urllib.parse.parse_qsl(url.query))
        if url.path == API + "/register":
            try:
                if remember(json.loads(self.body()), self.client_address[0]):
                    publish_peers()
            except Exception:
                pass
            return self.reply(200, info())
        if url.path == API + "/prepare-upload":
            return self.prepare()
        if url.path == API + "/upload":
            return self.upload(q)
        if url.path == API + "/cancel":
            cancel_incoming(q.get("sessionId", ""))
            return self.reply(200)
        self.reply(404)

    def prepare(self):
        if state["mode"] == "off":
            return self.reply(403, {"message": "Not receiving"})
        try:
            req = json.loads(self.body())
        except Exception:
            return self.reply(400)
        try:
            sender, files = req["info"], req["files"]
            if not isinstance(sender, dict) or not isinstance(files, dict) or not 1 <= len(files) <= 1000:
                raise ValueError()
            for fid, f in files.items():
                if (not isinstance(f, dict) or not isinstance(f.get("size"), int) or isinstance(f["size"], bool)
                        or not 0 <= f["size"] <= 2**63 - 1 or not isinstance(f.get("fileName"), str)
                        or not f["fileName"] or len(f["fileName"]) > 4096):
                    raise ValueError()
            total = sum(f["size"] for f in files.values())
            os.makedirs(DOWNLOADS, exist_ok=True)
            if total > shutil.disk_usage(DOWNLOADS).free:
                return self.reply(507)
        except (KeyError, TypeError, ValueError, OSError):
            return self.reply(400)
        sid = secrets.token_hex(16)
        with session_lock:
            if len(sessions) >= MAX_SESSIONS:
                return self.reply(429)
            s = {"id": sid, "from": str(sender.get("alias", "Someone"))[:200], "files": files, "tokens": {},
                 "paths": [], "done": 0, "total": total, "accepted": None, "cancelled": False,
                 "claimed": set(), "touched": time.monotonic()}
            sessions[sid] = s
            ev = answers[sid] = threading.Event()
        emit("incoming", session=sid, **{"from": s["from"]}, type=sender.get("deviceType") or "mobile",
             files=[{"name": f["fileName"], "size": f["size"]} for f in files.values()], total=total)
        ev.wait(60)
        with session_lock:
            answers.pop(sid, None)
            if s["cancelled"]:
                return self.reply(403)
            if not s["accepted"]:
                sessions.pop(sid, None)
                # expired: nobody answered in time (else it was declined here)
                emit("declined", session=sid, dir="in", **{"from": s["from"]}, expired=s["accepted"] is None)
                return self.reply(403, {"message": "Declined or timed out"})
            s["tokens"] = {fid: secrets.token_hex(16) for fid in files}
            s["touched"] = time.monotonic()
        self.reply(200, {"sessionId": sid, "files": s["tokens"]})

    def upload(self, q):
        sid, fid = q.get("sessionId", ""), q.get("fileId", "")
        with session_lock:
            s = sessions.get(sid)
            if not s or s["cancelled"] or fid not in s["tokens"] or not secrets.compare_digest(s["tokens"][fid], q.get("token", "")):
                return self.reply(403)
            try:
                length = int(self.headers.get("Content-Length", "-1"))
                if length != s["files"][fid]["size"] or self.headers.get("Transfer-Encoding"):
                    return self.reply(400)
            except ValueError:
                return self.reply(400)
            if fid in s["claimed"]:
                return self.reply(409)
            s["claimed"].add(fid)
        name = os.path.basename(s["files"][fid]["fileName"])
        if name in ("", ".", ".."):
            name = "file"
        path = None
        try:
            base, ext = os.path.splitext(name)
            for n in range(10000):
                candidate = os.path.join(DOWNLOADS, name if n == 0 else "%s (%d)%s" % (base, n, ext))
                try:
                    out = open(candidate, "xb")
                    path = candidate
                    break
                except FileExistsError:
                    continue
            if path is None:
                raise OSError("No free destination name")
            left, last = length, 0
            with out:
                while left:
                    if s["cancelled"]:
                        raise OSError("Cancelled")
                    chunk = self.rfile.read(min(1 << 20, left))
                    if not chunk:
                        raise OSError("Transfer ended before all bytes arrived")
                    # The disk write happens outside the lock: holding it for
                    # each 1 MB write stalled every other upload, cancel and
                    # answer behind this one
                    with session_lock:
                        if s["cancelled"]:
                            raise OSError("Cancelled")
                    out.write(chunk)
                    left -= len(chunk)
                    with session_lock:
                        s["done"] += len(chunk)
                        s["touched"] = time.monotonic()
                        done, total = s["done"], s["total"]
                    if time.monotonic() - last > .3:
                        last = time.monotonic()
                        emit("progress", session=sid, dir="in", done=done, total=total, name=name)
            with session_lock:
                if s["cancelled"]:
                    raise OSError("Cancelled")
                s["paths"].append(path)
                self.reply(200)
                if len(s["paths"]) == len(s["files"]):
                    sessions.pop(sid, None)
                    emit("finished", session=sid, dir="in", paths=s["paths"], **{"from": s["from"]})
        except (OSError, ValueError):
            if path:
                try: os.unlink(path)
                except FileNotFoundError: pass
            cancel_incoming(sid, "Transfer interrupted or could not be saved")
            try: self.reply(500)
            except OSError: pass


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    slots = threading.BoundedSemaphore(16)
    def get_request(self):
        sock, address = super().get_request()
        sock.settimeout(15)
        return TLS_CONTEXT.wrap_socket(sock, server_side=True, do_handshake_on_connect=False), address
    def handle_error(self, request, address):
        # A peer can drop a TLS handshake or HTTP connection at any time.
        # Expected network failures should not fill the shell log with traces.
        if not isinstance(sys.exception(), (OSError, ssl.SSLError)):
            super().handle_error(request, address)
    def process_request(self, request, address):
        if not self.slots.acquire(blocking=False):
            request.close()
            return
        try:
            super().process_request(request, address)
        except BaseException:
            self.slots.release()
            raise
    def process_request_thread(self, request, address):
        try:
            super().process_request_thread(request, address)
        finally:
            self.slots.release()


def expire_sessions():
    while True:
        time.sleep(15)
        with session_lock:
            expired = [sid for sid, s in sessions.items() if time.monotonic() - s["touched"] > 120]
        for sid in expired:
            cancel_incoming(sid, "Transfer timed out")


def serve():
    try:
        srv = Server(("", PORT), Handler)
    except OSError:
        emit("error", message="Port %d is in use (is the LocalSend app open?). You can still send." % PORT)
        return
    state["server"] = True
    emit("ready", port=PORT, receiving=state["mode"] != "off")
    srv.serve_forever()


# ── Sending ──────────────────────────────────────────────────────────────
class Cancelled(Exception):
    """The user cancelled a send (reported as such, not as a failure)."""


def cancel_remote(p, remote):
    """Tell the receiver to drop the session, so it doesn't wait for files."""
    try:
        c = connect(p, 5)
        try:
            c.request("POST", API + "/cancel?" + urllib.parse.urlencode({"sessionId": remote}))
            c.getresponse().read(MAX_BODY)
        finally:
            c.close()
    except Exception:
        pass


def send(to, paths):
    p = peers.get(to)
    sid = "out-" + secrets.token_hex(6)
    if not p:
        return emit("failed", session=sid, message="That device isn't nearby any more")
    files, ids = {}, {}
    for i, path in enumerate(paths):
        try:
            size = os.path.getsize(path)
        except OSError:
            continue
        fid = "f%d" % i
        ids[fid] = path
        files[fid] = {"id": fid, "fileName": os.path.basename(path), "size": size, "fileType": "application/octet-stream"}
    if not files:
        return emit("failed", session=sid, message="Nothing to send")
    total = sum(f["size"] for f in files.values())
    active_outgoing.add(sid)
    c = None
    emit("progress", session=sid, dir="out", done=0, total=total, name=p["alias"], waiting=True)
    try:
        c = connect(p, timeout=90)  # they decide on their side
        c.request("POST", API + "/prepare-upload", json.dumps({"info": info(), "files": files}), {"Content-Type": "application/json"})
        r = c.getresponse()
        body = r.read(MAX_BODY + 1)
        c.close()
        if len(body) > MAX_BODY: raise ValueError("Oversized peer response")
        if sid in cancelled:
            # Cancelled while they were deciding: if they accepted, tell them
            if r.status == 200:
                try:
                    cancel_remote(p, json.loads(body)["sessionId"])
                except (ValueError, KeyError, TypeError):
                    pass
            raise Cancelled()
        if r.status == 204:
            return emit("finished", session=sid, dir="out", paths=[], to=p["alias"])
        if r.status == 403:
            return emit("declined", session=sid, to=p["alias"])
        if r.status != 200:
            return emit("failed", session=sid, message="%s couldn't take the files (%d)" % (p["alias"], r.status))
        resp = json.loads(body)
        remote, tokens = resp["sessionId"], resp["files"]
        if not isinstance(remote, str) or not isinstance(tokens, dict) or not tokens or any(fid not in ids or not isinstance(token, str) for fid, token in tokens.items()):
            raise ValueError("Invalid upload tokens")
        done, last = 0, 0
        for fid, token in tokens.items():
            path = ids[fid]
            c = connect(p, timeout=60)
            qs = urllib.parse.urlencode({"sessionId": remote, "fileId": fid, "token": token})
            c.putrequest("POST", API + "/upload?" + qs)
            c.putheader("Content-Type", "application/octet-stream")
            c.putheader("Content-Length", str(files[fid]["size"]))
            c.endheaders()
            with open(path, "rb") as f:
                left = files[fid]["size"]
                while left:
                    if sid in cancelled:
                        c.close()
                        cancel_remote(p, remote)
                        raise Cancelled()
                    chunk = f.read(min(1 << 20, left))
                    if not chunk:
                        raise OSError("Source file changed during transfer")
                    left -= len(chunk)
                    c.send(chunk)
                    done += len(chunk)
                    if time.time() - last > 0.3:
                        last = time.time()
                        emit("progress", session=sid, dir="out", done=done, total=total, name=p["alias"])
            r = c.getresponse()
            r.read(MAX_BODY + 1)
            c.close()
            if r.status != 200:
                return emit("failed", session=sid, message="%s stopped the transfer" % p["alias"])
        emit("finished", session=sid, dir="out", paths=[ids[fid] for fid in tokens], to=p["alias"])
    except Cancelled:
        emit("failed", session=sid, message="Cancelled")
    except UnencryptedPeer as e:
        emit("failed", session=sid, message=str(e))
    except Exception as e:
        emit("failed", session=sid, message="Couldn't reach %s (%s)" % (p["alias"], e.__class__.__name__))
    finally:
        if c: c.close()
        active_outgoing.discard(sid)
        cancelled.discard(sid)


# ── Commands ─────────────────────────────────────────────────────────────
def main():
    try:
        configure_tls()
    except (OSError, subprocess.SubprocessError, ssl.SSLError):
        emit("error", message="Could not initialize encrypted sharing. Check openssl and private state storage.")
        return
    threading.Thread(target=expire_sessions, daemon=True).start()
    threading.Thread(target=listen, daemon=True).start()
    threading.Thread(target=serve, daemon=True).start()
    for line in sys.stdin:
        try:
            c = json.loads(line)
        except Exception:
            continue
        if not isinstance(c, dict):
            continue
        cmd = c.get("cmd")
        if cmd == "mode":
            state["mode"] = c.get("mode", "off")
            if c.get("alias"):
                state["alias"] = c["alias"]
            if state["mode"] != "off":
                announce()
        elif cmd == "scan":
            announce()
            publish_peers()
        elif cmd == "send":
            threading.Thread(target=send, args=(c.get("to"), c.get("files", [])), daemon=True).start()
        elif cmd == "cancel":
            sid = c.get("session", "")
            if sid in active_outgoing: cancelled.add(sid)
            cancel_incoming(sid)
        elif cmd == "answer":
            s = sessions.get(c.get("session", ""))
            if s:
                s["accepted"] = bool(c.get("accept"))
                ev = answers.get(s["id"])
                if ev:
                    ev.set()
    os._exit(0)  # stdin closed: the shell quit


if __name__ == "__main__":
    main()
