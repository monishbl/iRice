#!/usr/bin/env python3
"""Makes iRice's own UI sounds from sine partials, in several styles:
shell/assets/sounds/<pack>/<event>.ogg (Settings → Notifications → Style).

  tools/make-sounds.py

Pure Python (no numpy); ffmpeg encodes the result as Ogg Vorbis. Every sound
is original. The packs:
  soft     the default: low, rounded sine tones, slow attacks, few notes
  marimba  a wooden mallet: bright attack, warm body, quick decay
  keys     a felt electric piano: gentle tines with a soft bell overtone
  taps     short low "thocks" and ticks, almost no melody
  minimal  quiet ticks; only logging in gets a (soft) tone
  glass    the first set: glassy taps, sparkles and pops (the most playful)
Each pack has the same events (see EVENTS); Sounds.qml picks the folder.
"""
import math, os, random, struct, subprocess, tempfile, wave

RATE = 48000
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "shell", "assets", "sounds")
OUT = ROOT  # the pack being written (set per pack)
EVENTS = ["startup", "charging", "display-connect", "display-disconnect", "lock", "unlock", "notification", "battery",
          "volume", "usb-connect", "usb-disconnect", "received", "record-start", "record-stop", "error", "today"]


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def buffer(seconds):
    n = int(seconds * RATE)
    return [0.0] * n, [0.0] * n


def tone(buf, start, f, amp, attack, tau, pan=0.0, partials=((1, 1),), length=None, shimmer=0.0, glide=1.0, glide_time=0.05):
    """A note: sine partials (ratio, weight) with a smooth attack and an
    exponential decay (tau seconds), panned -1..1. glide: the pitch slides
    from f * glide to f over glide_time (a "pop" or a "tock")."""
    left, right = buf
    n0 = int(start * RATE)
    end = len(left) if length is None else min(len(left), n0 + int(length * RATE))
    gl, gr = math.cos((pan + 1) * math.pi / 4), math.sin((pan + 1) * math.pi / 4)
    phases = [random.random() * 2 * math.pi for _ in partials]
    acc = 0.0  # phase of the fundamental, integrated (the pitch can move)
    for i in range(n0, end):
        t = (i - n0) / RATE
        fi = f * (1 + (glide - 1) * math.exp(-t / glide_time)) if glide != 1.0 else f
        acc += 2 * math.pi * fi / RATE
        env = (0.5 - 0.5 * math.cos(math.pi * min(1.0, t / attack))) * math.exp(-t / tau)
        if env < 1e-5 and t > attack:
            break
        s = 0.0
        for (ratio, w), ph in zip(partials, phases):
            # higher partials fade sooner, like a struck glass
            s += w * math.exp(-t * (ratio - 1) * 3.0 / max(tau, 0.05)) * math.sin(acc * ratio + ph)
        if shimmer:
            s *= 1 + shimmer * math.sin(2 * math.pi * 5.2 * t)
        v = amp * env * s
        left[i] += v * gl
        right[i] += v * gr


def finish(buf, name, peak_db):
    left, right = buf
    n = len(left)
    # remove DC, fade the ends (no clicks), normalise the peak
    for ch in (left, right):
        m = sum(ch) / n
        for i in range(n):
            ch[i] -= m
    fin, fout = int(0.004 * RATE), int(0.04 * RATE)
    for ch in (left, right):
        for i in range(fin):
            ch[i] *= i / fin
        for i in range(fout):
            ch[n - 1 - i] *= i / fout
    peak = max(max(abs(x) for x in left), max(abs(x) for x in right))
    gain = 10 ** (peak_db / 20) / peak
    frames = b"".join(struct.pack("<hh", int(l * gain * 32767), int(r * gain * 32767)) for l, r in zip(left, right))
    with tempfile.NamedTemporaryFile(suffix=".wav") as tmp:
        with wave.open(tmp.name, "wb") as w:
            w.setnchannels(2)
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes(frames)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", tmp.name, "-c:a", "libvorbis", "-q:a", "6",
                        os.path.join(OUT, name + ".ogg")], check=True)
    print(name, f"{n / RATE:.2f}s")


def startup():
    # F major, spread over three octaves; each note is three slightly detuned
    # voices for warmth, the top ones with a little shimmer
    buf = buffer(2.6)
    notes = [(41, 0.9, -0.3), (48, 0.75, 0.25), (53, 0.7, -0.15), (57, 0.5, 0.3), (60, 0.42, -0.25), (65, 0.22, 0.1)]
    for k, (m, amp, pan) in enumerate(notes):
        for cents, p in ((-4, -0.4), (0, 0.0), (5, 0.4)):
            f = hz(m) * 2 ** (cents / 1200)
            tone(buf, 0.012 * k, f, amp / 3, attack=0.09 + 0.02 * k, tau=0.95 - 0.06 * k,
                 pan=max(-1, min(1, pan + p * 0.3)), partials=((1, 1), (2, 0.18), (3, 0.05)),
                 shimmer=0.06 if m >= 57 else 0.0)
    finish(buf, "startup", -6)


def charging():
    # two quick glass taps, the second a fourth higher; 2.76 is the inharmonic
    # partial that makes it sound like glass rather than a beep
    buf = buffer(0.55)
    glass = ((1, 1), (2.76, 0.22), (5.4, 0.06))
    tone(buf, 0.0, hz(84), 0.6, attack=0.003, tau=0.09, pan=-0.1, partials=glass)
    tone(buf, 0.085, hz(89), 0.7, attack=0.003, tau=0.13, pan=0.1, partials=glass)
    finish(buf, "charging", -6)


def display(name, a, b):
    buf = buffer(0.48)
    soft = ((1, 1), (2, 0.12))
    tone(buf, 0.0, hz(a), 0.6, attack=0.012, tau=0.11, pan=-0.15, partials=soft)
    tone(buf, 0.11, hz(b), 0.6, attack=0.012, tau=0.16, pan=0.15, partials=soft)
    finish(buf, name, -8)


def lock():
    # a small, dry latch: a bright tick over a short low body
    buf = buffer(0.16)
    tone(buf, 0.0, 2600, 0.35, attack=0.0006, tau=0.006, partials=((1, 1), (1.7, 0.4)))
    tone(buf, 0.0, 340, 0.9, attack=0.001, tau=0.022, glide=1.6, glide_time=0.01)
    tone(buf, 0.028, 1700, 0.18, attack=0.0006, tau=0.005)
    finish(buf, "lock", -6)


def unlock():
    # the latch the other way: lighter, and it lifts
    buf = buffer(0.18)
    tone(buf, 0.0, 1700, 0.2, attack=0.0006, tau=0.005)
    tone(buf, 0.022, 420, 0.7, attack=0.001, tau=0.02, glide=0.7, glide_time=0.012)
    tone(buf, 0.03, 2900, 0.3, attack=0.0006, tau=0.007, partials=((1, 1), (1.7, 0.35)))
    finish(buf, "unlock", -8)


def notification():
    # a soft glass "ding-ding" a major third apart, the second quieter
    buf = buffer(0.9)
    glass = ((1, 1), (2.76, 0.12), (5.4, 0.03))
    tone(buf, 0.0, hz(88), 0.6, attack=0.004, tau=0.22, pan=-0.1, partials=glass)
    tone(buf, 0.12, hz(84), 0.45, attack=0.004, tau=0.3, pan=0.1, partials=glass)
    finish(buf, "notification", -9)


def battery():
    # two falling notes, a minor third: something needs attention, nothing broke
    buf = buffer(0.75)
    soft = ((1, 1), (2, 0.15), (3, 0.04))
    tone(buf, 0.0, hz(76), 0.6, attack=0.01, tau=0.16, pan=-0.1, partials=soft)
    tone(buf, 0.16, hz(73), 0.6, attack=0.01, tau=0.26, pan=0.1, partials=soft)
    finish(buf, "battery", -8)


def volume():
    # the volume "pop": a tiny bubble whose pitch drops as it sounds
    buf = buffer(0.09)
    tone(buf, 0.0, 520, 1.0, attack=0.002, tau=0.018, glide=1.9, glide_time=0.012, partials=((1, 1), (2, 0.1)))
    finish(buf, "volume", -10)


def usb(name, a, b):
    # like the display two-tone, quicker and lighter
    buf = buffer(0.3)
    tone(buf, 0.0, hz(a), 0.5, attack=0.004, tau=0.05, pan=-0.1, partials=((1, 1), (2.76, 0.08)))
    tone(buf, 0.06, hz(b), 0.5, attack=0.004, tau=0.08, pan=0.1, partials=((1, 1), (2.76, 0.08)))
    finish(buf, name, -10)


def received():
    # something arrived: a quick rising sparkle that lands on a soft chord
    buf = buffer(1.0)
    glass = ((1, 1), (2.76, 0.1))
    for k, m in enumerate((79, 84, 88, 91)):
        tone(buf, 0.045 * k, hz(m), 0.35, attack=0.003, tau=0.12 + 0.05 * k, pan=-0.3 + 0.2 * k, partials=glass)
    for m in (72, 76, 79):
        tone(buf, 0.18, hz(m), 0.25, attack=0.04, tau=0.35, partials=((1, 1), (2, 0.1)))
    finish(buf, "received", -9)


def recording(name, notes):
    buf = buffer(0.25 + 0.09 * len(notes))
    for k, m in enumerate(notes):
        tone(buf, 0.09 * k, hz(m), 0.6, attack=0.004, tau=0.06, partials=((1, 1), (2, 0.1)))
    finish(buf, name, -10)


def error():
    # "no": a low, dull knock that falls a little
    buf = buffer(0.35)
    tone(buf, 0.0, 290, 1.0, attack=0.002, tau=0.07, glide=1.25, glide_time=0.04, partials=((1, 1), (2, 0.35), (3, 0.12)))
    tone(buf, 0.0, 900, 0.12, attack=0.001, tau=0.01)
    finish(buf, "error", -7)


def glass_pack():
    startup()
    charging()
    display("display-connect", 72, 79)
    display("display-disconnect", 79, 72)
    lock()
    unlock()
    notification()
    battery()
    volume()
    usb("usb-connect", 84, 91)
    usb("usb-disconnect", 91, 84)
    received()
    recording("record-start", [91])
    recording("record-stop", [91, 86])
    error()
    today_glass()


# ── Instrument packs ────────────────────────────────────────────────────
# Each pack is an instrument (how one note sounds), a tick (lock, volume,
# "no"), a register and a loudness; the events are the same short phrases
# in every pack, so they keep their meaning whichever style is chosen.

class Pack:
    name = ""
    shift = 0  # semitones added to every note
    gain = 0.0  # dB added to every event's peak
    room = 1.0  # longer files for instruments that ring longer
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        raise NotImplementedError
    def tick(self, buf, t, kind):
        """kind: lock | unlock | volume | error"""
        raise NotImplementedError
    def chord(self, buf):
        """The startup sound"""
        raise NotImplementedError


class Soft(Pack):
    # rounded sines with a hint of second harmonic, low and unhurried
    name, shift, gain, room = "soft", -5, 0.0, 1.3
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        tone(buf, t, hz(m + self.shift), amp, attack=0.018, tau=0.16 * ring, pan=pan, partials=((1, 1), (2, 0.08)))
    def tick(self, buf, t, kind):
        if kind == "volume":
            tone(buf, t, 440, 1.0, attack=0.004, tau=0.025, partials=((1, 1), (2, 0.05)))
        elif kind == "error":
            tone(buf, t, 220, 1.0, attack=0.006, tau=0.09, partials=((1, 1), (2, 0.2)))
        else:  # a soft, low latch; unlocking a fifth up
            f = 330 if kind == "lock" else 495
            tone(buf, t, f, 0.9, attack=0.003, tau=0.03, partials=((1, 1), (2, 0.1)))
    def chord(self, buf):
        for k, (m, amp, pan) in enumerate([(41, 0.8, -0.2), (48, 0.6, 0.2), (53, 0.55, -0.1), (57, 0.4, 0.15)]):
            for cents in (-3, 3):
                tone(buf, 0.03 * k, hz(m) * 2 ** (cents / 1200), amp / 2, attack=0.25, tau=0.9, pan=pan,
                     partials=((1, 1), (2, 0.1)))


class Marimba(Pack):
    # a struck wooden bar: the fourth partial (about 3.9x) and a tenth (9.9x)
    # ring briefly over a warm fundamental
    name, shift, gain = "marimba", -12, 0.0
    bar = ((1, 1), (3.93, 0.45), (9.9, 0.12))
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        tone(buf, t, hz(m + self.shift), amp, attack=0.002, tau=0.18 * ring, pan=pan, partials=self.bar)
    def tick(self, buf, t, kind):
        f = {"lock": 392, "unlock": 523, "volume": 659, "error": 196}[kind]
        tau = 0.05 if kind != "error" else 0.12
        tone(buf, t, f, 1.0, attack=0.0015, tau=tau, partials=self.bar)
    def chord(self, buf):
        # a rolled F major on a low bar, like a gentle mallet sweep
        for k, m in enumerate((53, 57, 60, 65, 69)):
            tone(buf, 0.07 * k, hz(m - 12), 0.6, attack=0.002, tau=0.45, pan=-0.3 + 0.15 * k, partials=self.bar)


class Keys(Pack):
    # a felt electric piano: soft tine, a little bell (7x) and a slow tremolo
    name, shift, gain, room = "keys", -9, 0.0, 2.2
    tine = ((1, 1), (2, 0.3), (3, 0.06), (7.1, 0.04))
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        tone(buf, t, hz(m + self.shift), amp, attack=0.006, tau=0.35 * ring, pan=pan, partials=self.tine, shimmer=0.04)
    def tick(self, buf, t, kind):
        f = {"lock": hz(64), "unlock": hz(71), "volume": hz(76), "error": hz(52)}[kind]
        tone(buf, t, f, 1.0, attack=0.004, tau=0.06 if kind != "error" else 0.15, partials=self.tine)
    def chord(self, buf):
        # an Fmaj9 voicing, played softly with the notes a touch apart
        for k, (m, amp, pan) in enumerate([(41, 0.7, -0.2), (52, 0.45, 0.1), (55, 0.45, -0.1), (57, 0.4, 0.2), (60, 0.35, -0.15), (64, 0.25, 0.15)]):
            tone(buf, 0.025 * k, hz(m), amp, attack=0.01, tau=1.0, pan=pan, partials=self.tine, shimmer=0.05)


class Taps(Pack):
    # low, short thocks; pitch only hints at up or down
    name, shift, gain = "taps", -24, -2.0
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        f = hz(m + self.shift)
        tone(buf, t, f, amp, attack=0.0015, tau=0.03 * min(ring, 1.5), pan=pan, partials=((1, 1), (2.3, 0.25)))
        tone(buf, t, 3200, amp * 0.06, attack=0.0005, tau=0.003, pan=pan)  # the contact
    def tick(self, buf, t, kind):
        f = {"lock": 260, "unlock": 300, "volume": 380, "error": 170}[kind]
        tone(buf, t, f, 1.0, attack=0.001, tau=0.025 if kind != "error" else 0.05, partials=((1, 1), (2.3, 0.25)))
        tone(buf, t, 3000, 0.07, attack=0.0005, tau=0.003)
    def chord(self, buf):
        # three soft knocks rising, over a low bloom that fades
        for k, m in enumerate((65, 69, 72)):
            self.note(buf, 0.12 * k, m, 0.8, pan=-0.2 + 0.2 * k)
        tone(buf, 0.3, hz(41), 0.35, attack=0.2, tau=0.5, partials=((1, 1), (2, 0.1)))


class Minimal(Pack):
    # quiet ticks for everything but logging in
    name, shift, gain = "minimal", 0, -2.0
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        # a tick whose pitch follows the phrase a little
        f = 900 * 2 ** ((m - 84) / 24)
        tone(buf, t, f, amp, attack=0.0008, tau=0.008, pan=pan, partials=((1, 1), (1.5, 0.2)))
    def tick(self, buf, t, kind):
        f = {"lock": 700, "unlock": 900, "volume": 1000, "error": 450}[kind]
        tone(buf, t, f, 1.0, attack=0.0008, tau=0.008 if kind != "error" else 0.02, partials=((1, 1), (1.5, 0.2)))
    def chord(self, buf):
        tone(buf, 0.0, hz(53), 0.7, attack=0.2, tau=0.6, partials=((1, 1), (2, 0.06)))
        tone(buf, 0.05, hz(60), 0.4, attack=0.25, tau=0.55, partials=((1, 1),))


def phrase(pack, name, length, notes, peak):
    """notes: (time, midi, amp, pan, ring)"""
    buf = buffer(length * pack.room)
    for t, m, amp, pan, ring in notes:
        pack.note(buf, t, m, amp, pan, ring)
    finish(buf, name, peak + pack.gain)


def build(pack):
    buf = buffer(1.6 if pack.name == "taps" else 3.2 if pack.name == "keys" else 2.6)
    pack.chord(buf)
    finish(buf, "startup", -7 + pack.gain)
    # charging: two notes up a fourth
    phrase(pack, "charging", 0.6, [(0, 79, 0.6, -0.1, 0.8), (0.09, 84, 0.7, 0.1, 1.0)], -7)
    # displays: a fifth up, or down
    phrase(pack, "display-connect", 0.6, [(0, 72, 0.6, -0.15, 0.8), (0.11, 79, 0.6, 0.15, 1.1)], -9)
    phrase(pack, "display-disconnect", 0.6, [(0, 79, 0.6, -0.15, 0.8), (0.11, 72, 0.6, 0.15, 1.1)], -9)
    for kind in ("lock", "unlock", "volume", "error"):
        buf = buffer({"volume": 0.2, "error": 0.6}.get(kind, 0.3) * pack.room)
        pack.tick(buf, 0.0, kind)
        if kind == "error":  # "no": two knocks
            pack.tick(buf, 0.11, kind)
        finish(buf, kind, {"lock": -7, "unlock": -8, "volume": -11, "error": -7}[kind] + pack.gain)
    # notification: one note, then a third below, the second quieter
    phrase(pack, "notification", 0.8, [(0, 84, 0.6, -0.1, 1.0), (0.12, 80, 0.4, 0.1, 1.3)], -10)
    # low battery: a minor third down
    phrase(pack, "battery", 0.8, [(0, 76, 0.6, -0.1, 1.0), (0.16, 73, 0.6, 0.1, 1.5)], -9)
    # USB: like displays, quicker and higher
    phrase(pack, "usb-connect", 0.4, [(0, 84, 0.5, -0.1, 0.5), (0.06, 91, 0.5, 0.1, 0.6)], -11)
    phrase(pack, "usb-disconnect", 0.4, [(0, 91, 0.5, -0.1, 0.5), (0.06, 84, 0.5, 0.1, 0.6)], -11)
    # received: a rising arpeggio that settles
    phrase(pack, "received", 0.9, [(0.05 * k, m, 0.45, -0.3 + 0.2 * k, 0.8 + 0.3 * k) for k, m in enumerate((72, 76, 79, 84))], -10)
    phrase(pack, "record-start", 0.35, [(0, 84, 0.6, 0, 0.6)], -11)
    phrase(pack, "record-stop", 0.45, [(0, 84, 0.6, 0, 0.6), (0.09, 79, 0.6, 0, 0.8)], -11)
    today(pack)


# The morning screen (Today) rises with its entrance: the first note as the
# screen frosts over, climbing an octave (G B D G) to the peak as the panel
# settles (TODAY_PEAK s in; TodayScreen times the sound so the two meet),
# then a quiet sparkle while its rows rise in. A soft swell underneath grows
# into the peak, so the whole phrase lifts rather than steps.
TODAY_PEAK = 0.66
TODAY_STEPS = ((0.0, 67, 0.30, -0.30, 1.0), (0.22, 71, 0.36, -0.12, 1.1), (0.44, 74, 0.43, 0.08, 1.3),
               (TODAY_PEAK, 79, 0.56, 0.0, 2.2), (0.86, 83, 0.17, 0.22, 1.2), (1.02, 86, 0.13, 0.34, 1.4))


def today(pack):
    buf = buffer(min(3.0, 2.0 * pack.room))
    tone(buf, 0.0, hz(max(36, 55 + pack.shift)), 0.20, attack=TODAY_PEAK, tau=0.9, pan=0.0, partials=((1, 1), (2, 0.12)))
    for t, m, amp, pan, ring in TODAY_STEPS:
        pack.note(buf, t, m, amp, pan, ring)
    finish(buf, "today", -9 + pack.gain)


def today_glass():
    buf = buffer(2.2)
    tone(buf, 0.0, hz(67), 0.16, attack=TODAY_PEAK, tau=0.9, partials=((1, 1), (2, 0.1)))
    for k, (t, m, amp, pan, ring) in enumerate(TODAY_STEPS):
        tone(buf, t, hz(m + 12), amp * 0.85, attack=0.012, tau=0.45 + 0.25 * ring, pan=pan,
             partials=((1, 1), (2.76, 0.12), (5.4, 0.04)), shimmer=0.04)
    finish(buf, "today", -9)


if __name__ == "__main__":
    import sys
    only_today = sys.argv[1:] == ["today"]  # tools/make-sounds.py today: just today.ogg in every pack
    for pack in (Soft(), Marimba(), Keys(), Taps(), Minimal(), None):
        random.seed(7)  # the same files every run
        OUT = os.path.join(ROOT, pack.name if pack else "glass")
        os.makedirs(OUT, exist_ok=True)
        print("──", os.path.basename(OUT))
        if only_today:
            today(pack) if pack else today_glass()
        elif pack:
            build(pack)
        else:
            glass_pack()
