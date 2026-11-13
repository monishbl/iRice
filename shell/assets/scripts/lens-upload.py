#!/usr/bin/env python3
"""Search a picture with Google Lens (circle to search's "Search the web",
the Lens shortcut's lens-search.sh).

  lens-upload.py FILE   (or --browser FILE)
        hands the picture to the browser, which uploads it to Lens itself:
        prints one line of JSON, {"ok": true, "url": a one-time page on
        127.0.0.1} or {"ok": false, "error", "fallback"}. Open the url in the
        browser: the page sends the picture to Lens as Lens's own page does
        and the results open there.
  lens-upload.py --direct FILE
        the old way: uploads FILE from here and prints {"ok": true, "url":
        the results page} (kept to diagnose with; see below why it fails)

Why the browser uploads it (the "image expired" page): Lens keeps an
upload's results with the session that sent it. Uploaded from here, the
results address (now www.google.com/search?…&vsrid=…&gsessionid=…, after a
303 that also sets this script's own NID cookie) belongs to a session the
browser doesn't have, so opening it there shows that the image has expired,
however soon it's opened. Sent by the browser, upload and results share the
browser's session.

Nothing is uploaded unless asked; this serves the page once, to 127.0.0.1
only, under a random address, then stops (at most two minutes). The picture
never leaves the computer except to Lens, from the browser. "fallback":
something here failed: open https://lens.google.com/ and paste the picture.
"""
import base64
import html
import json
import mimetypes
import os
import secrets
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UPLOAD = "https://lens.google.com/v3/upload"
# (a usual browser: Lens serves other clients a page that asks for one)
AGENT = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"
LIMIT = 20 * 1024 * 1024
WAIT = 120


def results_url(status, location):
    """The results page from Lens's answer to the upload (a redirect to it), else None."""
    if status not in (301, 302, 303, 307, 308) or not location:
        return None
    u = urllib.parse.urlsplit(location.strip())
    host = (u.hostname or "").lower()
    if u.scheme != "https" or not (host == "google.com" or host.endswith(".google.com")):
        return None
    return location.strip()


def read(path):
    try:
        with open(path, "rb") as f:
            data = f.read(LIMIT + 1)
    except OSError:
        return None, {"ok": False, "error": "The picture is gone. Circle it again to search it.", "fallback": False}
    if len(data) > LIMIT:
        return None, {"ok": False, "error": "The picture is too big for Lens. Circle a smaller area.", "fallback": False}
    return data, None


def page(path, data):
    """The page the browser opens: it sends the picture to Lens as a form,
    as lens.google.com's own upload does (encoded_image), and Lens answers
    with its results in that same browser session"""
    kind = mimetypes.guess_type(path)[0] or "image/jpeg"
    action = UPLOAD + "?" + urllib.parse.urlencode({"ep": "ccm", "s": "", "st": str(int(time.time() * 1000))})
    b64 = base64.b64encode(data).decode()
    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="referrer" content="no-referrer">
<meta name="viewport" content="width=device-width">
<title>Searching with Google Lens</title>
<style>
:root {{ color-scheme: light dark; }}
body {{ margin: 0; height: 100vh; display: grid; place-items: center; font: 15px/1.4 system-ui, sans-serif; background: #FAFAFA; color: #27272A; }}
@media (prefers-color-scheme: dark) {{ body {{ background: #111113; color: #D4D4D8; }} }}
p {{ margin: 0; }}
</style></head>
<body><p id="m">Sending the picture to Google Lens…</p>
<form id="f" method="post" enctype="multipart/form-data" action="{html.escape(action)}">
<input type="file" name="encoded_image" id="i" hidden></form>
<script>
try {{
  const s = atob("{b64}");
  const a = new Uint8Array(s.length);
  for (let k = 0; k < s.length; k++) a[k] = s.charCodeAt(k);
  const t = new DataTransfer();
  t.items.add(new File([a], "picture{mimetypes.guess_extension(kind) or '.jpg'}", {{ type: "{kind}" }}));
  document.getElementById("i").files = t.files;
  document.getElementById("f").submit();
}} catch (e) {{
  document.getElementById("m").textContent = "This browser couldn't send the picture. Open lens.google.com and paste it with Ctrl+V.";
}}
</script>
<noscript><p>Turn on JavaScript for this page, or open lens.google.com and paste the picture with Ctrl+V.</p></noscript>
</body></html>""".encode()


def serve_once(body):
    """A one-time page on 127.0.0.1 (a random port and path): its address,
    or None. The server runs on in a detached child until the page has been
    fetched once, or WAIT seconds."""
    import socket
    token = secrets.token_urlsafe(18)
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.bind(("127.0.0.1", 0))
        sock.listen(4)
    except OSError:
        return None
    port = sock.getsockname()[1]
    try:
        pid = os.fork()
    except OSError:
        return None
    if pid:
        sock.close()
        return f"http://127.0.0.1:{port}/{token}"
    # the child: detached, quiet; the caller reads the parent's one line
    try:
        os.setsid()
        null = os.open(os.devnull, os.O_RDWR)
        for fd in (0, 1, 2):
            os.dup2(null, fd)
        sock.settimeout(1.0)
        end = time.monotonic() + WAIT
        while time.monotonic() < end:
            try:
                conn, _ = sock.accept()
            except (socket.timeout, OSError):
                continue
            with conn:
                conn.settimeout(5)
                try:
                    req = conn.recv(4096).decode("latin-1")
                except OSError:
                    continue
                line = req.split("\r\n", 1)[0].split(" ")
                if len(line) >= 2 and line[0] == "GET" and line[1] == "/" + token:
                    head = ("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\n"
                            "Referrer-Policy: no-referrer\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % len(body))
                    try:
                        conn.sendall(head.encode() + body)
                    except OSError:
                        continue
                    break
                try:
                    conn.sendall(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                except OSError:
                    pass
    finally:
        os._exit(0)


def browser(path):
    data, err = read(path)
    if err:
        return err
    url = serve_once(page(path, data))
    if not url:
        return {"ok": False, "error": "Couldn't hand the picture to the browser.", "fallback": True}
    return {"ok": True, "url": url}


def body(path, data):
    """The form Lens's page sends: the picture as encoded_image."""
    boundary = "----tessera" + secrets.token_hex(12)
    kind = mimetypes.guess_type(path)[0] or "image/jpeg"
    head = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"encoded_image\"; "
            f"filename=\"{os.path.basename(path).replace(chr(34), '')}\"\r\nContent-Type: {kind}\r\n\r\n").encode()
    return boundary, head + data + f"\r\n--{boundary}--\r\n".encode()


class Stay(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None  # (the redirect is the answer: don't follow it)


def upload(path):
    data, err = read(path)
    if err:
        return err
    boundary, payload = body(path, data)
    query = urllib.parse.urlencode({"ep": "ccm", "s": "", "st": str(int(time.time() * 1000))})
    req = urllib.request.Request(UPLOAD + "?" + query, data=payload, method="POST", headers={
        "Content-Type": "multipart/form-data; boundary=" + boundary,
        "User-Agent": AGENT,
        "Accept": "text/html,*/*",
    })
    status, location = 0, ""
    try:
        with urllib.request.build_opener(Stay).open(req, timeout=30) as r:
            status, location = r.status, r.headers.get("Location", "")
    except urllib.error.HTTPError as e:
        status, location = e.code, e.headers.get("Location", "")
    except (urllib.error.URLError, OSError):
        return {"ok": False, "error": "Couldn't reach Google Lens. Check your internet connection and try again.", "fallback": False}
    url = results_url(status, location)
    if url:
        return {"ok": True, "url": url}
    return {"ok": False, "error": "Google Lens didn't take the picture this way (it answered %d)." % status, "fallback": True}


if __name__ == "__main__":
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--direct":
        print(json.dumps(upload(args[1])), flush=True)
    elif len(args) == 2 and args[0] == "--browser" or len(args) == 1:
        print(json.dumps(browser(args[-1])), flush=True)
    else:
        sys.exit("usage: lens-upload.py [--browser | --direct] FILE")
