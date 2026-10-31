"""fastpair.py: Fast Pair service data (Google's public spec): a model ID in
pairing mode, and batteries (left, right, case) from the other adverts."""
import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/fastpair.py"
spec = importlib.util.spec_from_file_location("fastpair", SCRIPT)
fp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fp)


class Parse(unittest.TestCase):
    def test_pairing_mode(self):
        self.assertEqual(fp.parse(bytes([0x2C, 0xC1, 0x6A])), {"model": "2CC16A", "pairing": True})

    def test_batteries_after_the_key_filter(self):
        # flags, account key filter (len 4, type 0), salt (len 1, type 1), battery (len 3, type 3)
        data = bytes([0x00, 0x40, 1, 2, 3, 4, 0x11, 0x99, 0x33, 0x80 | 85, 70, 0x7F])
        r = fp.parse(data)
        self.assertFalse(r["pairing"])
        self.assertEqual(r["battery"], {"left": {"level": 85, "charging": True}, "right": {"level": 70, "charging": False}})
        self.assertTrue(r["showUi"])

    def test_hidden_battery(self):
        r = fp.parse(bytes([0x00, 0x34, 50, 50, 0x80 | 20]))
        self.assertFalse(r["showUi"])
        self.assertEqual(r["battery"]["case"], {"level": 20, "charging": True})

    def test_no_battery(self):
        self.assertEqual(fp.parse(bytes([0x00, 0x20, 9, 9])), {"pairing": False})

    def test_truncated(self):
        self.assertEqual(fp.parse(bytes([0x00, 0x00, 0x33, 60])), {"pairing": False, "battery": {"left": {"level": 60, "charging": False}}, "showUi": True})


if __name__ == "__main__":
    unittest.main()
