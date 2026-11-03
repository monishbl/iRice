"""Secure by default: what iRice does before anyone changes a setting, and
the guards that keep it that way (SECURITY-AUDIT.md).

  sharing        nothing is visible until you turn it on; nothing is saved
                 before you say yes; names from other devices are cleaned
  link sharing   only on a private address, only with the link's secret,
                 never filling the disk or replacing a file
  root helper    each verb refuses what it doesn't expect; the ones that
                 run without a password are the few fixed ones
  Wi-Fi helper   (root) a phone's network can't take over this computer's
                 routes
  private files  0600 where they hold something private; never through a
                 link in a shared folder
  the shell      no shell command built from outside data, links from
                 outside open only on the web, and text from outside is
                 shown as text (never as markup that loads pictures)

Nothing here asks for a password, changes the system or touches the network
beyond 127.0.0.1.
"""
import contextlib
import importlib.util
import io
import json
import inspect
import os
import re
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from http.server import ThreadingHTTPServer
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"
QS = SCRIPTS / "quickshare"
sys.path.insert(0, str(QS))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


R = load("irice_root_sec", SCRIPTS / "irice-root.py")
WIFI = load("qs_wifi_helper_sec", QS / "qs-wifi-helper.py")
LINK = load("link_share_sec", SCRIPTS / "link-share.py")
CAST = load("cast_sec", SCRIPTS / "cast.py")


# ── Defaults ──────────────────────────────────────────────────────────────
class Defaults(unittest.TestCase):
    def config(self):
        return (ROOT / "shell/common/Config.qml").read_text()

    def test_sharing_is_off_until_turned_on(self):
        text = self.config()
        sharing = re.search(r"property JsonObject sharing: JsonObject \{(.*?)\n            \}", text, re.S).group(1)
        self.assertRegex(sharing, r'property string receiving: "off"')  # LocalSend
        self.assertRegex(sharing, r'property string quickShare: "off"')  # Quick Share

    def test_localsend_starts_not_receiving(self):
        ls = load("localsend_default", SCRIPTS / "localsend.py")
        self.assertEqual(ls.state["mode"], "off")

    def test_quick_share_starts_invisible(self):
        from nearby.service import QuickShareService
        svc = QuickShareService(lambda ev: None, None, name="Test")
        self.assertFalse(svc.visible)
        self.assertIsNone(svc.server)


# ── Receiving asks first ──────────────────────────────────────────────────
class LocalSendAsksFirst(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.m = load("localsend_sec", SCRIPTS / "localsend.py")
        self.m.DOWNLOADS = self.tmp.name
        self.events = []
        self.m.emit = lambda ev, **kw: self.events.append((ev, kw))

    def prepare(self, files):
        body = json.dumps({"info": {"alias": "Phone"}, "files": files}).encode()
        h = object.__new__(self.m.Handler)
        h.headers = {"Content-Length": str(len(body))}
        h.rfile = io.BytesIO(body)
        h.replies = []
        h.reply = lambda code, body=None: h.replies.append((code, body))
        return h

    def test_not_receiving_refuses_without_asking(self):
        h = self.prepare({"f": {"fileName": "a.txt", "size": 1}})
        h.prepare()
        self.assertEqual(h.replies[0][0], 403)
        self.assertEqual(self.events, [])

    def test_nothing_is_saved_or_unlocked_until_you_say_yes(self):
        self.m.state["mode"] = "everyone"
        h = self.prepare({"f": {"fileName": "../../.bashrc", "size": 3},
                          "g": {"fileName": "invoice\u202egpj.exe", "size": 3}})
        t = threading.Thread(target=h.prepare, daemon=True)
        t.start()
        for _ in range(200):
            if self.events:
                break
            time.sleep(0.01)
        ev, data = self.events[0]
        self.assertEqual(ev, "incoming")
        # What you're asked about is what would land: cleaned names
        self.assertEqual([f["name"] for f in data["files"]], ["bashrc", "invoicegpj.exe"])
        self.assertEqual(os.listdir(self.tmp.name), [])
        self.assertEqual(self.m.sessions[data["session"]]["tokens"], {})  # no upload can start
        self.m.sessions[data["session"]]["accepted"] = False
        self.m.answers[data["session"]].set()
        t.join(5)
        self.assertEqual(h.replies[0][0], 403)
        self.assertEqual(os.listdir(self.tmp.name), [])

    def test_names_are_cleaned_like_quick_shares(self):
        from nearby.files import safe_name
        for raw in ["../../.ssh/authorized_keys", "a\nb.txt", "\u202etxt.exe", "..", ".", "", "C:\\x\\y.doc",
                    "x" * 300 + ".jpg", ".hidden"]:
            got = self.m.clean_name(raw)
            self.assertNotIn("/", got)
            self.assertFalse(got.startswith("."), raw)
            self.assertTrue(all(c.isprintable() for c in got), raw)
            self.assertLessEqual(len(got.encode()), 200)
            self.assertTrue(got)
            self.assertEqual(got, safe_name(raw, "file") if safe_name(raw, "file") else "file")
        self.assertTrue(self.m.clean_name("x" * 300 + ".jpg").endswith(".jpg"))

    def test_received_files_are_not_executable(self):
        s = dict(id="s", files={"f": dict(fileName="run.sh", size=4)}, tokens={"f": "t"}, paths=[], done=0, total=4,
                 cancelled=False, claimed=set(), touched=time.monotonic(), **{"from": "x"})
        self.m.sessions["s"] = s
        h = object.__new__(self.m.Handler)
        h.headers = {"Content-Length": "4"}
        h.rfile = io.BytesIO(b"echo")
        h.reply = lambda code, body=None: None
        h.upload({"sessionId": "s", "fileId": "f", "token": "t"})
        mode = os.stat(os.path.join(self.tmp.name, "run.sh")).st_mode
        self.assertFalse(mode & 0o111)


class QuickShareFiles(unittest.TestCase):
    def test_partial_files_are_private_and_finished_ones_not_executable(self):
        from nearby.files import FileSink
        with tempfile.TemporaryDirectory() as d:
            sink = FileSink(d, "../evil.sh", 4)
            part = sink._part
            self.assertEqual(os.stat(part).st_mode & 0o777, 0o600)
            sink.write(0, b"echo")
            sink.finish()
            self.assertEqual(os.path.dirname(sink.path), d)
            self.assertEqual(os.path.basename(sink.path), "evil.sh")
            self.assertFalse(os.stat(sink.path).st_mode & 0o111)


# ── Link sharing (an iPhone's browser) ───────────────────────────────────
class LinkShare(unittest.TestCase):
    def test_only_a_private_address(self):
        self.assertTrue(LINK.private("192.168.1.20"))
        self.assertTrue(LINK.private("10.0.0.5"))
        for addr in ("8.8.8.8", "81.2.69.1", "203.0.113.5", "127.0.0.1", "169.254.3.4", "0.0.0.0", "100.64.0.2", "fe80::1", "fd00::1", "nonsense"):
            self.assertFalse(LINK.private(addr), addr)
        links = [
            {"ifname": "docker0", "flags": ["UP"], "addr_info": [{"family": "inet", "local": "172.17.0.1"}]},
            {"ifname": "tailscale0", "flags": ["UP"], "addr_info": [{"family": "inet", "local": "100.64.0.2"}]},
            {"ifname": "wg0", "flags": ["UP"], "addr_info": [{"family": "inet", "local": "10.9.0.2"}]},
            {"ifname": "eth0", "flags": ["UP"], "addr_info": [{"family": "inet", "local": "203.0.113.5"}]},
        ]
        self.assertEqual(LINK.pick_address(links, wireless=lambda n: False), (None, None))
        links.append({"ifname": "wlan0", "flags": ["UP"], "addr_info": [{"family": "inet", "local": "192.168.1.20"}]})
        self.assertEqual(LINK.pick_address(links, wireless=lambda n: n == "wlan0"), ("192.168.1.20", "wlan0"))

    def test_the_lifetime_is_short(self):
        self.assertLessEqual(LINK.LIFETIME, 10 * 60)

    def serve(self, share):
        server = ThreadingHTTPServer(("127.0.0.1", 0), LINK.handler_for(share))
        server.daemon_threads = True
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return f"http://127.0.0.1:{server.server_address[1]}"

    def post(self, url, data):
        req = urllib.request.Request(url, data=data, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=5) as r:
                return r.status
        except urllib.error.HTTPError as e:
            return e.code

    def test_the_secret_is_needed_and_guessing_stops_it(self):
        LINK.emit = lambda obj: None
        with tempfile.TemporaryDirectory() as d:
            token = "s3cret-token-0123456789"
            share = LINK.Share("receive", token, folder=d)
            base = self.serve(share)
            self.assertEqual(self.post(base + "/wrong/upload?name=a.txt", b"x"), 404)
            self.assertEqual(os.listdir(d), [])
            for _ in range(LINK.MAX_STRANGERS):
                share.check("/guess/")
            self.assertTrue(share.stop.is_set())

    def test_an_upload_is_cleaned_never_replaces_and_isnt_executable(self):
        LINK.emit = lambda obj: None
        with tempfile.TemporaryDirectory() as d:
            Path(d, "notes.txt").write_text("mine")
            token = "tok"
            base = self.serve(LINK.Share("receive", token, folder=d))
            self.assertEqual(self.post(f"{base}/{token}/upload?name=..%2F..%2Fnotes.txt", b"theirs"), 200)
            self.assertEqual(Path(d, "notes.txt").read_text(), "mine")
            self.assertEqual(Path(d, "notes (2).txt").read_text(), "theirs")
            self.assertFalse(os.stat(Path(d, "notes (2).txt")).st_mode & 0o111)
            self.assertEqual(sorted(os.listdir(d)), ["notes (2).txt", "notes.txt"])  # (no partial left)

    def test_a_phone_never_fills_the_disk(self):
        LINK.emit = lambda obj: None
        keep = LINK.KEEP_FREE
        LINK.KEEP_FREE = shutil.disk_usage(tempfile.gettempdir()).free + 1
        try:
            with tempfile.TemporaryDirectory() as d:
                base = self.serve(LINK.Share("receive", "tok", folder=d))
                self.assertEqual(self.post(f"{base}/tok/upload?name=big.bin", b"x" * 10), 507)
                self.assertEqual(os.listdir(d), [])
        finally:
            LINK.KEEP_FREE = keep

    def test_publish_takes_a_free_name(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, "a.txt").write_text("old")
            tmp = Path(d, ".part")
            tmp.write_text("new")
            self.assertEqual(LINK.publish(str(tmp), d, "a.txt"), os.path.join(d, "a (2).txt"))
            self.assertEqual(Path(d, "a.txt").read_text(), "old")
            self.assertFalse(tmp.exists())

    def test_the_qr_code_is_private(self):
        if not shutil.which("qrencode"):
            self.skipTest("qrencode isn't installed")
        path, why = LINK.make_qr("http://192.168.1.2:53320/tok/")
        self.assertIsNone(why)
        try:
            self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)
        finally:
            os.unlink(path)


# ── Cast ─────────────────────────────────────────────────────────────────
class Cast(unittest.TestCase):
    def test_only_web_links_and_never_as_an_option(self):
        seen = {}

        def run(cmd, **kw):
            seen["cmd"] = cmd
            return subprocess.CompletedProcess(cmd, 0, "https://media.example/v.mp4\n", "")
        for bad in ("--exec=touch /tmp/x", "file:///etc/passwd", "-o/tmp/x"):
            with self.assertRaises(CAST.CastError):
                CAST.resolve(bad, run=run)
        CAST.resolve("https://video.example/watch?v=1", run=run)
        self.assertEqual(seen["cmd"][-2:], ["--", "https://video.example/watch?v=1"])

    def test_the_file_server_listens_on_one_address(self):
        with tempfile.NamedTemporaryFile(suffix=".mp4") as f:
            srv = CAST.OneFile(f.name, port=0, bind="127.0.0.1")
            try:
                self.assertEqual(srv.server_address[0], "127.0.0.1")
                self.assertGreaterEqual(len(srv.token), 20)  # (128 bits, url-safe)
            finally:
                srv.server_close()


# ── The root helper ──────────────────────────────────────────────────────
class RootHelper(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.d = Path(self.tmp.name)

    def test_priority_writes_only_its_fixed_rule_and_stays_off_through_updates(self):
        etc = self.d / "etc/ananicy.d"
        etc.mkdir(parents=True)
        rule = etc / "irice.rules"
        self.assertIn(str(rule), self.setup_written())  # (on by default)
        R.priority(False, prefix=str(self.d), run=lambda *a, **k: None)
        self.assertFalse(rule.exists())
        self.assertTrue((self.d / "etc/irice/priority-off").exists())
        self.assertNotIn(str(rule), self.setup_written())  # (an update leaves it off)
        self.assertFalse(rule.exists())
        R.priority(True, prefix=str(self.d), run=lambda *a, **k: None)
        self.assertEqual(rule.read_bytes(), R.ANANICY_TEXT)
        self.assertFalse((self.d / "etc/irice/priority-off").exists())
        # (nothing to pass it: on or off is all it takes)
        self.assertEqual(list(inspect.signature(R.priority).parameters), ["on", "prefix", "run"])

    def setup_written(self):
        src = self.d / "src"
        for rel, _dest, _mode in R.FILES:
            f = src / rel
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_bytes((ROOT / "shell" / rel).read_bytes())
        return R.setup(str(src), os.getuid(), prefix=str(self.d), run=lambda *a, **k: None)

    def test_only_fixed_jobs_run_without_a_password(self):
        # (each takes nothing free-form: fixed fixes, a checked image, a
        # checked look, the shell's priority on or off with a fixed rule, and
        # sharing's ports: only the fixed sets in PORT_SETS (sharing's own
        # ports, nothing wider), runtime rules where the firewall has them,
        # opened while you're visible and closed after. The shell asks for
        # it by itself as visibility changes, so a password each time would
        # be asked again and again; at worst it opens what Settings → Sharing
        # opens anyway, and closes it)
        rules = (ROOT / "shell/assets/polkit/49-irice.rules").read_text()
        silent = re.search(r"var silent = \{(.*?)\};", rules, re.S).group(1)
        self.assertEqual(sorted(re.findall(r'"org\.irice\.root\.([a-z-]+)"', silent)),
                         ["fix", "login-look", "priority", "sharing-ports", "wallpaper"])
        self.assertIn('subject.active && subject.local && subject.isInGroup("wheel")', rules)
        # Every action asks for an administrator by default
        tree = ET.parse(ROOT / "shell/assets/polkit/org.irice.root.policy")
        for action in tree.getroot().iter("action"):
            for k in ("allow_any", "allow_inactive", "allow_active"):
                self.assertEqual(action.find(f"defaults/{k}").text, "auth_admin", action.get("id"))
            ann = {a.get("key"): a.text for a in action.iter("annotate")}
            self.assertEqual(ann["org.freedesktop.policykit.exec.path"], "/usr/lib/irice/irice-root")

    def test_the_fixes_are_fixed(self):
        self.assertEqual(set(R.FIXES), {"networkmanager", "bluetooth", "avahi", "upower", "profiles",
                                        "btpairing", "fprintd"})
        for bad in ("firewall; reboot", "../firewall", "", "FIREWALL"):
            with self.assertRaises(R.Refused):
                R.fix_command(bad)

    def test_sharing_ports_are_only_sharings_and_never_kept(self):
        # (LocalSend 53317, Quick Share 53318, its faster link 53319, link
        # sharing and Cast 53320, mDNS 5353, DHCP 67 for a Quick Share
        # hotspot. Nothing wider.)
        ports = {n for sets in R.PORT_SETS.values() for _p, lo, hi in sets for n in range(lo, hi + 1)}
        self.assertLessEqual(ports, {53317, 53318, 53319, 53320, 5353, 67})
        # With every firewall, everything open: nothing is made to last
        # (ufw's rules last by themselves: close takes them out again)
        for kind in ("ufw", "firewalld", "nftables", "iptables"):
            calls = []

            def run(cmd, **kw):
                calls.append(" ".join(cmd))
                out, code = "", 0
                if cmd[:2] == ["ufw", "status"]:
                    out = "Status: active\n" if kind == "ufw" else ""
                elif cmd[:2] == ["firewall-cmd", "--state"]:
                    code = 0 if kind == "firewalld" else 252
                elif cmd[:2] == ["systemctl", "is-active"]:
                    code = 0 if cmd[-1] == kind else 3
                elif any(a.startswith("--query-port") for a in cmd):
                    code = 1
                elif cmd == ["nft", "list", "chains"]:
                    out = "table inet filter {\n\tchain input {\n\t\ttype filter hook input priority filter; policy drop;\n\t}\n}\n"
                return subprocess.CompletedProcess(cmd, code, out, "")
            R.sharing_ports(list(R.PORT_SETS), run=run, prefix=str(self.d))
            self.assertTrue([c for c in calls if re.search(r"ufw allow|--add-port|nft insert|tables -I ", c)], kind)
            for c in calls:
                self.assertNotRegex(c, r"--permanent --add|--reload|ufw allow from|--zone=trusted|policy accept|ufw disable|nft -f|-save", c)

    def test_sharing_ports_takes_only_open_or_close_and_fixed_names(self):
        # (main, as root: the job and the old fix's id; anything else refused)
        asked = []
        with mock.patch.object(R, "sharing_ports", lambda sets: (asked.append(sets), ([], []))[1]), \
                mock.patch.object(R.os, "geteuid", return_value=0), contextlib.redirect_stdout(io.StringIO()):
            for argv, want in ((["sharing-ports", "open", "quickshare", "mdns"], ["quickshare", "mdns"]),
                               (["sharing-ports", "open"], list(R.PORT_SETS)), (["sharing-ports", "close"], []),
                               (["fix", "firewall"], list(R.PORT_SETS))):
                self.assertEqual(R.main(argv), 0, argv)
                self.assertEqual(asked.pop(), want, argv)
            for bad in (["sharing-ports"], ["sharing-ports", "close", "quickshare"], ["sharing-ports", "on"]):
                with contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(R.main(bad), R.REFUSED_EXIT, bad)
        self.assertEqual(asked, [])

    def test_wallpaper_takes_only_a_full_path_to_an_image_file(self):
        for bad in ("relative.jpg", "/a\nb.jpg", "/a\0b", "", None):
            with self.assertRaises(R.Refused):
                R.check_image_path(bad)
        fifo = self.d / "pipe"
        os.mkfifo(fifo)
        with self.assertRaises(R.Refused):
            R.read_as(os.getuid(), str(fifo), 10)  # (never a FIFO or device: no hang, no device read)
        text = self.d / "x.jpg"
        text.write_text("#!/bin/sh\n")
        dest = self.d / "share/wallpaper.jpg"
        dest.parent.mkdir()
        with self.assertRaises(R.Refused):
            R.wallpaper(str(text), os.getuid(), dest=str(dest))
        self.assertFalse(dest.exists())

    def test_wallpaper_wont_write_through_a_linked_folder(self):
        img = self.d / "a.png"
        img.write_bytes(b"\x89PNG\r\n\x1a\n" + b"0" * 10)
        real = self.d / "elsewhere"
        real.mkdir()
        (self.d / "share").symlink_to(real)
        with self.assertRaises(R.Refused):
            R.wallpaper(str(img), os.getuid(), dest=str(self.d / "share/wallpaper.jpg"))
        self.assertEqual(os.listdir(real), [])

    def test_login_look_takes_exactly_its_values(self):
        good = {"style": "look", "look": "glass", "mode": "dark", "materialStyle": "tonalSpot", "clockFace": "dynamic",
                "clockWeight": 500, "clockWidth": 100, "seed": "#123456", "clockRound": False}
        R.check_login_look(json.dumps(good))
        for change in ({"extra": 1}, {"seed": "#12345g"}, {"seed": "#123456; rm"}, {"look": "glass\""},
                       {"clockWeight": True}, {"clockWeight": 5000}, {"clockRound": "yes"}):
            with self.assertRaises(R.Refused, msg=change):
                R.check_login_look(json.dumps(dict(good, **change)))
        with self.assertRaises(R.Refused):
            R.check_login_look("x" * 5000)

    def test_quick_share_trust_names_one_real_user(self):
        for bad in ('bob"; polkit.Result.YES; //', "Bob", "-x", "", "a" * 40, "bob\n"):
            with self.assertRaises(R.Refused):
                R.quickshare_trust_rule(bad)
        rule = R.quickshare_trust_rule("bob")
        self.assertIn('subject.user === "bob" && subject.active && subject.local', rule)
        self.assertIn('words.length !== 2', rule)  # (wifi-helper with no arguments, nothing else)
        self.assertIn('"org.irice.root.wifi-helper"', rule)

    def test_face_takes_only_a_camera(self):
        for bad in ("/dev/../etc/shadow", "/dev/video0/../../etc/shadow", "/etc/passwd", "/dev/v4l/by-id/../../sda",
                    "/dev/video", "/dev/sda"):
            self.assertIsNone(R.CAMERA_PATH.match(bad), bad)
        self.assertTrue(R.CAMERA_PATH.match("/dev/video0"))

    def test_only_pacman_runs_as_root_and_only_where_it_should(self):
        R.check_pacman(["pacman", "-S", "--needed", "--noconfirm", "--", "foo"])
        R.check_pacman(["/usr/bin/pacman", "-U", "--config", "/etc/pacman.conf", "/home/u/.cache/yay/x.pkg.tar.zst"])
        for bad in (["sh", "-c", "id"], ["/tmp/pacman", "-S", "x"], ["pacman", "-Q"], ["pacman", "-S", "-Q", "x"],
                    ["pacman", "-S", "--config", "/tmp/evil.conf", "x"], ["pacman", "-S", "--config=/tmp/e", "x"],
                    ["pacman", "-S", "--hookd=/tmp/h", "x"], ["pacman", "-S", "--hookdir", "/tmp/h", "x"],
                    ["pacman", "-Sr", "/mnt", "x"], ["pacman", "-S", "-b", "/tmp/db", "x"],
                    ["pacman", "-U", "--root=/mnt", "x"], ["pacman", "-S", "--gpgdir", "/tmp/k", "x"],
                    ["pacman", "-S", "x\0y"], "pacman -S x", []):
            with self.assertRaises(R.Refused, msg=bad):
                R.check_pacman(bad)

    def test_update_takes_package_names_only(self):
        for bad in (["--install", "-x"], ["--install", "a;b"], ["--remove", "$(id)"], ["--snapshot", "rm"],
                    ["--setup", "relative"], ["--what", "x"], ["--install"]):
            with self.assertRaises(R.Refused, msg=bad):
                R.parse_update(bad, [])

    def test_going_back_takes_package_files_from_a_cache_only(self):
        cache = self.d / "cache"
        cache.mkdir()
        outside = self.d / "outside"
        outside.mkdir()
        good = cache / "vim-9.1-1-x86_64.pkg.tar.zst"
        good.write_bytes(b"x")
        self.assertEqual(R.check_package_file(str(good), [str(cache)]), str(good))
        evil = outside / "vim-9.1-1-x86_64.pkg.tar.zst"
        evil.write_bytes(b"x")
        link = cache / "evil-1-1-x86_64.pkg.tar.zst"
        link.symlink_to(evil)  # (a link in the cache to a file outside it)
        for bad in (str(evil), str(link), str(cache / "../outside/vim-9.1-1-x86_64.pkg.tar.zst"),
                    "relative-1-1-x86_64.pkg.tar.zst", str(cache / "notes.txt"),
                    "http://archive.archlinux.org/packages/v/vim/vim-9.1-1-x86_64.pkg.tar.zst",
                    "https://evil.example/packages/v/vim/vim-9.1-1-x86_64.pkg.tar.zst"):
            with self.assertRaises(R.Refused, msg=bad):
                R.check_package_file(bad, [str(cache)])

    def test_setup_and_the_login_copy_skip_links(self):
        src = self.d / "shell"
        (src / "a").mkdir(parents=True)
        (src / "a/ok.qml").write_text("x")
        secret = self.d / "secret"
        secret.write_text("root-only")
        (src / "a/link.qml").symlink_to(secret)
        files = R.read_tree(str(src))
        self.assertEqual(sorted(files), ["a/ok.qml"])


# ── Quick Share's Wi-Fi helper (root) ────────────────────────────────────
class WifiHelper(unittest.TestCase):
    def test_a_phones_lease_stays_on_its_own_small_network(self):
        import ipaddress
        self.assertEqual(WIFI.lease_network("192.168.49.23", "255.255.255.0"), ("192.168.49.23", 24))
        # A wide mask would route the internet (or half of it) to the phone
        for mask in ("0.0.0.0", "128.0.0.0", "255.0.0.0", "255.255.0.0"):
            self.assertEqual(WIFI.lease_network("192.168.49.23", mask), ("192.168.49.23", 24))
        self.assertEqual(WIFI.lease_network("192.168.49.23", "255.255.255.128"), ("192.168.49.23", 25))
        home = [ipaddress.IPv4Network("192.168.1.0/24"), ipaddress.IPv4Network("10.0.0.0/8")]
        for ip in ("8.8.8.8", "1.1.1.1", "127.0.0.2", "169.254.1.1", "224.0.0.1", "0.0.0.0",
                   "192.168.1.77", "10.20.30.40", "192.168.49.0", "192.168.49.255", "nonsense"):
            with self.assertRaises(WIFI.Refused, msg=ip):
                WIFI.lease_network(ip, "255.255.255.0", home)
        self.assertEqual(WIFI.lease_network("192.168.49.5", "255.255.255.0", home), ("192.168.49.5", 24))

    def test_requests_are_checked(self):
        for bad in ("", "x" * 33, "a\nb", "a\0b", None, 5):
            with self.assertRaises(WIFI.Refused):
                WIFI.check_ssid(bad)
        for bad in ("short", "x" * 64, "pass\nword1", "pässword12", None):
            with self.assertRaises(WIFI.Refused):
                WIFI.check_passphrase(bad)
        for bad in ("5180; reboot", 99999, -5, 1.5, True):
            with self.assertRaises(WIFI.Refused):
                WIFI.check_freq(bad)
        for bad in ("wlan0; id", "-x" * 9, "", "a/b"):
            with self.assertRaises(WIFI.Refused):
                WIFI.check_ifname(bad)

    def test_its_hotspot_has_a_strong_password(self):
        p = WIFI.new_passphrase()
        self.assertGreaterEqual(len(p), 12)
        self.assertNotEqual(p, WIFI.new_passphrase())


# ── Private files ────────────────────────────────────────────────────────
class PrivateFiles(unittest.TestCase):
    def fresh_debuglog(self, env):
        old = dict(os.environ)
        os.environ.clear()
        os.environ.update(env)
        try:
            return load("debuglog_sec", QS / "nearby/debuglog.py")
        finally:
            os.environ.clear()
            os.environ.update(old)

    def test_the_quick_share_log_never_goes_to_a_shared_folder(self):
        base = {k: v for k, v in os.environ.items() if k != "XDG_RUNTIME_DIR"}
        self.assertIsNone(self.fresh_debuglog(dict(base, IRICE_QS_DEBUG="1"))._path)
        with tempfile.TemporaryDirectory() as d:
            os.chmod(d, 0o755)  # (others can read it: not a runtime folder)
            self.assertIsNone(self.fresh_debuglog(dict(base, IRICE_QS_DEBUG="1", XDG_RUNTIME_DIR=d))._path)

    def test_the_quick_share_log_is_private_and_never_followed(self):
        with tempfile.TemporaryDirectory() as d:
            os.chmod(d, 0o700)
            env = dict(os.environ, IRICE_QS_DEBUG="1", XDG_RUNTIME_DIR=d)
            log = self.fresh_debuglog(env)
            self.assertEqual(log._path, os.path.join(d, "irice-quickshare.log"))
            log.log("test", "hello")
            self.assertEqual(os.stat(log._path).st_mode & 0o777, 0o600)
            os.unlink(log._path)
            target = Path(d, "victim")
            target.write_text("")
            os.symlink(target, log._path)
            log.log("test", "through the link")
            self.assertEqual(target.read_text(), "")

    def test_localsends_key_is_private(self):
        if not shutil.which("openssl"):
            self.skipTest("openssl isn't installed")
        ls = load("localsend_tls", SCRIPTS / "localsend.py")
        with tempfile.TemporaryDirectory() as d:
            ls.configure_tls(d)
            folder = Path(d, "irice/localsend")
            self.assertEqual(folder.stat().st_mode & 0o777, 0o700)
            self.assertEqual((folder / "private.pem").stat().st_mode & 0o777, 0o600)

    def test_wifi_codes_stay_in_a_private_folder(self):
        wq = load("wifi_qr_sec", SCRIPTS / "wifi-qr.py")
        self.assertEqual(os.stat(wq.runtime_dir()).st_mode & 0o077, 0)
        # (forget removes only its own codes, never a path it's handed)
        with tempfile.TemporaryDirectory() as d:
            other = Path(d, "wifi-qr-0123456789ab.svg")
            other.write_text("x")
            wq.forget(str(other))
            self.assertTrue(other.exists())


# ── Backups carry no secrets ─────────────────────────────────────────────
class Backup(unittest.TestCase):
    def test_every_secret_looking_setting_is_scrubbed(self):
        sb = load("settings_backup_sec", SCRIPTS / "settings-backup.py")
        text = (ROOT / "shell/common/Config.qml").read_text()
        names = re.findall(r"property (?:string|var|list<\w+>) (\w+)", text)
        secretish = [n for n in names if re.search(r"(key|token|secret|password|passphrase|credentials?|cookie)s?$", n, re.I)]
        self.assertIn("klipyKey", secretish)
        for n in secretish:
            self.assertTrue(sb.SECRET.search(n), f"{n} looks like a secret but a backup would carry it: extend SECRET")

    def test_an_export_holds_no_secrets_and_is_private(self):
        sb = load("settings_backup_sec2", SCRIPTS / "settings-backup.py")
        with tempfile.TemporaryDirectory() as d:
            cfg = Path(d, "config")
            cfg.mkdir()
            (cfg / "config.json").write_text(json.dumps({
                "emoji": {"klipyKey": "SECRET-A"}, "google": {"taskListId": "SECRET-B"},
                "ai": {"models": [json.dumps({"id": "m", "apiKey": "SECRET-C", "token": "SECRET-D"})]},
                "x": {"password": "SECRET-E", "passphrase": "SECRET-F", "authToken": "SECRET-G"}}))
            (cfg / "calendars.json").write_text(json.dumps({"sources": [{"url": "https://cal/SECRET-H.ics"}]}))
            out = Path(d, "out")
            out.mkdir()
            f = sb.export(str(out), str(cfg))
            self.assertEqual(os.stat(f).st_mode & 0o777, 0o600)
            import tarfile
            with tarfile.open(f) as t:
                blob = b"".join(t.extractfile(i).read() for i in t if i.isreg())
                names = t.getnames()
            self.assertNotIn(b"SECRET", blob)
            self.assertNotIn("calendars.json", names)


# ── The shell (QML) ──────────────────────────────────────────────────────
def qml_files():
    for top in ("shell", "material"):
        for p in sorted((ROOT / top).rglob("*.qml")):
            if "build" not in p.parts:
                yield p


def code_lines(path):
    """The file's lines with whole-line comments blanked (numbers kept)."""
    return re.sub(r"(?m)^[ \t]*//.*$", "", path.read_text(errors="replace"))


LIT = r'''(?:"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*')'''
SH_C_DYNAMIC = re.compile(
    r'''["'](?:/(?:usr/)?bin/)?(?:ba|z|da)?sh["']\s*,\s*["']-[a-z]*c["']\s*,'''
    r'''(?!\s*''' + LIT + r'''(?:\s*\+\s*''' + LIT + r''')*\s*[,\]])''')
# `sh -c` whose script isn't one literal: each checked by hand (SECURITY-AUDIT.md),
# its data passed as "$1"…, quoted, or numbers only
SH_C_ALLOWED = {
    "shell/services/BootSnapshots.qml": 1,  # constants joined
    "shell/services/Needs.qml": 1,  # constants joined; the folder as $1
    "shell/services/Clipboard.qml": 1,  # a constant flag; the id and db as $1, $2
    "shell/services/Files.qml": 1,  # the file manager quoted (q); the path as $1
    "shell/services/ScreenTools.qml": 1,  # its runner: every caller quotes (q) or passes numbers
    "shell/services/Search.qml": 1,  # ">command": what you typed, in a terminal, on purpose
}


class Shell(unittest.TestCase):
    def test_no_shell_command_is_built_from_data(self):
        found = {}
        for p in qml_files():
            rel = str(p.relative_to(ROOT))
            n = len(SH_C_DYNAMIC.findall(code_lines(p)))
            if n:
                found[rel] = n
        self.assertEqual(found, SH_C_ALLOWED,
                         "`sh -c` with a script built from values: pass the values as \"$1\"… after the script "
                         "(… \"sh\", value), or check it by hand and list it here")

    def test_screen_text_never_reaches_trans_as_an_option(self):
        st = (ROOT / "shell/services/ScreenTools.qml").read_text()
        self.assertRegex(st, r'"trans -brief -no-ansi -t " \+ q\(t\) \+ " -- " \+ q\(src\)')
        for p in (ROOT / "shell/modules/sidebar/Translator.qml",):
            self.assertIn('"-t", target, "--", input.text]', p.read_text())

    def test_links_from_outside_open_only_on_the_web(self):
        bad = []
        for p in qml_files():
            for n, line in enumerate(code_lines(p).splitlines(), 1):
                if "onLinkActivated" in line and "Launch.openLink(" not in line:
                    bad.append(f"{p.relative_to(ROOT)}:{n}: {line.strip()}")
        self.assertEqual(bad, [], "a link in text opens with Launch.openLink (web and mail only)")

    def test_received_files_open_by_path_not_as_a_url(self):
        bad = []
        for rel in ("shell/services/QuickShare.qml", "shell/services/LinkShare.qml", "shell/services/LocalSend.qml",
                    "shell/modules/share/IncomingCard.qml", "material/modules/share/IncomingCard.qml"):
            p = ROOT / rel
            if p.exists() and re.search(r'Launch\.open\("file://"\s*\+', p.read_text()):
                bad.append(rel)
        self.assertEqual(bad, [], "a name with % or # would open another file: use Files.open(path)")

    def test_notification_bodies_are_sanitised(self):
        for rel in ("shell/modules/notifications/NotificationCard.qml",
                    "shell/modules/notifications/MaterialNotificationCard.qml"):
            self.assertIn("text: Notifications.safeMarkup(root.notification?.body)", (ROOT / rel).read_text())

    # Text showing what other people or programs wrote: never markup (Qt's
    # default, AutoText, takes <img src="http://…"> and fetches it)
    UNTRUSTED = re.compile(
        r"trackTitle|trackArtist|trackAlbum"
        r"|notification\??\.summary|modelData\.summary"
        r"|Apps\.active\??\.title|modelData\.title\b"
        r"|req\??\.from\b|peer\.modelData\.name"
        r"|Cast\.device|Cast\.title"
        r"|event\??\.(title|subtitle)\b|card\??\.(text|name)\b"
        r"|rowItem\.modelData\.text"
        r"|app\??\.description|app\??\.summary"
        r"|\bssid\b|root\.what\b")
    # Components whose own Text is plain (checked below)
    PLAIN_COMPONENTS = {"Marquee", "HoverTip", "Heading", "ListRow"}
    BLOCK = re.compile(r"^(\s*)([A-Z][A-Za-z0-9_.]*)\s*\{\s*$")

    def test_outside_text_is_shown_as_text(self):
        bad = []
        for p in qml_files():
            lines = code_lines(p).split("\n")
            for i, l in enumerate(lines):
                m = re.match(r"^(\s*)text:\s*(.*)$", l)
                if not m or not self.UNTRUSTED.search(l):
                    continue
                # the innermost enclosing `Type {` block
                typ, start, j = None, None, i - 1
                while j >= 0:
                    b = self.BLOCK.match(lines[j])
                    if b and len(b.group(1)) < len(m.group(1)):
                        typ, start = b.group(2), j
                        break
                    j -= 1
                if typ is None or typ in self.PLAIN_COMPONENTS:
                    continue
                k, own = start + 1, []
                while k < len(lines) and not (lines[k].strip() == "}" and len(lines[k]) - len(lines[k].lstrip()) == len(lines[start]) - len(lines[start].lstrip())):
                    if len(lines[k]) - len(lines[k].lstrip()) == len(m.group(1)):
                        own.append(lines[k])
                    k += 1
                if not any(re.match(r"^\s*textFormat:\s*Text\.PlainText\b", x) for x in own):
                    bad.append(f"{p.relative_to(ROOT)}:{i + 1}: {l.strip()}")
        self.assertEqual(bad, [], "add textFormat: Text.PlainText")

    def test_text_components_are_plain(self):
        for rel, n in (("shell/modules/menubar/Marquee.qml", 2), ("shell/modules/menubar/HoverTip.qml", 1),
                       ("shell/modules/controlcenter/ListRow.qml", 2)):
            self.assertGreaterEqual((ROOT / rel).read_text().count("textFormat: Text.PlainText"), n, rel)
        self.assertRegex((ROOT / "shell/modules/clipboard/ClipboardPanel.qml").read_text(), r"component Heading: Text \{\n\s*textFormat: Text\.PlainText")

    def run_js(self, source, calls):
        if not shutil.which("node"):
            self.skipTest("node isn't installed")
        r = subprocess.run(["node", "-e", source + "\nprocess.stdout.write(JSON.stringify(" + calls + "));"],
                           capture_output=True, text=True, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    @staticmethod
    def function(path, name):
        text = path.read_text()
        start = text.index(f"function {name}(")
        depth, i = 0, text.index("{", start)
        while True:
            depth += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
            if depth == 0:
                return text[start:i]

    def test_the_markup_and_link_checks(self):
        launch = ROOT / "shell/common/Launch.qml"
        notes = ROOT / "shell/services/Notifications.qml"
        js = (self.function(launch, "linkAllowed") + "\nconst Launch = { linkAllowed };\n"
              + self.function(notes, "safeMarkup") + "\n" + self.function(notes, "plainBody"))
        inputs = ["<b>Hi</b> <i>there</i><br/>", '<img src="http://tracker.example/p.png">seen',
                  '<a href="https://ok.example/x">ok</a>', '<a href="file:///home/u/x.desktop">run</a>',
                  "<a href='javascript:alert(1)'>x</a>", '<font color="red">r</font><IMG SRC=x>',
                  '<a href="https://a.example/" onclick="x">y</a>', "1 < 2 and 3 > 2", None]
        out = self.run_js(js, "[" + ",".join(f"safeMarkup({json.dumps(x)})" for x in inputs) + "]")
        self.assertEqual(out[0], "<b>Hi</b> <i>there</i><br>")
        self.assertEqual(out[1], "seen")
        self.assertEqual(out[2], '<a href="https://ok.example/x">ok</a>')
        self.assertEqual(out[3], "<a>run</a>")
        self.assertEqual(out[4], "<a>x</a>")
        self.assertEqual(out[5], "r")
        self.assertEqual(out[6], '<a href="https://a.example/">y</a>')
        self.assertEqual(out[7], "1 < 2 and 3 > 2")
        self.assertEqual(out[8], "")
        links = ["https://x.example", "HTTP://x", "mailto:a@b.c", "file:///etc/passwd", "smb://host/share",
                 "javascript:alert(1)", "steam://run/1", " https://x.example", "", None, "/home/u/x.desktop"]
        allowed = self.run_js(js, "[" + ",".join(f"linkAllowed({json.dumps(x)})" for x in links) + "]")
        self.assertEqual(allowed, [True, True, True, False, False, False, False, True, False, False, False])
        plain = self.run_js(js, 'plainBody("<b>a</b> &amp;lt; &lt;b&gt;")')
        self.assertEqual(plain, "a &lt; <b>")


if __name__ == "__main__":
    unittest.main()
