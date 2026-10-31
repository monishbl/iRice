"""phone.py: phones from `adb track-devices -l`, with their name, Android
version and battery (a stand-in adb: no phone is touched)."""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/phone.py"
spec = importlib.util.spec_from_file_location("phone", SCRIPT)
ph = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ph)

FAKE_ADB = r'''#!/usr/bin/env python3
import sys
a = sys.argv[1:]
if a[:1] == ["track-devices"]:
    block = "R5CT1234\tdevice usb:1-2 product:RMX3371 model:RMX3371 device:RE5473 transport_id:3\nZY22\tunauthorized usb:1-3 transport_id:4\n"
    sys.stdout.write("%04x%s" % (len(block), block)); sys.stdout.flush()
elif a[:2] == ["-s", "R5CT1234"] and a[2] == "shell":
    cmd = " ".join(a[3:])
    if "getprop" in cmd:
        # (no device name set, no ro.product.marketname: Realme's own property)
        print("null\n\nrealme GT Master Edition\n\nRMX3371\n14")
    elif "dumpsys battery" in cmd:
        print("Current Battery Service state:\n  AC powered: false\n  status: 2\n  level: 76\n")
'''


class Track(unittest.TestCase):
    def test_parse(self):
        d = ph.parse_track("R5CT1234\tdevice usb:1-2 model:Pixel_8 transport_id:3\n192.168.1.7:37201\tdevice product:x model:SM_S911B\n")
        self.assertEqual(d[0], {"serial": "R5CT1234", "state": "device", "model": "Pixel 8", "wireless": False})
        self.assertTrue(d[1]["wireless"])

    def test_watch(self):
        with tempfile.TemporaryDirectory() as t:
            fake = Path(t, "adb")
            fake.write_text(FAKE_ADB)
            fake.chmod(0o755)
            r = subprocess.run([sys.executable, "-I", str(SCRIPT), "watch"], capture_output=True, text=True, timeout=30,
                               env=dict(os.environ, IRICE_ADB=str(fake)))
            first = json.loads(r.stdout.splitlines()[0])
            ok, locked = first["devices"]
            self.assertEqual(ok["model"], "realme GT Master Edition")
            self.assertEqual(ok["android"], "14")
            self.assertEqual(ok["battery"], 76)
            self.assertTrue(ok["charging"])
            self.assertEqual(locked["state"], "unauthorized")
            self.assertNotIn("battery", locked)


if __name__ == "__main__":
    unittest.main()
