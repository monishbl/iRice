"""clipboard-previews.py: previews of entries no longer listed are removed
(the folder is in RAM, and every image copied used to stay there)."""
import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/clipboard-previews.py"
spec = importlib.util.spec_from_file_location("clipboard_previews", SCRIPT)
cp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cp)


class Prune(unittest.TestCase):
    def test_only_listed_previews_stay(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            for name in ("10.png", "11.png", "12.jpg", "tmpabc"):
                (d / name).write_bytes(b"x")
            cp.prune(d, {str(d / "11.png")})
            self.assertEqual(sorted(p.name for p in d.iterdir()), ["11.png", "tmpabc"])  # (a decode in progress stays)

    def test_nothing_listed_empties_it(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            (d / "10.png").write_bytes(b"x")
            cp.previews("/nonexistent.db", str(d), [])
            self.assertEqual(list(d.iterdir()), [])
