"""The bug report script (shell/assets/scripts/bug-report.py): what it takes
out of a log before anything leaves the computer, the size limits, and that
it only ever sends over https."""
import importlib.util
import io
import json
import os
import shutil
import subprocess
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "shell/assets/scripts/bug-report.py"


def load():
    spec = importlib.util.spec_from_file_location("bug_report", SCRIPT)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


SAMPLE = """\
\x1b[33m WARN\x1b[0m: loading /home/alice/.config/irice/config.json
 INFO: user alice on alice-laptop, mail alice.smith@example.org
 INFO: token = ghp_AbC123dEf456GhI789jKl012MnO345pQr678
 INFO: Authorization: Bearer sk-ant-api03-ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ
 INFO: jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dGVzdHNpZ25hdHVyZQ
 INFO: hash 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
 INFO: connected to Home Sweet Home at 192.168.1.42 (gw fe80::1c2b:3d4e:5f60:7a8b)
 INFO: localhost 127.0.0.1 stays
 INFO: AirPods of Sam battery 80% (AA:BB:CC:DD:EE:FF)
 INFO: plain words and /usr/share/icons/hicolor/48x48/apps stay as they are
 INFO: alicewonder is a different word
"""


class Scrubbing(unittest.TestCase):
    def setUp(self):
        self.m = load()
        self.out = self.m.scrub(SAMPLE, names={"Home Sweet Home", "AirPods of Sam"},
                                home="/home/alice", user="alice", host="alice-laptop")

    def test_personal_details_are_gone(self):
        for secret in ("/home/alice", "alice.smith@example.org", "ghp_AbC123dEf456GhI789jKl012MnO345pQr678",
                       "sk-ant-api03", "eyJhbGciOiJIUzI1NiJ9", "9f86d081884c7d659a2f", "192.168.1.42",
                       "fe80::1c2b", "AA:BB:CC:DD:EE:FF", "Home Sweet Home", "AirPods of Sam", "alice-laptop",
                       "\x1b["):
            self.assertNotIn(secret, self.out, secret)
        self.assertNotIn(" alice ", self.out)

    def test_what_replaces_them_says_what_it_was(self):
        for label in ("~/.config/irice", "<email>", "<redacted>", "<token>", "<hex>", "<ip>", "<mac>", "<name>", "<user>", "<host>"):
            self.assertIn(label, self.out, label)

    def test_harmless_text_is_kept(self):
        self.assertIn("127.0.0.1", self.out)
        self.assertIn("/usr/share/icons/hicolor/48x48/apps", self.out)
        self.assertIn("plain words", self.out)
        self.assertIn("alicewonder", self.out)  # not the user name, only contains it
        self.assertIn("battery 80%", self.out)


class Payload(unittest.TestCase):
    def setUp(self):
        self.m = load()
        self.report = {"title": "Dock vanishes", "happened": "It's gone after waking", "steps": "", "expected": "",
                       "includeLog": True, "includeInfo": True, "includeShot": False,
                       "log": "a log", "sysinfo": {"iRice": "abc123", "Hyprland": "0.56.2"}}

    def test_unticked_parts_are_not_sent(self):
        body = self.m.payload(dict(self.report, includeLog=False, includeInfo=False))
        self.assertNotIn("log", body)
        self.assertNotIn("sysinfo", body)
        self.assertEqual(body["version"], "abc123")

    def test_title_and_what_happened_are_needed(self):
        with self.assertRaisesRegex(ValueError, "Add a title"):
            self.m.payload(dict(self.report, title="  "))
        with self.assertRaisesRegex(ValueError, "Say what happened"):
            self.m.payload(dict(self.report, happened=""))

    def test_a_report_over_2_mb_is_refused(self):
        big = dict(self.report, happened="x" * 20000, log="y" * 200000)
        self.assertLess(len(json.dumps(self.m.payload(big))), self.m.MAX_BYTES)
        with mock.patch.object(self.m, "MAX_BYTES", 1000):
            with self.assertRaisesRegex(ValueError, "too large"):
                self.m.payload(big)

    @unittest.skipUnless(shutil.which("magick") or shutil.which("convert"), "needs ImageMagick")
    def test_the_screenshot_is_downscaled_and_fits(self):
        with tempfile.TemporaryDirectory() as d:
            shot = os.path.join(d, "screen.png")
            magick = shutil.which("magick") or shutil.which("convert")
            subprocess.run([magick, "-size", "3840x2160", "plasma:", shot], check=True, capture_output=True)
            body = self.m.payload(dict(self.report, includeShot=True, shot=shot))
            self.assertIn(body["screenshot"]["type"], ("image/webp", "image/jpeg"))
            self.assertLessEqual(len(json.dumps(body)), self.m.MAX_BYTES)
            out = os.path.join(d, "out")
            with open(out, "wb") as f:
                import base64
                f.write(base64.b64decode(body["screenshot"]["data"]))
            size = subprocess.run([magick, "identify", "-format", "%w %h", out] if "magick" in magick else ["identify", "-format", "%w %h", out],
                                  capture_output=True, text=True).stdout.split()
            self.assertLessEqual(max(int(size[0]), int(size[1])), self.m.MAX_SIDE)

    def test_a_missing_screenshot_is_reported(self):
        with self.assertRaisesRegex(ValueError, "screenshot"):
            self.m.payload(dict(self.report, includeShot=True, shot="/nonexistent.png"))


class Sending(unittest.TestCase):
    def setUp(self):
        self.m = load()
        self.report = {"title": "T", "happened": "H", "includeLog": False, "includeInfo": False, "includeShot": False}

    def test_only_https(self):
        with mock.patch.object(self.m.urllib.request, "urlopen") as urlopen:
            for url in ("http://irice-bugs.monishbl.in/api/bug-reports", "ftp://x/y", "file:///etc/passwd"):
                with self.assertRaisesRegex(ValueError, "https"):
                    self.m.send(self.report, endpoint=url)
            urlopen.assert_not_called()

    def test_answers_and_errors(self):
        ok = io.BytesIO(json.dumps({"ok": True, "id": "abc", "issue_url": "https://github.com/x/1"}).encode())
        with mock.patch.object(self.m.urllib.request, "urlopen", return_value=mock.MagicMock(__enter__=lambda s: ok, __exit__=lambda *a: None)) as urlopen:
            r = self.m.send(self.report)
            self.assertEqual(r["issue_url"], "https://github.com/x/1")
            req = urlopen.call_args[0][0]
            self.assertTrue(req.full_url.startswith("https://"))
            self.assertEqual(urlopen.call_args[1]["timeout"], self.m.TIMEOUT)

        def http_error(code, body):
            return urllib.error.HTTPError("https://x", code, "err", {}, io.BytesIO(json.dumps(body).encode()))
        with mock.patch.object(self.m.urllib.request, "urlopen", side_effect=http_error(429, {})):
            with self.assertRaisesRegex(ValueError, "Wait an hour"):
                self.m.send(self.report)
        with mock.patch.object(self.m.urllib.request, "urlopen", side_effect=http_error(400, {"error": "\"title\" is required."})):
            with self.assertRaisesRegex(ValueError, "title"):
                self.m.send(self.report)
        with mock.patch.object(self.m.urllib.request, "urlopen", side_effect=urllib.error.URLError("down")):
            with self.assertRaisesRegex(ValueError, "Check you're online"):
                self.m.send(self.report)

    def test_the_github_link_leaves_out_the_log(self):
        url = self.m.github_url(dict(self.report, includeLog=True, log="SECRET-LOG", includeInfo=True, sysinfo={"Hyprland": "0.56"}))
        self.assertTrue(url.startswith("https://github.com/monishbl/iRice/issues/new?"))
        self.assertNotIn("SECRET-LOG", url)
        self.assertIn("bug-report", url)

    def test_command_line_errors_are_json(self):
        out = subprocess.run(["python3", "-I", str(SCRIPT), "send"], input=json.dumps({"title": "", "happened": "x"}),
                             capture_output=True, text=True, timeout=20)
        self.assertEqual(out.returncode, 1)
        self.assertEqual(json.loads(out.stdout), {"ok": False, "error": "Add a title, then send again."})


if __name__ == "__main__":
    unittest.main()
