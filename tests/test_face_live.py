"""face_live.py: the blink check for regular cameras, from EAR series (no
camera, no OpenCV): a real blink passes; a photo (flat, noisy, tilted,
eyes shut) doesn't; the eye model's download is checked."""
import hashlib
import importlib.util
import io
import json
import math
import os
import random
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/face_live.py"
spec = importlib.util.spec_from_file_location("face_live", SCRIPT)
fl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fl)

FPS = 30.0


def series(ears, fps=FPS, shapes=None, t0=0.0):
    out = []
    for i, e in enumerate(ears):
        t = t0 + i / fps
        if shapes is None:
            out.append((t, e))
        else:
            out.append((t, e, shapes[i]))
    return out


def noisy(level, n, sd=0.01, seed=1):
    r = random.Random(seed)
    return [level + r.gauss(0, sd) for _ in range(n)]


def blink(open_ear=0.30, closed_ear=0.08, before=20, shut=5, after=10, sd=0.01, seed=1):
    """open, a few frames shut (with the in-between frames), open again"""
    return noisy(open_ear, before, sd, seed) + [open_ear * 0.6] + [closed_ear] * shut + [open_ear * 0.6] + noisy(open_ear, after, sd, seed + 1)


class Ear(unittest.TestCase):
    def test_open_and_closed_eye(self):
        # corner, two upper lid points, corner, two lower lid points
        open_eye = [(0, 0), (3, -2), (7, -2), (10, 0), (7, 2), (3, 2)]
        shut_eye = [(0, 0), (3, -0.3), (7, -0.3), (10, 0), (7, 0.3), (3, 0.3)]
        self.assertAlmostEqual(fl.ear(open_eye), 0.4)
        self.assertAlmostEqual(fl.ear(shut_eye), 0.06)
        self.assertEqual(fl.ear([(0, 0)] * 6), 0.0)

    def test_measure_from_68_points(self):
        pts = [(0.0, 0.0)] * 68
        pts[36:42] = [(0, 0), (3, -2), (7, -2), (10, 0), (7, 2), (3, 2)]
        pts[42:48] = [(30, 0), (33, -2), (37, -2), (40, 0), (37, 2), (33, 2)]
        pts[30] = (20, 30)
        e, shape = fl.measure(pts)
        self.assertAlmostEqual(e, 0.4)
        self.assertAlmostEqual(shape, 1.0)  # (30 down over 30 across)
        self.assertIsNone(fl.measure(pts[:40]))


class Blinks(unittest.TestCase):
    def test_a_blink(self):
        self.assertTrue(fl.blinked(series(blink())))

    def test_narrow_eyes_blink_too(self):
        # (the open level is learnt: 0.2 open, 0.12 shut)
        self.assertTrue(fl.blinked(series(blink(open_ear=0.20, closed_ear=0.12, sd=0.005))))

    def test_a_quick_blink_at_15_fps(self):
        self.assertTrue(fl.blinked(series(blink(before=8, shut=1, after=4), fps=15)))

    def test_a_photo_is_flat(self):
        self.assertFalse(fl.blinked(series([0.30] * 180)))

    def test_a_photo_with_noise(self):
        # (a handheld photo: landmarks jitter, a few percent)
        self.assertFalse(fl.blinked(series(noisy(0.30, 180, sd=0.02, seed=7))))

    def test_eyes_shut_throughout(self):
        self.assertFalse(fl.blinked(series(noisy(0.07, 120, sd=0.01))))

    def test_eyes_that_close_and_stay_shut(self):
        self.assertFalse(fl.blinked(series(noisy(0.3, 20) + [0.07] * 90)))

    def test_too_slow_is_not_a_blink(self):
        # (shut for 1.5 s: someone swapping photos, not a blink)
        self.assertFalse(fl.blinked(series(blink(shut=45))))

    def test_a_tilted_photo_is_not_a_blink(self):
        # tilting a photo squashes its EAR and its shape alike
        n = 40
        ears, shapes = [], []
        for i in range(n):
            k = math.cos(math.radians(50)) if 20 <= i < 25 else 1.0
            ears.append(0.30 * k)
            shapes.append(1.0 * k)
        self.assertFalse(fl.blinked(series(ears, shapes=shapes)))
        # ...while a blink keeps the face's shape
        self.assertTrue(fl.blinked(series(blink(), shapes=[1.0 + 0.01 * ((i % 3) - 1) for i in range(len(blink()))])))

    def test_losing_the_face_starts_over(self):
        # open frames, the face gone (a swap), then something "reopens"
        s = series(noisy(0.3, 20) + [0.08] * 2) + [(0.75 + i / FPS, None) for i in range(20)]
        s += series([0.3] * 10, t0=1.5)
        self.assertFalse(fl.blinked(s))

    def test_a_blink_after_waiting(self):
        # (open for a while first, then a blink: still counts)
        self.assertTrue(fl.blinked(series(noisy(0.3, 120) + blink(before=0))))

    def test_a_persons_own_closing(self):
        # deep blinks: halfway down to them; shallow ones: the photo-safe limit
        self.assertEqual(fl.personal_closing(0.2), 0.6)
        self.assertEqual(fl.personal_closing(0.0), 0.5)
        self.assertEqual(fl.personal_closing(0.6), fl.CLOSED)
        # blinks that barely show: not added (a photo's jitter looks the same)
        self.assertIsNone(fl.personal_closing(0.8))
        # never looser than CLOSED, whatever enrolment kept
        self.assertEqual(fl.Blink(0.95).closed, fl.CLOSED)

    def test_a_tighter_closing_misses_a_shallow_blink(self):
        shallow = series(blink(open_ear=0.30, closed_ear=0.20))  # (down to 67%)
        self.assertTrue(fl.blinked(shallow))
        self.assertFalse(fl.blinked(shallow, closed=0.5))

    def test_the_dip_is_measured(self):
        b = fl.Blink()
        for x in series(blink(open_ear=0.30, closed_ear=0.09, sd=0.0)):
            b.feed(*x)
        self.assertTrue(b.blinked)
        self.assertAlmostEqual(b.dip, 0.3, places=2)

    def test_iou(self):
        self.assertAlmostEqual(fl.iou((0, 0, 10, 10), (0, 0, 10, 10)), 1.0)
        self.assertEqual(fl.iou((0, 0, 10, 10), (20, 20, 5, 5)), 0.0)
        self.assertAlmostEqual(fl.iou((0, 0, 10, 10), (5, 0, 10, 10)), 50 / 150)


class Response(io.BytesIO):
    def __init__(self, data):
        super().__init__(data)
        self.headers = {"Content-Length": str(len(data))}

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()


class Files(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.old = os.environ.get("XDG_DATA_HOME")
        os.environ["XDG_DATA_HOME"] = self.tmp.name

    def tearDown(self):
        if self.old is None:
            os.environ.pop("XDG_DATA_HOME", None)
        else:
            os.environ["XDG_DATA_HOME"] = self.old
        self.tmp.cleanup()

    def test_where_it_keeps_things(self):
        self.assertEqual(fl.data_dir(), os.path.join(self.tmp.name, "tessera/face"))
        self.assertEqual(fl.enrolled(), 0)
        os.makedirs(fl.data_dir())
        Path(fl.me_file()).write_text(json.dumps({"version": 1, "features": [[0.1] * 128, [0.2] * 128]}))
        self.assertEqual(fl.enrolled(), 2)
        fl.clear()
        self.assertEqual(fl.enrolled(), 0)
        fl.clear()  # (twice is fine)

    def test_download_checks_the_checksum(self):
        data = b"model bytes" * 1000
        good = hashlib.sha256(data).hexdigest()
        dest = os.path.join(self.tmp.name, "m/lbf.yaml")
        self.assertEqual(fl.download("x", good, dest, opener=lambda u, timeout: Response(data)), "")
        self.assertEqual(Path(dest).read_bytes(), data)
        self.assertEqual(os.stat(os.path.dirname(dest)).st_mode & 0o777, 0o700)
        bad = os.path.join(self.tmp.name, "m/bad.yaml")
        err = fl.download("x", "0" * 64, bad, opener=lambda u, timeout: Response(data))
        self.assertIn("checksum", err)
        self.assertFalse(os.path.exists(bad))
        self.assertFalse(os.path.exists(bad + ".part"))

    def test_download_fails_cleanly_offline(self):
        def offline(u, timeout):
            raise OSError("no network")
        dest = os.path.join(self.tmp.name, "lbf.yaml")
        self.assertIn("internet", fl.download("x", "0" * 64, dest, opener=offline))
        self.assertFalse(os.path.exists(dest + ".part"))

    def test_the_model_is_pinned(self):
        self.assertRegex(fl.EYE_MODEL_SHA256, r"^[0-9a-f]{64}$")
        self.assertRegex(fl.EYE_MODEL_URL, r"/[0-9a-f]{40}/")  # (a commit, not a branch)
        self.assertFalse(fl.eye_model_ok(os.path.join(self.tmp.name, "nothing")))

    def test_check_without_a_camera_says_so(self):
        self.assertEqual(fl.main(["face_live.py", "check"]), 2)
        self.assertEqual(fl.main(["face_live.py", "nonsense"]), 2)


if __name__ == "__main__":
    unittest.main()
