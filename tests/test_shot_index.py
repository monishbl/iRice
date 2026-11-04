"""shot-index.py: the words in your screenshots for Search, read by tesseract
on this computer (a stand-in tesseract here): only shots it hasn't read,
newest first, gone ones dropped, the index owner-only and never half
written, and the languages passed only as tesseract codes."""
import importlib.util
import json
import os
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/shot-index.py"
spec = importlib.util.spec_from_file_location("shot_index", SCRIPT)
si = importlib.util.module_from_spec(spec)
spec.loader.exec_module(si)

# Prints "words of <name>" and logs each call (its arguments) to calls.log
FAKE_TESSERACT = r'''#!/usr/bin/env python3
import os, sys
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "calls.log"), "a") as f:
    f.write("|".join(sys.argv[1:]) + "\n")
name = os.path.basename(sys.argv[1])
if name.startswith("broken"):
    sys.exit(1)
print("words   of\n\n  " + name + "  ")
'''


class Index(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.shots, self.bin, self.index = t / "Screenshots", t / "bin", t / "state" / "tessera" / "screenshot-text.json"
        self.shots.mkdir()
        self.bin.mkdir()
        fake = self.bin / "tesseract"
        fake.write_text(FAKE_TESSERACT)
        fake.chmod(0o755)
        self.old = time.time() - 3600

    def tearDown(self):
        self.tmp.cleanup()

    def shot(self, name, age=0):
        p = self.shots / name
        p.write_bytes(b"\x89PNG")
        os.utime(p, (self.old - age, self.old - age))
        return p

    def run_index(self, langs="deu+eng"):
        env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}")
        r = subprocess.run([sys.executable, "-I", str(SCRIPT), "run", str(self.shots), str(self.index), langs],
                           capture_output=True, text=True, env=env, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def calls(self):
        log = self.bin / "calls.log"
        return log.read_text().splitlines() if log.exists() else []

    def test_reads_only_new_shots_newest_first_and_drops_gone_ones(self):
        a, b = self.shot("Screenshot a.png", age=10), self.shot("Screenshot b.png")
        self.shot("notes.txt")
        self.assertEqual(self.run_index(), {"ok": True, "added": 2, "total": 2})
        self.assertEqual([c.split("|")[0] for c in self.calls()], [str(b), str(a)])
        self.assertEqual(self.calls()[0], f"{b}|stdout|-l|deu+eng")
        shots = json.loads(self.index.read_text())["shots"]
        self.assertEqual(shots[str(a)]["t"], "words of\nScreenshot a.png")
        self.assertEqual(shots[str(a)]["m"], int(a.stat().st_mtime))
        # Again: nothing new, nothing read
        self.assertEqual(self.run_index(), {"ok": True, "added": 0, "total": 2})
        self.assertEqual(len(self.calls()), 2)
        # A new one, and one gone
        a.unlink()
        self.shot("Screenshot c.jpg")
        self.assertEqual(self.run_index(), {"ok": True, "added": 1, "total": 2})
        self.assertEqual(sorted(json.loads(self.index.read_text())["shots"]), [str(b), str(self.shots / "Screenshot c.jpg")])

    def test_a_changed_shot_is_read_again_an_unreadable_one_later(self):
        a = self.shot("Screenshot a.png")
        self.shot("broken.png")
        self.assertEqual(self.run_index()["total"], 1)
        os.utime(a, (self.old + 60, self.old + 60))
        self.assertEqual(self.run_index()["added"], 1)
        self.assertEqual(sum("broken.png" in c for c in self.calls()), 2)

    def test_a_shot_still_being_written_waits(self):
        p = self.shots / "Screenshot now.png"
        p.write_bytes(b"\x89PNG")
        self.assertEqual(self.run_index(), {"ok": True, "added": 0, "total": 0})
        # (one dated ahead of the clock isn't new: it's read)
        os.utime(p, (time.time() + 3600, time.time() + 3600))
        self.assertEqual(self.run_index(), {"ok": True, "added": 1, "total": 1})

    def test_the_index_is_private_and_replaced_whole(self):
        self.shot("Screenshot a.png")
        self.run_index()
        self.assertEqual(stat.S_IMODE(self.index.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(self.index.parent.stat().st_mode), 0o700)
        self.assertFalse(Path(str(self.index) + ".tmp").exists())
        # (an old temporary file with loose bits doesn't leak them)
        tmp = Path(str(self.index) + ".tmp")
        tmp.write_text("{}")
        tmp.chmod(0o644)
        self.shot("Screenshot b.png")
        self.run_index()
        self.assertEqual(stat.S_IMODE(self.index.stat().st_mode), 0o600)

    def test_only_tesseract_codes_reach_it_as_languages(self):
        self.shot("Screenshot a.png")
        self.run_index(langs="--psm 0; rm")
        self.assertTrue(self.calls()[0].endswith("|stdout|-l|eng"), self.calls())

    def test_a_broken_index_starts_over(self):
        self.index.parent.mkdir(parents=True)
        self.index.write_text("{not json")
        self.shot("Screenshot a.png")
        self.assertEqual(self.run_index()["total"], 1)

    def test_text_is_tidied_and_capped(self):
        self.assertEqual(si.clean("  a \t b \n\n\n c  "), "a b\nc")
        self.assertEqual(len(si.clean("x" * (si.MAX_TEXT + 50))), si.MAX_TEXT)


if __name__ == "__main__":
    unittest.main()
