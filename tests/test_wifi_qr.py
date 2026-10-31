"""wifi-qr.py: a saved network as a WIFI: QR code (with a stand-in nmcli and
the real qrencode), and Wi-Fi codes read back out of an image (zbar)."""
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/wifi-qr.py"
spec = importlib.util.spec_from_file_location("wifi_qr", SCRIPT)
wq = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wq)

FAKE_NMCLI = r'''#!/usr/bin/env python3
import json, os, sys
st = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "state.json")))
a = [x for x in sys.argv[1:] if x not in ("--escape", "no")]
if a[:5] == ["-t", "-f", "NAME,UUID,TYPE,ACTIVE", "connection", "show"]:
    for c in st["conns"]:
        print(f"{c.get('name', c['ssid'])}:{c['uuid']}:802-11-wireless:{'yes' if c.get('active') else 'no'}")
    sys.exit(0)
secret = a[0] == "-s"
if secret:
    a = a[1:]
field, uuid = a[1], a[-1]
c = next(c for c in st["conns"] if c["uuid"] == uuid)
if field == "802-11-wireless-security.psk":
    print(c.get("psk", "") if secret else "")
elif field == "802-11-wireless-security.wep-key0":
    print(c.get("wep", "") if secret else "")
else:
    print({"802-11-wireless.ssid": c["ssid"], "802-11-wireless-security.key-mgmt": c.get("km", "wpa-psk"),
           "802-11-wireless.hidden": "yes" if c.get("hidden") else "no"}[field])
'''


class Text(unittest.TestCase):
    def test_escaping_round_trip(self):
        t = wq.wifi_text('Café; "Home":5G', "WPA", "pa\\ss;word,1", hidden=True)
        self.assertEqual(t, 'WIFI:T:WPA;S:Café\; \\"Home\\"\\:5G;P:pa\\\\ss\;word\\,1;H:true;;')
        self.assertEqual(wq.parse_wifi(t), {"ssid": 'Café; "Home":5G', "security": "WPA",
                                            "password": "pa\\ss;word,1", "hidden": True})

    def test_open_network_has_no_password(self):
        self.assertEqual(wq.wifi_text("Cafe", "nopass", "ignored"), "WIFI:T:nopass;S:Cafe;;")
        self.assertEqual(wq.parse_wifi("WIFI:T:nopass;S:Cafe;;")["security"], "nopass")

    def test_not_wifi_or_broken(self):
        self.assertIsNone(wq.parse_wifi("https://example.com"))
        self.assertIsNone(wq.parse_wifi("WIFI:T:WPA;P:x;;"))  # no name
        self.assertIsNone(wq.parse_wifi("WIFI:S:" + "x" * 40 + ";;"))  # too long a name

    def test_security_from_networkmanager(self):
        self.assertEqual([wq.security_of(k) for k in ("wpa-psk", "sae", "none", "", "wpa-eap")],
                         ["WPA", "SAE", "nopass", "nopass", None])


@unittest.skipUnless(shutil.which("qrencode"), "needs qrencode")
class Share(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        d = Path(self.dir.name)
        (d / "bin").mkdir()
        (d / "bin/nmcli").write_text(FAKE_NMCLI)
        (d / "bin/nmcli").chmod(0o755)
        self.state = d / "bin/state.json"
        self.env = dict(os.environ, PATH=f"{d / 'bin'}:/usr/bin:/bin", XDG_RUNTIME_DIR=str(d / "run"))
        (d / "run").mkdir(mode=0o700)

    def tearDown(self):
        self.dir.cleanup()

    def run_script(self, *args):
        p = subprocess.run(["python3", "-I", str(SCRIPT), *args], env=self.env, capture_output=True, text=True, timeout=30)
        return json.loads(p.stdout)

    def conns(self, *conns):
        self.state.write_text(json.dumps({"conns": list(conns)}))

    def test_the_active_profile_becomes_a_private_code(self):
        self.conns({"uuid": "u-old", "ssid": "Home", "psk": "oldpassword"},
                   {"uuid": "u-now", "ssid": "Home", "psk": "rightpassword", "active": True})
        r = self.run_script("share", "Home")
        self.assertEqual((r["ok"], r["security"], r["password"]), (True, "WPA", "rightpassword"))
        qr = Path(r["qr"])
        self.assertTrue(qr.read_text().lstrip().startswith("<?xml"))
        self.assertEqual(qr.stat().st_mode & 0o777, 0o600)
        self.assertEqual(qr.parent, Path(self.dir.name, "run/irice"))
        self.run_script("forget", str(qr))
        self.assertFalse(qr.exists())

    def test_profile_names_with_colons_and_wep(self):
        self.conns({"uuid": "u1", "ssid": "Lab", "name": "Lab: 2nd floor", "km": "none", "wep": "abcde"},
                   {"uuid": "u2", "ssid": "Cafe", "km": "none"})
        r = self.run_script("share", "Lab")
        self.assertEqual((r["ok"], r["security"], r["password"]), (True, "WEP", "abcde"))
        r = self.run_script("share", "Cafe")
        self.assertEqual((r["ok"], r["security"], r["password"]), (True, "nopass", ""))

    def test_forget_touches_only_its_own_codes(self):
        other = Path(self.dir.name, "run/keep.svg")
        other.write_text("x")
        self.run_script("forget", str(other))
        self.assertTrue(other.exists())

    def test_enterprise_and_unknown_networks_are_refused(self):
        self.conns({"uuid": "u1", "ssid": "eduroam", "km": "wpa-eap"})
        r = self.run_script("share", "eduroam")
        self.assertFalse(r["ok"])
        self.assertIn("username", r["error"])
        self.assertFalse(self.run_script("share", "Elsewhere")["ok"])

    def test_a_withheld_password_says_so(self):
        self.conns({"uuid": "u1", "ssid": "Home", "psk": ""})
        r = self.run_script("share", "Home")
        self.assertFalse(r["ok"])
        self.assertIn("password", r["error"])

    @unittest.skipUnless(shutil.which("zbarimg"), "needs zbar")
    def test_a_code_in_a_screenshot_is_found(self):
        png = Path(self.dir.name, "shot.png")
        subprocess.run(["qrencode", "-t", "PNG", "-s", "6", "-o", str(png)],
                       input=wq.wifi_text("Café 5G", "SAE", "s3cret-pass"), text=True, check=True)
        r = self.run_script("scan", str(png))
        self.assertEqual(r["found"], [{"ssid": "Café 5G", "security": "SAE", "password": "s3cret-pass", "hidden": False}])

    @unittest.skipUnless(shutil.which("zbarimg"), "needs zbar")
    def test_other_codes_are_ignored(self):
        png = Path(self.dir.name, "shot.png")
        subprocess.run(["qrencode", "-t", "PNG", "-o", str(png), "https://example.com"], check=True)
        self.assertEqual(self.run_script("scan", str(png))["found"], [])


if __name__ == "__main__":
    unittest.main()
