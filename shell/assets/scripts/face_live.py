#!/usr/bin/env python3
"""Face unlock with a regular (colour) camera: the blink check.

Howdy (howdy-next) matches the face; a regular camera would also match a
photo, so with one the lock asks for a blink as well. Its PAM file
(shell/assets/pam/tessera-face-rgb) runs Howdy first, then this, as you,
through pam_exec (`tessera-ui --face-live`, which runs `face_live.py check`).
An infrared camera doesn't need it (a photo doesn't show up in infrared).

  face_live.py check [--device DEV] [--timeout S]
      watch the camera: the same face throughout (it must be the one added
      here), its eyes open, then closed, then open again within 3 s. Exit 0
      when it blinked, 1 when not (no face, someone else, a photo, too
      slow), 2 when it can't look (no camera, no models, no OpenCV). Lines
      for the lock start "Tessera: " (pam_exec passes them on as PAM info).
  face_live.py enrol [--device DEV]
      remember your face for the check (a few frames' SFace features) and
      your blink (how far your EAR drops: personal_closing), kept in
      ~/.local/share/tessera/face/me.json, yours only (0600). Blinks that
      don't show with this camera: not added (exit 1, said why)
  face_live.py clear      forget it
  face_live.py models     download the eye-landmark model, once (SHA-256
                          pinned below); lines "PROGRESS <0-100>"
  face_live.py status     JSON: {"cv2", "eyeModel", "howdyModels", "enrolled"}
  face_live.py check --admin
      administrator prompts (opt-in; tessera-root face admin on): polkit's
      helper runs this as root for PAM_USER, from root's copies in
      /var/lib/tessera/face (the camera, the face, the eye model)

Nothing leaves this computer; the only download is the model below.

The blink: OpenCV's YuNet finds the face (Howdy's own model file), its
68-point LBF facemark model places the eyelids, and the eye aspect ratio
(EAR: the eyelids' gap over the eye's width; Soukupová and Čech, 2016)
drops when the eyes close. The open level is learnt from the person in
front of the camera, so it works for narrow and wide eyes alike; a photo's
EAR stays flat (only noise), so it never blinks.
"""
import glob
import hashlib
import json
import math
import os
import pwd
import sys
import time

# The 68-point LBF facemark model (OpenCV GSoC 2017, used by cv2.face's
# FacemarkLBF), pinned to a commit and its SHA-256. The smallest 68-point
# model OpenCV reads without another runtime: dlib's is 99 MB and needs dlib.
EYE_MODEL = "lbfmodel.yaml"
EYE_MODEL_URL = "https://raw.githubusercontent.com/kurnianggoro/GSOC2017/252614093b29280c549351971d44d71e63c9197c/data/lbfmodel.yaml"
EYE_MODEL_SHA256 = "70dd8b1657c42d1595d6bd13d97d932877b3bed54a95d3c4733a0f740d1fd66b"
EYE_MODEL_SIZE = 56375857

HOWDY_MODELS = os.environ.get("TESSERA_HOWDY_MODELS", "/usr/share/howdy/models")
# SFace's cosine score for "the same person" (OpenCV's own threshold)
SAME_PERSON = 0.363

# The blink (seconds, and the EAR as a share of the open level)
WINDOW = 3.0        # open, closed, open again: all within this
CLOSED = 0.70       # closed: at or below 70% of the open level
REOPEN = 0.85       # open again: back to 85% of it
MAX_CLOSED = 1.0    # eyes shut longer than this isn't a blink
MIN_OPEN = 0.15     # an open level below this EAR isn't an open eye
SHAPE = 0.12        # the face's proportion may change this much in a blink
OPEN_FRAMES = 3     # frames seen open before a closing counts
LOST = 0.4          # the face gone longer than this starts over


def say(text):
    """A line for the lock (pam_exec's stdout becomes PAM info)"""
    print("Tessera: " + text, flush=True)


def log(text):
    print("face_live: " + text, file=sys.stderr, flush=True)


# ── The blink, from a series of EARs (no OpenCV needed: tested alone) ──────

def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def ear(eye):
    """Eye aspect ratio of six points in the 68-point order (corner, two
    upper lid points, corner, two lower lid points)"""
    p = eye
    w = dist(p[0], p[3])
    if w <= 0:
        return 0.0
    return (dist(p[1], p[5]) + dist(p[2], p[4])) / (2.0 * w)


def centre(pts):
    return (sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts))


def measure(points):
    """(EAR, shape) from 68 landmarks: both eyes' mean EAR (36-41, 42-47),
    and the face's own proportion (the eyes to the nose tip, 30, over the
    distance between the eyes). A blink moves only the eyelids; a photo
    tilted towards or away from the camera squashes the whole face, its
    shape with its EAR."""
    if len(points) < 48:
        return None
    left, right = centre(points[36:42]), centre(points[42:48])
    across = dist(left, right)
    shape = dist(centre([left, right]), points[30]) / across if across > 0 else 0.0
    return (ear(points[36:42]) + ear(points[42:48])) / 2.0, shape


class Blink:
    """Feed it (time, EAR or None when there's no face, and the face's
    shape when known); `blinked` turns true once the eyes were open, closed,
    then open again within WINDOW.

    The open level is the median of the recent open frames, so it follows
    the person (and the light); a closing is a drop to CLOSED of it, which
    a photo's jitter never reaches. A frame whose face shape is off by more
    than SHAPE from the open frames' (a tilted photo) counts for nothing."""

    def __init__(self, closed=CLOSED):
        # (a person's own closing, from enrolment: never looser than CLOSED)
        self.closed = min(closed, CLOSED)
        self.dip = 1.0           # the deepest closing seen, as a share of the open level
        self.reset()

    def reset(self):
        self.opens = []          # (t, ear, shape) of recent open frames
        self.closed_at = None    # when the eyes closed
        self.last_open = None    # the last open frame before closing
        self.blinked = False
        self.lost_at = None

    def level(self):
        if len(self.opens) < OPEN_FRAMES:
            return None
        e = sorted(x for _, x, _ in self.opens)
        m = e[len(e) // 2]
        return m if m >= MIN_OPEN else None

    def shape(self):
        s = sorted(g for _, _, g in self.opens if g is not None)
        return s[len(s) // 2] if s else None

    def feed(self, t, e, g=None):
        if self.blinked:
            return True
        if e is None:
            if self.lost_at is None:
                self.lost_at = t
            elif t - self.lost_at > LOST:
                self.reset()
            return False
        self.lost_at = None
        lvl = self.level()
        base = self.shape()
        if g is not None and base and abs(g - base) > base * SHAPE:
            # (the face itself changed shape: a tilt, not a blink)
            if self.closed_at is not None:
                self.closed_at = None
            return False
        if self.closed_at is None:
            if lvl is not None and e <= lvl * self.closed:
                self.closed_at = t
                self.dip = min(self.dip, e / lvl)
                return False
            # (an open frame: kept for the level, the last second's worth)
            if lvl is None or e > lvl * self.closed:
                self.opens.append((t, e, g))
                self.opens = [o for o in self.opens if t - o[0] <= 1.0]
                self.last_open = t
            return False
        # closed: open again in time, or too long
        if lvl:
            self.dip = min(self.dip, e / lvl)
        if t - self.closed_at > MAX_CLOSED:
            self.closed_at = None
            self.opens = []
            return False
        if lvl is not None and e >= lvl * REOPEN:
            if self.last_open is not None and t - self.last_open <= WINDOW:
                self.blinked = True
                return True
            self.closed_at = None
        return False


def blinked(series, closed=CLOSED):
    """True when a series of (t, EAR | None[, shape]) blinks"""
    b = Blink(closed)
    return any(b.feed(*x) for x in series)


def personal_closing(dip):
    """The closing to ask of this person, from their deepest blink while
    adding their face (a share of their open level): halfway down to it,
    never looser than CLOSED (photo-safe) nor tighter than 0.45. None when
    their blinks barely show (the camera or the light can't see them)."""
    if dip > CLOSED:
        return None
    return round(max(0.45, min(CLOSED, (1.0 + dip) / 2.0)), 3)


def iou(a, b):
    """Overlap of two boxes (x, y, w, h)"""
    ax, ay, aw, ah = a
    bx, by, bw, bh = b
    x1, y1 = max(ax, bx), max(ay, by)
    x2, y2 = min(ax + aw, bx + bw), min(ay + ah, by + bh)
    inter = max(0.0, x2 - x1) * max(0.0, y2 - y1)
    union = aw * ah + bw * bh - inter
    return inter / union if union > 0 else 0.0


# ── Where things are ───────────────────────────────────────────────────────

def home():
    h = os.environ.get("HOME")
    if h:
        return h
    try:
        return pwd.getpwuid(os.getuid()).pw_dir
    except KeyError:
        return "/"


# Administrator prompts (opt-in: tessera-root face admin on): polkit's helper
# runs this as root, for PAM_USER, from root's own copies (it can't read
# your home; and a copy you could change mustn't approve an administrator)
ADMIN_DIR = "/var/lib/tessera/face"
ADMIN_USER = ""  # (set by --admin)
USER_NAME = r"^[a-z_][a-z0-9_-]{0,31}\$?$"


def data_dir():
    if ADMIN_USER:
        return ADMIN_DIR
    base = os.environ.get("XDG_DATA_HOME") or os.path.join(home(), ".local/share")
    return os.path.join(base, "tessera/face")


def eye_model():
    return os.path.join(data_dir(), EYE_MODEL)


def me_file():
    return os.path.join(ADMIN_DIR, ADMIN_USER + ".json") if ADMIN_USER else os.path.join(data_dir(), "me.json")


def admin_device():
    """The camera root's copy names (tessera-root face admin on)"""
    try:
        with open(me_file()) as f:
            return str(json.load(f).get("device", ""))
    except (OSError, ValueError, AttributeError):
        return ""


def howdy_model(prefix):
    found = sorted(glob.glob(os.path.join(HOWDY_MODELS, prefix + "*.onnx")))
    return found[-1] if found else ""


def have_cv2():
    import importlib.util
    return importlib.util.find_spec("cv2") is not None


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def eye_model_ok(path=None, size=EYE_MODEL_SIZE):
    """There, and the size it should be (the SHA-256 is checked when it's
    downloaded; the file is yours, in your own folder)"""
    try:
        return os.path.getsize(path or eye_model()) == size
    except OSError:
        return False


def enrolled():
    try:
        with open(me_file()) as f:
            return len(json.load(f).get("features", []))
    except (OSError, ValueError, AttributeError):
        return 0


def status():
    return {"cv2": have_cv2(), "eyeModel": eye_model_ok(),
            "howdyModels": bool(howdy_model("face_detection_yunet") and howdy_model("face_recognition_sface")),
            "enrolled": enrolled()}


# ── The download ───────────────────────────────────────────────────────────

def download(url=EYE_MODEL_URL, sha=EYE_MODEL_SHA256, dest=None, opener=None):
    """The model, to a temporary file, checked, then moved into place.
    Returns an error message, or "" when it's there."""
    import urllib.request
    dest = dest or eye_model()
    os.makedirs(os.path.dirname(dest), mode=0o700, exist_ok=True)
    tmp = dest + ".part"
    try:
        with (opener or urllib.request.urlopen)(url, timeout=30) as r, open(tmp, "wb") as f:
            total = int(r.headers.get("Content-Length") or EYE_MODEL_SIZE)
            got, shown = 0, -1
            h = hashlib.sha256()
            while True:
                chunk = r.read(1 << 16)
                if not chunk:
                    break
                f.write(chunk)
                h.update(chunk)
                got += len(chunk)
                pct = min(100, got * 100 // max(total, 1))
                if pct != shown:
                    print(f"PROGRESS {pct}", flush=True)
                    shown = pct
        if h.hexdigest() != sha:
            os.unlink(tmp)
            return "The eye model didn't download correctly (its checksum is wrong). Try again."
        os.replace(tmp, dest)
        return ""
    except OSError as e:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        return f"The eye model couldn't be downloaded (check the internet connection): {e}"


# ── The camera ─────────────────────────────────────────────────────────────

class Eyes:
    """YuNet (face), LBF (68 landmarks), SFace (who): OpenCV's"""

    def __init__(self, cv2, width, height):
        self.cv2 = cv2
        det, rec = howdy_model("face_detection_yunet"), howdy_model("face_recognition_sface")
        if not det or not rec:
            raise RuntimeError("Howdy's face models aren't there (Settings → Face → Set Up)")
        if not hasattr(cv2, "face"):
            raise RuntimeError("OpenCV has no face module (python-opencv)")
        if not eye_model_ok():
            raise RuntimeError("the eye model isn't downloaded (Settings → Face → Set Up)")
        self.det = cv2.FaceDetectorYN.create(det, "", (width, height), 0.8, 0.3, 5)
        self.rec = cv2.FaceRecognizerSF.create(rec, "")
        self.marks = cv2.face.createFacemarkLBF()
        self.marks.loadModel(eye_model())
        self.size = (width, height)

    def face(self, frame):
        """The largest face's YuNet row, or None"""
        h, w = frame.shape[:2]
        if (w, h) != self.size:
            self.det.setInputSize((w, h))
            self.size = (w, h)
        _, faces = self.det.detect(frame)
        if faces is None or len(faces) == 0:
            return None
        return max(faces, key=lambda f: f[2] * f[3])

    def measure(self, frame, face):
        """(EAR, shape), or None"""
        import numpy as np
        box = np.array([[face[0], face[1], face[2], face[3]]], dtype=np.int32)
        ok, pts = self.marks.fit(frame, box)
        if not ok or len(pts) == 0:
            return None
        return measure([tuple(float(v) for v in p) for p in np.asarray(pts[0]).reshape(-1, 2)])

    def feature(self, frame, face):
        return self.rec.feature(self.rec.alignCrop(frame, face))

    def same(self, a, b):
        return self.rec.match(a, b, 0) >= SAME_PERSON  # 0: FR_COSINE


def open_camera(cv2, device):
    cap = cv2.VideoCapture(device, cv2.CAP_V4L2)
    if not cap.isOpened():
        return None
    cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
    return cap


def load_me(np):
    """(your features, your closing): what enrolment kept"""
    try:
        with open(me_file()) as f:
            v = json.load(f)
        closing = float(v.get("closed", CLOSED))
        return [np.array(x, dtype=np.float32).reshape(1, -1) for x in v["features"]], closing
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return [], CLOSED


def check(device, timeout=6.0):
    try:
        import cv2
        import numpy as np
    except ImportError:
        log("no OpenCV for Python (python-opencv)")
        return 2
    me, closing = load_me(np)
    if not me:
        log("no face added for the blink check")
        return 2
    cap = open_camera(cv2, device)
    if cap is None:
        log(f"can't open {device}")
        return 2
    try:
        eyes = Eyes(cv2, int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)) or 640, int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)) or 480)
    except (RuntimeError, cv2.error) as e:
        log(str(e))
        cap.release()
        return 2
    blink = Blink(closing)
    start = time.monotonic()
    last_box, known, told = None, False, ""
    try:
        while time.monotonic() - start < timeout:
            ok, frame = cap.read()
            t = time.monotonic()
            if not ok:
                continue
            f = eyes.face(frame)
            if f is None:
                blink.feed(t, None)
                if told != "look" and t - start > 1.0:
                    say("Look at the camera")
                    told = "look"
                continue
            box = tuple(float(x) for x in f[:4])
            # (another face, or the same one jumped: start over)
            if last_box is not None and iou(box, last_box) < 0.3:
                blink.reset()
                known = False
            last_box = box
            if not known:
                if not any(eyes.same(eyes.feature(frame, f), x) for x in me):
                    log("not the face added here")
                    continue
                known = True
                if told != "blink":
                    say("Blink to unlock")
                    told = "blink"
            m = eyes.measure(frame, f)
            if blink.feed(t, *(m or (None,))):
                # (still you, eyes open again)
                if any(eyes.same(eyes.feature(frame, f), x) for x in me):
                    return 0
                blink.reset()
                known = False
        say("No blink seen")
        return 1
    finally:
        cap.release()


def enrol(device, frames=5, timeout=10.0):
    """Your face's features, and your blink: how far your eyes' EAR drops
    when you blink, as this camera and its landmarks see it (personal_closing).
    A face whose blinks don't show isn't added: the lock couldn't tell it
    from a photo."""
    try:
        import cv2
        import numpy as np  # noqa: F401
    except ImportError:
        print("OpenCV for Python (python-opencv) isn't installed", file=sys.stderr)
        return 2
    cap = open_camera(cv2, device)
    if cap is None:
        print(f"The camera ({device}) couldn't be opened", file=sys.stderr)
        return 2
    try:
        try:
            eyes = Eyes(cv2, int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)) or 640, int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)) or 480)
        except (RuntimeError, cv2.error) as e:
            print(f"Face unlock isn't set up yet: {e}", file=sys.stderr)
            return 2
        say("Look at the camera and blink a few times")
        got, n, blink, blinks, last_box = [], 0, Blink(), 0, None
        dips = []
        start = time.monotonic()
        while time.monotonic() - start < timeout and (len(got) < frames or blinks < 2):
            ok, frame = cap.read()
            t = time.monotonic()
            if not ok:
                continue
            f = eyes.face(frame)
            if f is None:
                blink.feed(t, None)
                continue
            box = tuple(float(x) for x in f[:4])
            if last_box is not None and iou(box, last_box) < 0.3:
                # (someone else, or a jump: only one face is added)
                blink = Blink()
            last_box = box
            n += 1
            if len(got) < frames and n % 4 == 0:
                got.append(eyes.feature(frame, f).reshape(-1).tolist())
            m = eyes.measure(frame, f)
            if blink.feed(t, *(m or (None,))):
                blinks += 1
                dips.append(blink.dip)
                blink = Blink()
        if len(got) < frames:
            print("No face was found. Look at the camera in good light, then try again.", file=sys.stderr)
            return 1
        closing = personal_closing(min(dips)) if dips else None
        if closing is None:
            print("Your blink wasn't seen. Look straight at the camera in good light and blink normally, then try again. "
                  "If it still isn't seen, this camera can't do the blink check: use your password, or an infrared camera.", file=sys.stderr)
            return 1
    finally:
        cap.release()
    old = []
    try:
        with open(me_file()) as f:
            v = json.load(f)
        old, closing = v.get("features", []), max(closing, float(v.get("closed", 0)))
    except (OSError, ValueError, AttributeError, TypeError):
        pass
    os.makedirs(data_dir(), mode=0o700, exist_ok=True)
    tmp = me_file() + ".tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump({"version": 1, "features": old + got, "closed": min(closing, CLOSED)}, f)
    os.replace(tmp, me_file())
    return 0


def clear():
    try:
        os.unlink(me_file())
    except FileNotFoundError:
        pass
    return 0


def arg(args, name, default):
    if name in args:
        i = args.index(name)
        if i + 1 < len(args):
            return args[i + 1]
    return default


def main(argv):
    global ADMIN_USER
    import re
    cmd = argv[1] if len(argv) > 1 else ""
    rest = argv[2:]
    if "--admin" in rest:
        # (polkit's helper, as root: only a check, only from root's copies)
        user = os.environ.get("PAM_USER", "")
        if cmd != "check" or not re.match(USER_NAME, user):
            log("--admin: a check for PAM_USER only")
            return 2
        ADMIN_USER = user
        dev = admin_device()
        return check(dev, 6.0) if dev else 2
    device = arg(rest, "--device", os.environ.get("TESSERA_FACE_DEVICE", ""))
    if cmd == "status":
        print(json.dumps(status()))
        return 0
    if cmd == "models":
        if eye_model_ok():
            print("PROGRESS 100", flush=True)
            return 0
        err = download()
        if err:
            print(err, file=sys.stderr)
            return 1
        return 0
    if cmd == "clear":
        return clear()
    if cmd in ("check", "enrol"):
        if not device:
            log("no camera given (--device or TESSERA_FACE_DEVICE)")
            return 2
        if cmd == "enrol":
            return enrol(device)
        try:
            timeout = float(arg(rest, "--timeout", "6"))
        except ValueError:
            timeout = 6.0
        return check(device, timeout)
    print("usage: face_live.py check|enrol [--device DEV] | clear | models | status", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
