#!/usr/bin/env python3
"""Search a picture with Google Lens (circle to search's "Search the web",
the Lens shortcut's lens-search.sh).

  lens-upload.py FILE   uploads FILE to Lens as its web page does and prints
                        one line of JSON: {"ok": true, "url": the results
                        page} or {"ok": false, "error", "fallback"}

The caller opens the results in the browser (Launch.qml, so the browser
gets the person's own settings). Only on request: nothing is uploaded
otherwise. No cookies go or are kept: the results address carries Lens's
session itself. "fallback": Lens answered in a way this doesn't know (it
changed): open https://lens.google.com/ and paste the picture there.
"""
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


def results_url(status, location):
    """The results page from Lens's answer to the upload (a redirect to it), else None."""
    if status not in (301, 302, 303, 307, 308) or not location:
        return None
    u = urllib.parse.urlsplit(location.strip())
    host = (u.hostname or "").lower()
    if u.scheme != "https" or not (host == "google.com" or host.endswith(".google.com")):
        return None
    return location.strip()


def body(path, data):
    """The form Lens's page sends: the picture as encoded_image."""
    boundary = "----irice" + secrets.token_hex(12)
    kind = mimetypes.guess_type(path)[0] or "image/jpeg"
    head = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"encoded_image\"; "
            f"filename=\"{os.path.basename(path).replace(chr(34), '')}\"\r\nContent-Type: {kind}\r\n\r\n").encode()
    return boundary, head + data + f"\r\n--{boundary}--\r\n".encode()


class Stay(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None  # (the redirect is the answer: don't follow it)


def upload(path):
    try:
        with open(path, "rb") as f:
            data = f.read(LIMIT + 1)
    except OSError:
        return {"ok": False, "error": "The picture is gone. Circle it again to search it.", "fallback": False}
    if len(data) > LIMIT:
        return {"ok": False, "error": "The picture is too big for Lens. Circle a smaller area.", "fallback": False}
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
    if len(sys.argv) != 2:
        sys.exit("usage: lens-upload.py FILE")
    print(json.dumps(upload(sys.argv[1])))
