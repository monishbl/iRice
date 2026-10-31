#!/usr/bin/env python3
"""Live captions (services/Captions.qml): subtitles for the sound this
computer plays (videos, calls, podcasts; never the microphone), made here
with whisper.cpp's Tiny model (dictate.py's models and binary).

    captions.py run [LANG]   JSON lines on stdout until stopped (SIGTERM):
                             {"partial": text}  the sentence being heard
                             {"final": text}    a finished one
                             {"quiet": true}    nothing said for a while
                             {"error": text, "need": "binary"|"model"|"audio"}

How: the speakers' sound (the default output's monitor, through
PipeWire's recorder, else PulseAudio's) at 16 kHz mono, in segments. Every
1.2 s while someone speaks, the segment so far is turned into text (shown
as it grows); a pause, or 8 s of speech, finishes it. The sentence before
goes along as context, so a sentence split in two still reads right. Silence costs nothing:
it isn't sent to whisper. Nothing is saved: each piece is a file in the
runtime folder, removed straight away.
"""
import importlib.util
import json
import math
import os
import queue
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import threading
import time
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
RATE = 16000
CHUNK = RATE // 10 * 2      # bytes in 100 ms (s16 mono)
STEP = 1.2                  # s between looks at the sentence being heard
LONGEST = 8.0               # s: a sentence is finished at this length…
PAUSE = 0.7                 # …or after this much quiet, once it's this long:
SHORTEST = 2.5              # (shorter ones are too little for whisper to hear right)
QUIET = 350                 # RMS below which a chunk is silence
MODEL = "tiny"


def load_dictate():
    spec = importlib.util.spec_from_file_location("dictate", os.path.join(HERE, "dictate.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def emit(obj):
    print(json.dumps(obj), flush=True)


def rms(chunk):
    n = len(chunk) // 2
    if n == 0:
        return 0
    samples = struct.unpack(f"<{n}h", chunk[:n * 2])
    return math.sqrt(sum(s * s for s in samples) / n)


def recorder():
    """The speakers' sound, raw s16 mono 16 kHz on stdout. (IRICE_CAPTIONS_SINK:
    another output's, by name: tests play into a silent one.)"""
    sink = os.environ.get("IRICE_CAPTIONS_SINK", "")
    if shutil.which("pw-record"):
        return ["pw-record", "--raw", "--rate", str(RATE), "--channels", "1", "--format", "s16",
                "-P", "{ stream.capture.sink = true node.name = irice-captions media.name = \"Live captions\" }"] + \
            (["--target", sink] if sink else []) + ["-"]
    if shutil.which("parec"):
        return ["parec", "-d", (sink + ".monitor") if sink else "@DEFAULT_MONITOR@", f"--rate={RATE}", "--channels=1", "--format=s16le", "--raw",
                "--client-name=irice-captions"]
    return None


class Segmenter:
    """Splits the sound into sentences: decides when to look at the one
    being heard (partial) and when it's finished (final). Pure: fed chunks
    and their loudness, it says what to do (tested on its own)."""

    def __init__(self):
        self.audio = bytearray()
        self.spoken = False     # some speech in it
        self.quiet_for = 0.0    # s of silence at its end
        self.since_look = 0.0
        self.silent_total = 0.0

    def feed(self, chunk, loud):
        """Returns "partial", "final", "drop", "quiet" or None."""
        dt = len(chunk) / 2 / RATE
        self.audio += chunk
        self.since_look += dt
        if loud >= QUIET:
            self.spoken = True
            self.quiet_for = 0.0
            self.silent_total = 0.0
        else:
            self.quiet_for += dt
            self.silent_total += dt
        length = len(self.audio) / 2 / RATE
        if not self.spoken:
            # (nothing said: keep only the last moment, as a lead-in)
            if length > 0.5:
                del self.audio[:len(self.audio) - int(0.3 * RATE) * 2]
            return "quiet" if self.silent_total >= 4.0 and self.silent_total - dt < 4.0 else None
        if length >= LONGEST or (self.quiet_for >= PAUSE and length >= SHORTEST):
            return "final"
        if self.since_look >= STEP:
            self.since_look = 0.0
            return "partial"
        return None

    def take(self):
        audio = bytes(self.audio)
        self.audio = bytearray()
        self.spoken = False
        self.quiet_for = 0.0
        self.since_look = 0.0
        return audio


def transcribe(exe, model, audio, lang, threads, dictate, folder, before=""):
    fd, path = tempfile.mkstemp(prefix="caption-", suffix=".wav", dir=folder)
    os.close(fd)
    try:
        with wave.open(path, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes(audio)
        # (-ac 768: whisper looks at 15 s rather than 30, quicker, and still
        # more than the longest piece; the sentence before as its context)
        cmd = [exe, "-m", model, "-f", path, "-l", lang or "auto", "-t", str(threads), "-nt", "-np", "-ac", "768"]
        if before:
            cmd += ["--prompt", before[-200:]]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        return dictate.clean(r.stdout) if r.returncode == 0 else ""
    except subprocess.TimeoutExpired:
        return ""
    finally:
        try:
            os.unlink(path)
        except OSError:
            pass


class Stopped(BaseException):
    pass


def stopped(*_):
    raise Stopped()


def run(lang):
    dictate = load_dictate()
    exe = dictate.binary()
    if not exe:
        emit({"error": "Live captions need speech recognition (whisper.cpp). Install it in Settings → Accessibility.", "need": "binary"})
        return 1
    model = dictate.model_path(MODEL)
    if not os.path.isfile(model):
        emit({"error": "Live captions need the Tiny speech model. Download it in Settings → Accessibility.", "need": "model"})
        return 1
    cmd = recorder()
    if not cmd:
        emit({"error": "Live captions need PipeWire's or PulseAudio's recorder (pw-record or parec).", "need": "audio"})
        return 1
    folder = os.path.join(os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir(), "irice")
    os.makedirs(folder, mode=0o700, exist_ok=True)
    threads = max(1, min(4, (os.cpu_count() or 4) - 1))
    rec = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL)
    chunks = queue.Queue()

    def read():
        # (its own thread: whisper takes a moment, the sound keeps coming)
        while True:
            c = rec.stdout.read(CHUNK)
            chunks.put(c)
            if not c:
                return
    threading.Thread(target=read, daemon=True).start()
    seg = Segmenter()
    before = ""
    try:
        while True:
            c = chunks.get()
            if not c:
                emit({"error": "The sound couldn't be listened to. Check Settings → Sound.", "need": "audio"})
                return 1
            what = seg.feed(c, rms(c))
            if what == "quiet":
                emit({"quiet": True})
            elif what == "partial":
                # (only when it can keep up: behind, it waits for the end of the sentence)
                if chunks.qsize() < 5:
                    text = transcribe(exe, model, bytes(seg.audio), lang, threads, dictate, folder, before)
                    if text:
                        emit({"partial": text})
            elif what == "final":
                text = transcribe(exe, model, seg.take(), lang, threads, dictate, folder, before)
                before = text or before
                emit({"final": text})
    finally:
        rec.terminate()
        try:
            rec.wait(timeout=2)
        except subprocess.TimeoutExpired:
            rec.kill()


def main(argv):
    signal.signal(signal.SIGTERM, stopped)
    if argv[:1] == ["run"]:
        return run(argv[1] if len(argv) > 1 else "auto")
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except (Stopped, KeyboardInterrupt):
        sys.exit(0)
