#!/usr/bin/env python3
"""LocalSend for iRice: send files to, and receive files from, nearby devices
running LocalSend (phones, other computers), like AirDrop. Speaks LocalSend
protocol v2: multicast discovery on 224.0.0.167:53317 and plain HTTP.

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
       {"ev":"finished","session","dir","paths":[...]} | {"ev":"declined","session"}
       {"ev":"failed","session","message"} | {"ev":"error","message"}
Exits when stdin closes (the shell quit). Standard library only.
"""
import hashlib
import http.client
import http.server
import json
import os
import platform
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
cancelled = set()  # outgoing session ids to stop


def emit(ev, **kw):
    kw["ev"] = ev
    with lock:
        sys.stdout.write(json.dumps(kw) + "\n")
        sys.stdout.flush()


def info(announce=None):
    d = {"alias": state["alias"], "version": "2.1", "deviceModel": platform.system(), "deviceType": "desktop",
         "fingerprint": FINGERPRINT, "port": PORT, "protocol": "http", "download": False}
    if announce is not None:
        d["announce"] = announce
    return d


def publish_peers():
    now = time.time()
    for k in [k for k, p in peers.items() if now - p["seen"] > 90]:
        del peers[k]
    emit("peers", peers=[{"id": k, "alias": p["alias"], "type": p["type"], "model": p["model"]} for k, p in peers.items()])


def remember(d, ip):
    fp = d.get("fingerprint")
    if not fp or fp == FINGERPRINT:
        return False
    new = fp not in peers
    peers[fp] = {"alias": d.get("alias", "Device"), "type": d.get("deviceType") or "mobile", "model": d.get("deviceModel") or "",
                 "ip": ip, "port": int(d.get("port") or DISCOVERY), "protocol": d.get("protocol") or "https", "seen": time.time()}
    return new


def connect(p, timeout=10):
    if p["protocol"] == "https":
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE  # LocalSend uses self-signed certificates
        return http.client.HTTPSConnection(p["ip"], p["port"], timeout=timeout, context=ctx)
    return http.client.HTTPConnection(p["ip"], p["port"], timeout=timeout)


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


def reply_to(d, ip):
    """Someone announced: tell them about us (only while receiving)."""
    if state["mode"] == "off":
        return
    p = {"ip": ip, "port": int(d.get("port") or DISCOVERY), "protocol": d.get("protocol") or "https"}
    try:
        c = connect(p, timeout=4)
        c.request("POST", API + "/register", json.dumps(info()), {"Content-Type": "application/json"})
        c.getresponse().read()
    except Exception:
        # Fall back to answering on multicast
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
            s.sendto(json.dumps(info(False)).encode(), (GROUP, DISCOVERY))
            s.close()
        except OSError:
            pass


def listen():
    try:
        s = multicast_socket()
    except OSError as e:
        emit("error", message="Couldn't listen for nearby devices: %s" % e.strerror)
        return
    while True:
        try:
            data, (ip, _) = s.recvfrom(65536)
            d = json.loads(data.decode())
        except Exception:
            continue
        if d.get("fingerprint") == FINGERPRINT:
            continue
        if remember(d, ip):
            publish_peers()
        if d.get("announce"):
            threading.Thread(target=reply_to, args=(d, ip), daemon=True).start()


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
        return self.rfile.read(n) if n else b""

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
            s = sessions.pop(q.get("sessionId", ""), None)
            if s:
                emit("failed", session=s["id"], message="%s cancelled" % s["from"])
            return self.reply(200)
        self.reply(404)

    def prepare(self):
        if state["mode"] == "off":
            return self.reply(403, {"message": "Not receiving"})
        try:
            req = json.loads(self.body())
        except Exception:
            return self.reply(400)
        sender = req.get("info", {})
        files = req.get("files", {})
        sid = secrets.token_hex(8)
        s = {"id": sid, "from": sender.get("alias", "Someone"), "files": files, "tokens": {}, "paths": [], "done": 0,
             "total": sum(int(f.get("size") or 0) for f in files.values()), "accepted": None}
        sessions[sid] = s
        ev = answers[sid] = threading.Event()
        emit("incoming", session=sid, **{"from": s["from"]}, type=sender.get("deviceType") or "mobile",
             files=[{"name": f.get("fileName", "file"), "size": int(f.get("size") or 0)} for f in files.values()], total=s["total"])
        ev.wait(60)
        answers.pop(sid, None)
        if not s["accepted"]:
            sessions.pop(sid, None)
            return self.reply(403, {"message": "Declined"})
        s["tokens"] = {fid: secrets.token_hex(12) for fid in files}
        os.makedirs(DOWNLOADS, exist_ok=True)
        self.reply(200, {"sessionId": sid, "files": s["tokens"]})

    def upload(self, q):
        s = sessions.get(q.get("sessionId", ""))
        fid = q.get("fileId", "")
        if not s or s["tokens"].get(fid) != q.get("token"):
            return self.reply(403)
        name = os.path.basename(s["files"][fid].get("fileName") or "file") or "file"
        path = unique(os.path.join(DOWNLOADS, name))
        left = int(self.headers.get("Content-Length") or 0)
        last = 0
        try:
            with open(path, "wb") as out:
                while left > 0:
                    chunk = self.rfile.read(min(1 << 20, left))
                    if not chunk:
                        break
                    out.write(chunk)
                    left -= len(chunk)
                    s["done"] += len(chunk)
                    if time.time() - last > 0.3:
                        last = time.time()
                        emit("progress", session=s["id"], dir="in", done=s["done"], total=s["total"], name=name)
        except OSError as e:
            emit("failed", session=s["id"], message="Couldn't save %s: %s" % (name, e.strerror))
            return self.reply(500)
        s["paths"].append(path)
        self.reply(200)
        if len(s["paths"]) == len(s["files"]):
            sessions.pop(s["id"], None)
            emit("finished", session=s["id"], dir="in", paths=s["paths"], **{"from": s["from"]})


def serve():
    try:
        srv = http.server.ThreadingHTTPServer(("", PORT), Handler)
    except OSError:
        emit("error", message="Port %d is in use (is the LocalSend app open?). You can still send." % PORT)
        return
    state["server"] = True
    emit("ready", port=PORT, receiving=state["mode"] != "off")
    srv.serve_forever()


# ── Sending ──────────────────────────────────────────────────────────────
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
    emit("progress", session=sid, dir="out", done=0, total=total, name=p["alias"], waiting=True)
    try:
        c = connect(p, timeout=90)  # they decide on their side
        c.request("POST", API + "/prepare-upload", json.dumps({"info": info(), "files": files}), {"Content-Type": "application/json"})
        r = c.getresponse()
        body = r.read()
        if r.status == 204:
            return emit("finished", session=sid, dir="out", paths=[], to=p["alias"])
        if r.status == 403:
            return emit("declined", session=sid, to=p["alias"])
        if r.status != 200:
            return emit("failed", session=sid, message="%s couldn't take the files (%d)" % (p["alias"], r.status))
        resp = json.loads(body)
        remote, tokens = resp["sessionId"], resp["files"]
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
                while True:
                    if sid in cancelled:
                        c.close()
                        try:
                            connect(p, 5).request("POST", API + "/cancel?" + urllib.parse.urlencode({"sessionId": remote}))
                        except Exception:
                            pass
                        return emit("failed", session=sid, message="Cancelled")
                    chunk = f.read(1 << 20)
                    if not chunk:
                        break
                    c.send(chunk)
                    done += len(chunk)
                    if time.time() - last > 0.3:
                        last = time.time()
                        emit("progress", session=sid, dir="out", done=done, total=total, name=p["alias"])
            r = c.getresponse()
            r.read()
            if r.status != 200:
                return emit("failed", session=sid, message="%s stopped the transfer" % p["alias"])
        emit("finished", session=sid, dir="out", paths=list(ids.values()), to=p["alias"])
    except Exception as e:
        emit("failed", session=sid, message="Couldn't reach %s (%s)" % (p["alias"], e.__class__.__name__))


# ── Commands ─────────────────────────────────────────────────────────────
def main():
    threading.Thread(target=listen, daemon=True).start()
    threading.Thread(target=serve, daemon=True).start()
    for line in sys.stdin:
        try:
            c = json.loads(line)
        except Exception:
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
            cancelled.add(sid)
            if sessions.pop(sid, None):
                emit("failed", session=sid, message="Cancelled")
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
