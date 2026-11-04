"""Sharing through a phone's browser (assets/scripts/link-share.py): the
address it picks, cleaned names, and a real server on the loopback address:
the secret is needed, files download, uploads land safely, strangers stop
it."""
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/link-share.py"
spec = importlib.util.spec_from_file_location("link_share", SCRIPT)
ls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ls)


def link(name, addr, flags=("UP",)):
    return {"ifname": name, "flags": list(flags), "addr_info": [{"family": "inet", "local": addr}]}


class Pure(unittest.TestCase):
    def test_wifi_first_never_virtual_public_or_loopback(self):
        links = [link("lo", "127.0.0.1"), link("docker0", "172.17.0.1"), link("tailscale0", "100.90.1.2"),
                 link("enp3s0", "192.168.1.20"), link("wlan0", "192.168.0.112"), link("wwan0", "8.8.8.8")]
        self.assertEqual(ls.pick_address(links, wireless=lambda n: n == "wlan0"), ("192.168.0.112", "wlan0"))
        self.assertEqual(ls.pick_address(links[:3] + links[5:], wireless=lambda n: False), (None, None))
        self.assertEqual(ls.pick_address([link("wlan0", "10.0.0.5", flags=())]), (None, None))  # (down)

    def test_names_are_cleaned(self):
        self.assertEqual(ls.clean_name("../../.ssh/authorized_keys"), "authorized_keys")
        self.assertEqual(ls.clean_name("C:\\Users\\x\\photo.jpg"), "photo.jpg")
        self.assertEqual(ls.clean_name(".bashrc"), "bashrc")
        self.assertEqual(ls.clean_name("a\x00b\nc.txt"), "abc.txt")
        self.assertEqual(ls.clean_name("..."), "")
        long = ls.clean_name("x" * 400 + ".jpeg")
        self.assertTrue(long.endswith(".jpeg") and len(long.encode()) <= 200)

    def test_nothing_is_overwritten(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, "photo.jpg").write_bytes(b"old")
            self.assertEqual(ls.unique_path(d, "photo.jpg"), os.path.join(d, "photo (2).jpg"))
            self.assertEqual(ls.unique_path(d, "notes"), os.path.join(d, "notes"))


@unittest.skipUnless(shutil.which("qrencode"), "needs qrencode")
class Server(unittest.TestCase):
    def start(self, *args):
        self.rt = tempfile.TemporaryDirectory()
        env = dict(os.environ, TESSERA_LINKSHARE_HOST="127.0.0.1", XDG_RUNTIME_DIR=self.rt.name)
        self.proc = subprocess.Popen([sys.executable, "-I", str(SCRIPT), *args], env=env, text=True,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        ready = json.loads(self.proc.stdout.readline())
        self.assertEqual(ready["ev"], "ready", ready)
        self.assertEqual(os.stat(ready["qr"]).st_mode & 0o777, 0o600)
        return ready

    def tearDown(self):
        if getattr(self, "proc", None):
            self.proc.stdin.close()
            self.proc.wait(10)
            self.proc.stdout.close()
            self.rt.cleanup()

    def events(self):
        self.proc.stdin.close()
        self.proc.wait(10)
        return [json.loads(line) for line in self.proc.stdout.read().splitlines()]

    def test_send_needs_the_secret_and_downloads(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d, "photo one.jpg")
            f.write_bytes(os.urandom(300_000))
            ready = self.start("send", str(f))
            url = ready["url"]
            page = urllib.request.urlopen(url, timeout=5)
            self.assertIn("default-src 'none'", page.headers["Content-Security-Policy"])
            self.assertEqual(page.headers["Cache-Control"], "no-store")
            self.assertIn("photo one.jpg", page.read().decode())
            got = urllib.request.urlopen(url + "f/0", timeout=5)
            self.assertIn("attachment", got.headers["Content-Disposition"])
            self.assertEqual(got.read(), f.read_bytes())
            part = urllib.request.urlopen(urllib.request.Request(url + "f/0", headers={"Range": "bytes=10-19"}), timeout=5)
            self.assertEqual((part.status, part.read()), (206, f.read_bytes()[10:20]))
            base = url.rsplit("/", 2)[0]
            for wrong in ("/", "/f/0", "/" + "A" * 22 + "/f/0", "/../../etc/passwd"):
                with self.assertRaises(urllib.error.HTTPError) as e:
                    urllib.request.urlopen(base + wrong, timeout=5)
                self.assertEqual(e.exception.code, 404)
            qr = ready["qr"]
            evs = self.events()
            self.assertIn({"ev": "sent", "name": "photo one.jpg"}, evs)
            self.assertFalse(os.path.exists(qr))  # (removed at the end)

    def test_receive_saves_safely(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, "photo.jpg").write_bytes(b"mine")
            ready = self.start("receive", d)
            url = ready["url"]
            page = urllib.request.urlopen(url, timeout=5)
            csp = page.headers["Content-Security-Policy"]
            self.assertIn("script-src 'nonce-", csp)
            self.assertIn(csp.split("'nonce-")[1].split("'")[0], page.read().decode())
            data = os.urandom(50_000)
            for name in ("photo.jpg", "../../evil.sh"):
                r = urllib.request.urlopen(urllib.request.Request(
                    url + "upload?name=" + urllib.request.quote(name), data=data, method="POST"), timeout=5)
                self.assertEqual(r.status, 200)
            self.assertEqual(Path(d, "photo.jpg").read_bytes(), b"mine")  # (not overwritten)
            self.assertEqual(Path(d, "photo (2).jpg").read_bytes(), data)
            self.assertEqual(Path(d, "evil.sh").read_bytes(), data)
            self.assertFalse(os.access(Path(d, "evil.sh"), os.X_OK))
            self.assertEqual(sorted(p.name for p in Path(d).iterdir()), ["evil.sh", "photo (2).jpg", "photo.jpg"])
            with self.assertRaises(urllib.error.HTTPError) as e:
                urllib.request.urlopen(urllib.request.Request(url.rsplit("/", 2)[0] + "/x/upload?name=a",
                                                              data=b"x", method="POST"), timeout=5)
            self.assertEqual(e.exception.code, 404)
            got = [e for e in self.events() if e["ev"] == "received"]
            self.assertEqual(len(got), 2)

    def test_too_many_wrong_guesses_stop_it(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d, "a.txt")
            f.write_text("hi")
            ready = self.start("send", str(f))
            base = ready["url"].rsplit("/", 2)[0]
            for i in range(ls.MAX_STRANGERS):
                try:
                    urllib.request.urlopen(f"{base}/guess{i}/", timeout=5)
                except urllib.error.HTTPError:
                    pass
            self.proc.wait(10)
            evs = [json.loads(line) for line in self.proc.stdout.read().splitlines()]
            self.assertEqual(evs[-1], {"ev": "closed", "why": "closed"})
            self.assertTrue(any(e["ev"] == "error" and "guessing" in e["message"] for e in evs))


if __name__ == "__main__":
    unittest.main()
