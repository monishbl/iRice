"""lens-upload.py: circle to search's "Search the web". Lens's answer to the
upload (a redirect to the results) read right, the form it gets, and the
whole round trip against a stand-in Lens on localhost (nothing leaves the
computer)."""
import http.server
import importlib.util
import tempfile
import threading
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/lens-upload.py"
spec = importlib.util.spec_from_file_location("lens_upload", SCRIPT)
lens = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lens)

RESULTS = "https://www.google.com/search?vsrid=abc&udm=26&lns_mode=un&gsessionid=x&lsessionid=y"


class Answer(unittest.TestCase):
    def test_a_redirect_to_google_is_the_results_page(self):
        for status in (302, 303):
            self.assertEqual(lens.results_url(status, RESULTS), RESULTS)
        self.assertEqual(lens.results_url(303, "https://lens.google.com/search?p=1 \r\n"), "https://lens.google.com/search?p=1")

    def test_anything_else_is_not(self):
        self.assertIsNone(lens.results_url(200, ""))
        self.assertIsNone(lens.results_url(200, RESULTS))  # (not a redirect)
        self.assertIsNone(lens.results_url(303, ""))
        self.assertIsNone(lens.results_url(400, RESULTS))
        self.assertIsNone(lens.results_url(303, "http://www.google.com/search?x"))  # (not https)
        self.assertIsNone(lens.results_url(303, "https://evil.example/search"))
        self.assertIsNone(lens.results_url(303, "https://google.com.evil.example/search"))
        self.assertIsNone(lens.results_url(303, "javascript:alert(1)"))
        self.assertIsNone(lens.results_url(303, "/search?vsrid=abc"))  # (relative: unknown)

    def test_the_form_carries_the_picture_as_encoded_image(self):
        boundary, payload = lens.body("/run/x/shot-1.png", b"\x89PNG data")
        self.assertIn(b'name="encoded_image"; filename="shot-1.png"', payload)
        self.assertIn(b"Content-Type: image/png\r\n\r\n\x89PNG data\r\n", payload)
        self.assertTrue(payload.endswith(("--" + boundary + "--\r\n").encode()))


class StandIn(http.server.BaseHTTPRequestHandler):
    answer = (303, RESULTS)
    seen = []

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        StandIn.seen.append((self.path, dict(self.headers), self.rfile.read(n)))
        status, location = StandIn.answer
        self.send_response(status)
        if location:
            self.send_header("Location", location)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *a):
        pass


class RoundTrip(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = http.server.HTTPServer(("127.0.0.1", 0), StandIn)
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()
        cls.old = lens.UPLOAD
        lens.UPLOAD = "http://127.0.0.1:%d/v3/upload" % cls.server.server_port
        cls.tmp = tempfile.TemporaryDirectory()
        cls.pic = Path(cls.tmp.name) / "shot-1.jpg"
        cls.pic.write_bytes(b"\xff\xd8\xff\xe0 a jpeg")

    @classmethod
    def tearDownClass(cls):
        lens.UPLOAD = cls.old
        cls.server.shutdown()
        cls.tmp.cleanup()

    def setUp(self):
        StandIn.seen = []

    def test_uploaded_once_without_cookies_and_the_redirect_not_followed(self):
        StandIn.answer = (303, RESULTS)
        self.assertEqual(lens.upload(str(self.pic)), {"ok": True, "url": RESULTS})
        self.assertEqual(len(StandIn.seen), 1)
        path, headers, payload = StandIn.seen[0]
        self.assertTrue(path.startswith("/v3/upload?ep=ccm"))
        self.assertNotIn("Cookie", headers)
        self.assertIn("Mozilla/5.0", headers["User-Agent"])
        self.assertIn(b"\xff\xd8\xff\xe0 a jpeg", payload)

    def test_an_unknown_answer_offers_the_paste_fallback(self):
        StandIn.answer = (200, "")
        r = lens.upload(str(self.pic))
        self.assertFalse(r["ok"])
        self.assertTrue(r["fallback"])

    def test_a_missing_picture_or_no_connection_says_what_to_do(self):
        r = lens.upload(str(Path(self.tmp.name) / "gone.jpg"))
        self.assertEqual((r["ok"], r["fallback"]), (False, False))
        self.assertIn("Circle it again", r["error"])
        self.assertEqual(StandIn.seen, [])
        old = lens.UPLOAD
        lens.UPLOAD = "http://127.0.0.1:9/v3/upload"  # (nothing listens)
        try:
            r = lens.upload(str(self.pic))
        finally:
            lens.UPLOAD = old
        self.assertEqual((r["ok"], r["fallback"]), (False, False))
        self.assertIn("internet connection", r["error"])


if __name__ == "__main__":
    unittest.main()
