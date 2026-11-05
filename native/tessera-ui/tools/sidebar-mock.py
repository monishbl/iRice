#!/usr/bin/env python3
"""The sidebar's review mock (tools/sidebar-shots.sh): an OpenAI-compatible chat endpoint with canned answers
(and canned failures), and a yande.re-shaped post list with local pictures.
127.0.0.1 only. mock.py PORT PICTURES_DIR"""
import json, os, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

PORT = int(sys.argv[1]); PICS = sys.argv[2]
pics = sorted(f for f in os.listdir(PICS) if f.endswith(".jpg"))

LISBON = ("## A weekend in Lisbon\n\n"
          "1. **Saturday:** Tram 28 through Alfama, lunch at a *tasca*, sunset at Senhora do Monte\n"
          "2. **Sunday:** pastries in Belém, the riverside walk, LX Factory\n\n"
          "| Day | Morning | Evening |\n|---|---|---|\n| Sat | Alfama | Miradouro |\n| Sun | Belém | LX Factory |\n\n"
          "> Trams fill up by ten: go early.\n\n"
          "```sh\necho 'pack: walking shoes, light jacket'\n```\n\n"
          "More at [visitlisboa.com](https://www.visitlisboa.com).")
DEFAULT = "Here's a short answer, with **the key point** first, and `inline code` after it."

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def j(self, code, obj):
        b = json.dumps(obj).encode()
        self.send_response(code); self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        u = urlparse(self.path); q = parse_qs(u.query)
        if u.path.startswith("/img/"):
            f = os.path.join(PICS, pics[int(u.path[5:].split(".")[0]) % len(pics)])
            b = open(f, "rb").read()
            self.send_response(200); self.send_header("content-type", "image/jpeg")
            self.send_header("content-length", str(len(b))); self.end_headers(); self.wfile.write(b); return
        if u.path in ("/post.json", "/posts.json"):
            tags = q.get("tags", [""])[0]; page = int(q.get("page", ["1"])[0])
            if "nothing" in tags or page > 2:
                return self.j(200, [])
            if "broken" in tags:
                return self.j(500, {"error": "x"})
            key = "preview_file_url" if u.path == "/posts.json" else "preview_url"
            return self.j(200, [{"id": page * 100 + i, key: f"http://127.0.0.1:{PORT}/img/{page * 20 + i}.jpg"} for i in range(20)])
        return self.j(200, {"data": [{"id": "atlas-8b", "object": "model"}]})
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))) or b"{}")
        msgs = body.get("messages", []); last = (msgs[-1] if msgs else {}).get("content") or ""
        text = last if isinstance(last, str) else "picture"
        if "fail" in text.lower():
            return self.j(500, {"error": {"message": "The model is overloaded right now."}})
        ans = LISBON if ("lisbon" in text.lower() or "weekend" in text.lower()) else DEFAULT
        slow = "slow" in text.lower()
        self.send_response(200); self.send_header("content-type", "text/event-stream"); self.end_headers()
        time.sleep(0.3)
        words = ans.split(" ")
        for i, w in enumerate(words):
            self.wfile.write(b"data: " + json.dumps({"choices": [{"index": 0, "delta": {"content": w + (" " if i < len(words) - 1 else "")}}]}).encode() + b"\n\n")
            self.wfile.flush(); time.sleep(0.2 if slow else 0.03)
        self.wfile.write(b"data: " + json.dumps({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]}).encode() + b"\n\ndata: [DONE]\n\n")

ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
