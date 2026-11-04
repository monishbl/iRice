#!/usr/bin/env python3
"""Sharing with a phone that has no Quick Share (an iPhone), through its
browser: a QR code its camera opens (services/LinkShare.qml).

  link-share.py [--name NAME] send FILE...   a page with the files: Safari
                                             downloads them
  link-share.py [--name NAME] receive DIR    a page where the phone picks
                                             files; they're saved in DIR
(NAME: this computer's name on the pages; its host name otherwise)

Prints JSON lines: {"ev": "ready", "url", "qr" (an SVG file), "address"},
{"ev": "progress", "dir": "in"|"out", "name", "done", "total"},
{"ev": "sent", "name"}, {"ev": "received", "paths"}, {"ev": "error",
"message"} and {"ev": "closed", "why"}. It ends when stdin closes (the card
went), after LIFETIME, or after too many wrong guesses at the link.

Secure by default:
  - only on the local network: it listens on this computer's private
    address on Wi-Fi or Ethernet (never all addresses), and refuses to start
    without one
  - the link holds a 128-bit secret; anything else gets "not found", and
    after MAX_STRANGERS wrong ones it stops
  - up only while the card shows, at most LIFETIME
  - receiving: names are cleaned (no folders, no hidden files, nothing
    overwritten), each file at most MAX_UPLOAD, MAX_FILES per visit, a
    partial file is removed; nothing runs or opens by itself
  - pages: a strict Content-Security-Policy (the one script by nonce), not
    cached, not sniffed, no referrer
  - plain HTTP (a phone's browser won't take a self-signed certificate), so
    someone else on the same network could see what passes: the card says so
The QR code is a private file (0600, the runtime folder), removed at the end.
"""
import hmac
import html
import ipaddress
import json
import os
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LIFETIME = 10 * 60
MAX_UPLOAD = 4 * 1024 ** 3
MAX_FILES = 200
MAX_STRANGERS = 20
KEEP_FREE = 512 * 1024 ** 2  # receiving: never fills the disk past this
PORTS = (53320, 53319)  # (the firewall lets them in while the card shows: SharingPorts.qml)
CHUNK = 256 * 1024
VIRTUAL = re.compile(r"^(lo|docker|br-|virbr|veth|tailscale|wg|tun|tap|zt|vnet|vmnet|podman|cni|flannel|cali)")

LAN = tuple(ipaddress.ip_network(n) for n in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"))
_out = threading.Lock()


def emit(obj):
    with _out:
        try:
            sys.stdout.write(json.dumps(obj) + "\n")
            sys.stdout.flush()
        except (BrokenPipeError, ValueError):
            os._exit(0)


# ── Where phones can reach it ────────────────────────────────────────────
def private(addr):
    try:
        ip = ipaddress.ip_address(addr)
    except ValueError:
        return False
    # A home or office network's (RFC 1918; an iPhone's hotspot is 172.20.10.x).
    # Not ipaddress's wider "private", which takes 0.0.0.0 (listening there
    # means every network) and the documentation and benchmark ranges.
    return ip.version == 4 and any(ip in n for n in LAN)


def pick_address(ip_json, wireless=lambda name: os.path.exists(f"/sys/class/net/{name}/wireless")):
    """This computer's private IPv4 address phones on its network reach:
    Wi-Fi first, then wired; none on virtual links (containers, VPNs). From
    `ip -j -4 addr show up`."""
    found = []
    for link in ip_json:
        name = link.get("ifname", "")
        if VIRTUAL.match(name) or "UP" not in link.get("flags", []):
            continue
        for a in link.get("addr_info", []):
            if a.get("family") == "inet" and private(a.get("local", "")):
                found.append((0 if wireless(name) else 1, name, a["local"]))
    found.sort()
    return (found[0][2], found[0][1]) if found else (None, None)


def addresses():
    try:
        out = subprocess.run(["ip", "-j", "-4", "addr", "show", "up"], capture_output=True, text=True, timeout=5).stdout
        return json.loads(out or "[]")
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return []


# ── Names ───────────────────────────────────────────────────────────────
def clean_name(name):
    """A file name from a phone, safe to save: no folders, no hidden or
    control characters, not too long; "" if nothing is left."""
    name = name.replace("\\", "/").split("/")[-1]
    name = "".join(c for c in name if c.isprintable() and c not in '<>:"|?*')
    name = name.strip().lstrip(".").strip()
    if len(name.encode()) > 200:
        stem, dot, ext = name.rpartition(".")
        ext = ext[:16] if dot else ""
        keep = (stem if dot else name).encode()[:200 - len(ext.encode()) - 1].decode("utf-8", "ignore")
        name = keep + ("." + ext if ext else "")
    return name


def unique_path(folder, name):
    stem, dot, ext = name.rpartition(".")
    if not dot or not stem:
        stem, ext = name, ""
    path = os.path.join(folder, name)
    n = 2
    while os.path.lexists(path):
        path = os.path.join(folder, f"{stem} ({n})" + ("." + ext if ext else ""))
        n += 1
    return path


def publish(tmp, folder, name):
    """Gives a finished upload its name: a free one, never replacing a file
    (a hard link only succeeds on a free name; a drive without them, such
    as FAT, gets a rename to the name just found free)."""
    for _ in range(100):
        path = unique_path(folder, name)
        try:
            os.link(tmp, path)
        except FileExistsError:
            continue  # (taken meanwhile: the next free one)
        except OSError:
            os.rename(tmp, path)
            return path
        os.unlink(tmp)
        return path
    raise OSError("no free name to save the file under")


def size_text(n):
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1000 or unit == "GB":
            return f"{n:.0f} {unit}" if unit == "B" else f"{n:.1f} {unit}"
        n /= 1000
    return ""


# ── Pages ───────────────────────────────────────────────────────────────
STYLE = """
:root{color-scheme:light dark;--bg:#FAFAFA;--card:#FFF;--line:#E4E4E7;--text:#18181B;--sub:#52525B;--accent:#4F46E5;--onaccent:#FFF}
@media (prefers-color-scheme:dark){:root{--bg:#111113;--card:#1A1A1D;--line:#26262A;--text:#EDEDEF;--sub:#A1A1AA;--accent:#818CF8;--onaccent:#1E1B4B}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:17px/1.4 -apple-system,system-ui,sans-serif}
main{max-width:520px;margin:0 auto;padding:32px 16px}h1{font-size:24px;line-height:32px;font-weight:600;margin:0 0 8px}
p{color:var(--sub);margin:0 0 24px}ul{list-style:none;margin:0;padding:0;background:var(--card);border:1px solid var(--line);border-radius:14px}
li{display:flex;align-items:center;gap:12px;min-height:56px;padding:8px 16px}li+li{border-top:1px solid var(--line)}
.n{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.s{color:var(--sub);font-size:15px;font-variant-numeric:tabular-nums}
a.b,button,label.b{display:inline-flex;align-items:center;justify-content:center;min-height:44px;padding:0 20px;border-radius:10px;border:0;
background:var(--accent);color:var(--onaccent);font:inherit;font-weight:600;text-decoration:none;cursor:pointer}
a.b:focus-visible,button:focus-visible,label.b:focus-within{outline:2px solid var(--accent);outline-offset:2px}
input[type=file]{position:absolute;opacity:0;width:1px;height:1px}button:disabled{opacity:.5}
progress{width:100%;height:8px;margin:24px 0 0;accent-color:var(--accent)}#st{min-height:24px;margin-top:16px}.err{color:#DC2626}
"""


def page(title, body, nonce=None, script=""):
    return ("<!doctype html><html lang=en><head><meta charset=utf-8>"
            "<meta name=viewport content='width=device-width,initial-scale=1'>"
            f"<title>{html.escape(title)}</title><style>{STYLE}</style></head><body><main>{body}</main>"
            + (f"<script nonce='{nonce}'>{script}</script>" if script else "") + "</body></html>").encode()


RECEIVE_SCRIPT = """
const f=document.getElementById('f'),bar=document.getElementById('bar'),st=document.getElementById('st');

function up(i,done){if(i>=f.files.length){bar.hidden=true;st.textContent='Sent '+done+(done==1?' file':' files')+'. Choose more, or close this page.';f.value='';return;}
const file=f.files[i],x=new XMLHttpRequest();x.open('POST','upload?name='+encodeURIComponent(file.name));
x.upload.onprogress=e=>{if(e.lengthComputable){bar.max=e.total;bar.value=e.loaded;}};
x.onload=()=>{if(x.status==200){up(i+1,done+1);}else{bar.hidden=true;st.className='err';st.textContent=x.responseText||'That didn\\'t send. Choose the files again to retry.';}};
x.onerror=()=>{bar.hidden=true;st.className='err';st.textContent='The connection was lost. Stay on the same Wi-Fi, then choose the files again.';};
bar.hidden=false;st.className='';st.textContent='Sending '+(i+1)+' of '+f.files.length+'…';x.send(file);}
f.addEventListener('change',()=>{if(!f.files.length)return;if(f.files.length>"""+str(MAX_FILES)+"""){st.className='err';st.textContent='Choose at most """+str(MAX_FILES)+""" files at a time.';return;}up(0,0);});
"""


# ── The server ──────────────────────────────────────────────────────────
class Share:
    def __init__(self, mode, token, files=(), folder=None, computer="this computer"):
        self.mode = mode
        self.token = token
        self.files = [{"path": p, "name": clean_name(os.path.basename(p)) or "file", "size": os.path.getsize(p)}
                      for p in files]
        self.folder = folder
        self.computer = computer
        self.strangers = 0
        self.received = 0
        self.incoming = 0  # bytes of uploads under way (counted against the free space)
        self.stop = threading.Event()
        self.lock = threading.Lock()

    def check(self, path):
        """The rest of the path after the secret, or None (a stranger)."""
        parts = path.split("?", 1)[0].lstrip("/").split("/", 1)
        if parts and hmac.compare_digest(parts[0].encode(), self.token.encode()):
            return parts[1] if len(parts) > 1 else ""
        with self.lock:
            self.strangers += 1
            if self.strangers >= MAX_STRANGERS:
                emit({"ev": "error", "message": "Someone on this network kept guessing at the link, so sharing stopped."})
                self.stop.set()
        return None


def handler_for(share):
    class Handler(BaseHTTPRequestHandler):
        server_version = "Tessera"
        sys_version = ""
        protocol_version = "HTTP/1.1"
        timeout = 60

        def log_message(self, *args):
            pass

        def _headers(self, code, ctype, length, extra=None, nonce=None):
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(length))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Referrer-Policy", "no-referrer")
            self.send_header("X-Frame-Options", "DENY")
            self.send_header("Content-Security-Policy",
                             "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; connect-src 'self'; "
                             "form-action 'none'; base-uri 'none'; frame-ancestors 'none'"
                             + (f"; script-src 'nonce-{nonce}'" if nonce else ""))
            for k, v in (extra or {}).items():
                self.send_header(k, v)
            self.end_headers()

        def _text(self, code, text):
            body = text.encode()
            self._headers(code, "text/plain; charset=utf-8", len(body))
            self.wfile.write(body)

        def _not_found(self):
            self._text(404, "Not found")

        def do_GET(self):
            rest = share.check(self.path)
            if rest is None:
                return self._not_found()
            if rest == "":
                return self._page()
            if share.mode == "send" and rest.startswith("f/"):
                try:
                    i = int(rest[2:].split("?")[0])
                    f = share.files[i]
                except (ValueError, IndexError):
                    return self._not_found()
                return self._file(f)
            self._not_found()

        def _page(self):
            nonce = secrets.token_urlsafe(16)
            who = html.escape(share.computer)
            if share.mode == "send":
                items = "".join(
                    f"<li><span class=n>{html.escape(f['name'])}</span><span class=s>{size_text(f['size'])}</span>"
                    f"<a class=b href='f/{i}' download='{html.escape(f['name'], quote=True)}'>Save</a></li>"
                    for i, f in enumerate(share.files))
                body = (f"<h1>From {who}</h1><p>{len(share.files)} {'file' if len(share.files) == 1 else 'files'}. "
                        "Tap Save for each one; it goes to your downloads.</p><ul>" + items + "</ul>")
                data = page(f"From {share.computer}", body)
                nonce = None
            else:
                body = (f"<h1>Send to {who}</h1><p>Choose photos or files; they're sent right away and saved "
                        "on that computer.</p><label class=b>Choose files<input id=f type=file multiple></label>"
                        "<progress id=bar hidden></progress><div id=st role=status></div>")
                data = page(f"Send to {share.computer}", body, nonce, RECEIVE_SCRIPT)
            self._headers(200, "text/html; charset=utf-8", len(data), nonce=nonce)
            self.wfile.write(data)

        def _file(self, f):
            size = f["size"]
            start, end = 0, size - 1
            rng = self.headers.get("Range", "")
            m = re.fullmatch(r"bytes=(\d*)-(\d*)", rng.strip())
            partial = bool(m) and size > 0 and (m.group(1) or m.group(2))
            if partial:
                if m.group(1):
                    start = int(m.group(1))
                    end = min(int(m.group(2)), size - 1) if m.group(2) else size - 1
                else:
                    start = max(0, size - int(m.group(2)))
                if start > end or start >= size:
                    self._headers(416, "text/plain", 0, {"Content-Range": f"bytes */{size}"})
                    return
            quoted = urllib.parse.quote(f["name"])
            extra = {"Content-Disposition": f"attachment; filename*=UTF-8''{quoted}", "Accept-Ranges": "bytes"}
            if partial:
                extra["Content-Range"] = f"bytes {start}-{end}/{size}"
            self._headers(206 if partial else 200, "application/octet-stream", end - start + 1, extra)
            sent = 0
            try:
                with open(f["path"], "rb") as fh:
                    fh.seek(start)
                    left = end - start + 1
                    while left > 0 and not share.stop.is_set():
                        chunk = fh.read(min(CHUNK, left))
                        if not chunk:
                            break
                        self.wfile.write(chunk)
                        left -= len(chunk)
                        sent += len(chunk)
                        emit({"ev": "progress", "dir": "out", "name": f["name"], "done": start + sent, "total": size})
            except (OSError, ConnectionError):
                return
            if start + sent >= size:
                emit({"ev": "sent", "name": f["name"]})

        def do_POST(self):
            rest = share.check(self.path)
            if rest is None:
                return self._not_found()
            if share.mode != "receive" or rest.split("?")[0] != "upload":
                return self._not_found()
            q = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)
            name = clean_name(q.get("name", [""])[0]) or "file"
            try:
                length = int(self.headers.get("Content-Length", ""))
            except ValueError:
                return self._text(411, "Your browser didn't say how big the file is. Try another browser.")
            if length < 0 or length > MAX_UPLOAD:
                return self._text(413, "That file is too big (4 GB at most).")
            with share.lock:
                if share.received >= MAX_FILES:
                    return self._text(429, "That's enough files for one go. Open the link again for more.")
                os.makedirs(share.folder, exist_ok=True)
                # (the disk keeps room to spare: a phone never fills it)
                if length + share.incoming + KEEP_FREE > shutil.disk_usage(share.folder).free:
                    return self._text(507, "That computer doesn't have room for this file. Free some space on it, then try again.")
                tmp = os.path.join(share.folder, f".tessera-incoming-{secrets.token_hex(6)}.part")
                fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
                share.received += 1
                share.incoming += length
            done = 0
            try:
                with os.fdopen(fd, "wb") as out:
                    while done < length:
                        chunk = self.rfile.read(min(CHUNK, length - done))
                        if not chunk:
                            raise ConnectionError("cut short")
                        out.write(chunk)
                        done += len(chunk)
                        emit({"ev": "progress", "dir": "in", "name": name, "done": done, "total": length})
                with share.lock:
                    path = publish(tmp, share.folder, name)
            except (OSError, ConnectionError):
                try:
                    os.unlink(tmp)
                except OSError:
                    pass
                self.close_connection = True
                return
            finally:
                with share.lock:
                    share.incoming -= length
            emit({"ev": "received", "paths": [path]})
            self._text(200, "OK")

    return Handler


def make_qr(url):
    if not shutil.which("qrencode"):
        return None, "Showing the code needs qrencode. Install it, then try again."
    folder = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
    path = os.path.join(folder, f"tessera-link-{secrets.token_hex(6)}.svg")
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.close(fd)
        subprocess.run(["qrencode", "-t", "SVG", "-m", "1", "-l", "M", "-o", path], input=url.encode(),
                       check=True, timeout=10, capture_output=True)
        return path, None
    except (OSError, subprocess.SubprocessError) as e:
        try:
            os.unlink(path)
        except OSError:
            pass
        return None, f"Couldn't make the code ({e})."


def main(argv):
    name = None
    if len(argv) > 2 and argv[1] == "--name":
        name, argv = argv[2].strip() or None, argv[:1] + argv[3:]
    if len(argv) < 3 or argv[1] not in ("send", "receive"):
        print(__doc__, file=sys.stderr)
        return 2
    mode = argv[1]
    if mode == "send":
        files = [p for p in argv[2:] if os.path.isfile(p)]
        if not files:
            emit({"ev": "error", "message": "Only files can be shared this way; choose the files inside a folder."})
            return 1
        share_args = {"files": files}
    else:
        share_args = {"folder": os.path.expanduser(argv[2])}
    # (TESSERA_LINKSHARE_HOST: the tests, on the loopback address)
    host, link = (os.environ["TESSERA_LINKSHARE_HOST"], "test") if os.environ.get("TESSERA_LINKSHARE_HOST") else pick_address(addresses())
    if host is None:
        emit({"ev": "error", "message": "This computer isn't on a Wi-Fi or wired network. Connect it and the phone "
                                        "to the same Wi-Fi (or join the iPhone's Personal Hotspot), then try again."})
        return 1
    token = secrets.token_urlsafe(16)
    share = Share(mode, token, computer=name or socket.gethostname(), **share_args)
    # Ports the firewall lets in meanwhile (SharingPorts.qml, "links"):
    # Cast's file port when it isn't casting, the faster-link one, else any
    server = None
    for port in (0,) if os.environ.get("TESSERA_LINKSHARE_HOST") else PORTS + (0,):
        try:
            server = ThreadingHTTPServer((host, port), handler_for(share))
            break
        except OSError as e:
            why = e
    if server is None:
        emit({"ev": "error", "message": f"Couldn't start sharing ({why.strerror or why})."})
        return 1
    server.daemon_threads = True
    url = f"http://{host}:{server.server_address[1]}/{token}/"
    qr, why = make_qr(url)
    if qr is None:
        emit({"ev": "error", "message": why})
        server.server_close()
        return 1
    threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.5}, daemon=True).start()
    emit({"ev": "ready", "url": url, "qr": qr, "address": host, "link": link})

    def watch_stdin():
        try:
            while sys.stdin.readline():
                pass
        except (OSError, ValueError):
            pass
        share.stop.set()
    threading.Thread(target=watch_stdin, daemon=True).start()
    signal.signal(signal.SIGTERM, lambda *_: share.stop.set())  # (the shell ends it: still tidy up)
    why = "closed" if share.stop.wait(LIFETIME) else "expired"
    server.shutdown()
    server.server_close()
    try:
        os.unlink(qr)
    except OSError:
        pass
    emit({"ev": "closed", "why": why})
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
