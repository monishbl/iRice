#!/usr/bin/env python3
"""A canned, OpenAI-compatible chat endpoint for filming the AI sidebar in the
sandbox: streams fixed answers word by word, so every take is the same and no
real model or key is involved.
  mock-ai.py [port]    (127.0.0.1 only; the model is set to it as a custom
                        endpoint with no key, see capture.sh)
"""
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TRAM = ("That's one of **Lisbon's yellow trams**: a 1930s *Remodelado*, here in the Baixa, "
        "the same kind that runs route 28 up through Alfama.\n\n"
        "- Still in daily service, wooden seats and all\n"
        "- Ride route 28 early from Martim Moniz to get a seat\n"
        "- A 24-hour pass covers every tram")
LISBON = ("**A weekend in Lisbon**\n\n"
          "1. **Saturday:** Tram 28 through Alfama, lunch at a *tasca*, sunset at Senhora do Monte\n"
          "2. **Sunday:** pastries in Belém, the riverside walk, LX Factory\n\n"
          "```text\npack: walking shoes · light jacket · camera\n```")
DEFAULT = "Here's a short answer, with **the key point** first."


def answer(messages):
    last = messages[-1] if messages else {}
    content = last.get("content")
    if isinstance(content, list):  # (a picture: circle to search)
        return TRAM
    text = content or ""
    if "lisbon" in text.lower() or "weekend" in text.lower():
        return LISBON
    return DEFAULT


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps({"data": [{"id": "atlas-8b", "object": "model"}]}).encode())

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))) or b"{}")
        text = answer(body.get("messages", []))
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.end_headers()
        time.sleep(0.35)  # (a little thinking before the first word)
        words = text.split(" ")
        for i, w in enumerate(words):
            chunk = w + (" " if i < len(words) - 1 else "")
            self.wfile.write(b"data: " + json.dumps({"choices": [{"index": 0, "delta": {"content": chunk}}]}).encode() + b"\n\n")
            self.wfile.flush()
            time.sleep(0.045)
        self.wfile.write(b"data: " + json.dumps({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]}).encode() + b"\n\n")
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 47311
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
