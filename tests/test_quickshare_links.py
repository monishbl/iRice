"""Quick Share's link memory (nearby/links.py): fastest first, a link that
keeps failing with one phone skipped for it for a day."""
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "shell/assets/scripts/quickshare"))
from nearby import links  # noqa: E402


class LinkMemory(unittest.TestCase):
    def setUp(self):
        self.now = 1000.0
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, "irice", "quickshare-links.json")
        self.m = links.LinkMemory(self.path, clock=lambda: self.now)

    def tearDown(self):
        self.dir.cleanup()

    def test_two_failures_in_a_row_skip_it_for_a_day(self):
        self.m.failed("Pixel", "lan")
        self.assertFalse(self.m.skip("Pixel", "lan"))  # (one failure: still tried)
        self.m.failed("Pixel", "lan")
        self.assertTrue(self.m.skip("Pixel", "lan"))
        self.assertFalse(self.m.skip("Galaxy", "lan"))  # (only for that phone)
        self.now += links.SKIP_FOR + 1
        self.assertFalse(self.m.skip("Pixel", "lan"))

    def test_a_success_forgets_the_failures(self):
        self.m.failed("Pixel", "direct")
        self.m.worked("Pixel", "direct")
        self.m.failed("Pixel", "direct")
        self.assertFalse(self.m.skip("Pixel", "direct"))

    def test_order_keeps_fastest_first_and_never_leaves_nothing(self):
        for _ in range(2):
            self.m.failed("Pixel", "lan")
        self.assertEqual(self.m.order("Pixel", ["lan", "direct", "hotspot"]), ["direct", "hotspot"])
        self.assertEqual(self.m.order("Pixel", [("lan", 1)], key=lambda t: t[0]), [("lan", 1)])

    def test_kept_on_disk_privately(self):
        self.m.failed("Pixel", "lan")
        self.m.failed("Pixel", "lan")
        self.assertEqual(stat.S_IMODE(os.stat(self.path).st_mode), 0o600)
        again = links.LinkMemory(self.path, clock=lambda: self.now)
        self.assertTrue(again.skip("Pixel", "lan"))

    def test_a_damaged_file_is_ignored(self):
        os.makedirs(os.path.dirname(self.path))
        Path(self.path).write_text("{not json")
        self.assertFalse(links.LinkMemory(self.path).skip("Pixel", "lan"))

    def test_no_path_keeps_nothing_on_disk(self):
        m = links.LinkMemory(path="")
        m.failed("Pixel", "lan")
        m.failed("Pixel", "lan")
        self.assertTrue(m.skip("Pixel", "lan"))
        self.assertFalse(os.path.exists(self.path))


if __name__ == "__main__":
    unittest.main()
