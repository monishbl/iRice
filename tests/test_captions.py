"""captions.py: how the sound is split into sentences (when to look at the
one being heard, when it's finished, and silence costing nothing)."""
import importlib.util
import struct
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/captions.py"
spec = importlib.util.spec_from_file_location("captions", SCRIPT)
cap = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cap)

SILENT = bytes(cap.CHUNK)
LOUD = struct.pack(f"<{cap.CHUNK // 2}h", *([4000, -4000] * (cap.CHUNK // 4)))


class Splitting(unittest.TestCase):
    def feed(self, seg, chunk, n):
        return [seg.feed(chunk, cap.rms(chunk)) for _ in range(n)]

    def test_loudness(self):
        self.assertEqual(cap.rms(SILENT), 0)
        self.assertGreater(cap.rms(LOUD), cap.QUIET)

    def test_silence_is_never_sent(self):
        seg = cap.Segmenter()
        said = self.feed(seg, SILENT, 100)  # 10 s
        self.assertNotIn("partial", said)
        self.assertNotIn("final", said)
        self.assertEqual(said.count("quiet"), 1)  # (said once)
        self.assertLessEqual(len(seg.audio), int(0.5 * cap.RATE) * 2)  # (only a lead-in is kept)

    def test_a_sentence_then_a_pause(self):
        seg = cap.Segmenter()
        said = self.feed(seg, LOUD, 25)  # 2.5 s of speech
        self.assertEqual(said.count("partial"), 2)  # (every 1.2 s)
        said = self.feed(seg, SILENT, 7)
        self.assertEqual(said[-1], "final")  # (after 0.7 s of quiet)
        audio = seg.take()
        self.assertAlmostEqual(len(audio) / 2 / cap.RATE, 2.5 + 0.7, delta=0.15)
        self.assertEqual(len(seg.audio), 0)

    def test_a_short_pause_early_on_doesnt_end_it(self):
        seg = cap.Segmenter()
        self.feed(seg, LOUD, 10)
        said = self.feed(seg, SILENT, 10)  # (1 s of quiet after 1 s of speech)
        self.assertNotIn("final", said)

    def test_long_speech_is_cut(self):
        seg = cap.Segmenter()
        said = self.feed(seg, LOUD, 80)
        self.assertEqual(said[-1], "final")  # (at 8 s)


if __name__ == "__main__":
    unittest.main()
