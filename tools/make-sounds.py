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
  minimal  quiet ticks
  glass    the first set: glassy taps, sparkles and pops (the most playful)
  material the Material look's (material/assets/sounds/material): soft, dark
           sine notes in B-flat, short and round, with quiet low ticks
Each pack has the same events (see EVENTS); Sounds.qml picks the folder.
Logging in (startup) is one signature sound, the same in every pack.
"""
import math, os, random, struct, subprocess, tempfile, wave

RATE = 48000
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "shell", "assets", "sounds")
OUT = ROOT  # the pack being written (set per pack)
EVENTS = ["startup", "charging", "display-connect", "display-disconnect", "lock", "unlock", "notification", "battery",
          "volume", "usb-connect", "usb-disconnect", "received", "record-start", "record-stop", "error", "today", "ringtone"]


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


# ── Startup: one signature for every pack ───────────────────────────────
# Logging in is the moment to be glad of, so it's the loudest sound and the
# same in every style (like a computer's own chime): a quick glint rising up
# a D-flat major chord, opening into the chord itself, warm and wide (an
# added ninth on top), then a shimmer that blooms out of it and lifts a
# little, in a long room. Bright and opening, not a boom.
# Laptop speakers play little below 150 Hz, so the depth is in overtones
# and everything under ~130 Hz is filtered out, which no small speaker
# plays anyway and which would only use up loudness.

def reverb(buf, seconds, wet):
    """A long, smooth room: four combs and two allpasses per side
    (Schroeder), each side with its own lengths so the tail is wide."""
    left, right = buf
    out = []
    for side, ch in enumerate((left, right)):
        n = len(ch)
        acc = [0.0] * n
        spread = 23 * side
        for d in (1557, 1617, 1491, 1422):
            d += spread
            g = 10 ** (-3 * d / (seconds * RATE))  # 60 dB down after `seconds`
            line = [0.0] * n
            damp, lp = 0.25, 0.0
            for i in range(n):
                fb = line[i - d] if i >= d else 0.0
                lp = fb * (1 - damp) + lp * damp
                line[i] = ch[i] + g * lp
                acc[i] += line[i] * 0.25
        for d, g in ((225 + spread, 0.6), (556 + spread, 0.6)):
            x = acc
            y = [0.0] * n
            for i in range(n):
                xd = x[i - d] if i >= d else 0.0
                yd = y[i - d] if i >= d else 0.0
                y[i] = -g * x[i] + xd + g * yd
            acc = y
        out.append(acc)
    for ch, w in zip((left, right), out):
        for i in range(len(ch)):
            ch[i] = ch[i] + wet * w[i]


def highpass(buf, cutoff, poles=2):
    """Removes what's below `cutoff` Hz (one-pole stages, 6 dB/octave each)."""
    a = 1 / (1 + 2 * math.pi * cutoff / RATE)
    for ch in buf:
        for _ in range(poles):
            px = py = 0.0
            for i in range(len(ch)):
                x = ch[i]
                py = a * (py + x - px)
                px = x
                ch[i] = py


def saturate(buf, drive):
    """A little density, like a big speaker pushed: soft clipping."""
    for ch in buf:
        k = math.tanh(drive)
        for i in range(len(ch)):
            ch[i] = math.tanh(drive * ch[i]) / k


_SIGNATURE = None  # the first pack's file, copied to the rest


def signature_startup():
    global _SIGNATURE
    dest = os.path.join(OUT, "startup.ogg")
    if _SIGNATURE:
        if os.path.abspath(_SIGNATURE) != os.path.abspath(dest):
            with open(_SIGNATURE, "rb") as a, open(dest, "wb") as b:
                b.write(a.read())
        print("startup (signature)")
        return
    rng_state = random.getstate()
    random.seed(1984)
    buf = buffer(5.0)
    # the glint: a quick run up the chord, glassy, left to right
    for k, m in enumerate((61, 65, 68, 73, 77)):
        tone(buf, 0.045 * k, hz(m), 0.32 - 0.03 * k, attack=0.003, tau=0.5, pan=-0.6 + 0.3 * k,
             partials=((1, 1), (2.0, 0.25), (3.0, 0.08), (4.2, 0.05)))
    hit = 0.24
    # the chord: D-flat major with an added ninth, from just under middle C
    # to two octaves up; three voices a few cents apart for each note (warm
    # and wide), lower notes ring longer
    warm = ((1, 1), (2, 0.55), (3, 0.3), (4, 0.16), (5, 0.08), (6, 0.04))
    chord = [(49, 0.3, -0.15), (56, 0.36, 0.3), (61, 0.42, -0.35), (65, 0.4, 0.35), (68, 0.36, -0.3),
             (75, 0.28, 0.4), (77, 0.24, -0.2), (80, 0.18, 0.25)]
    for k, (m, amp, pan) in enumerate(chord):
        for cents, p in ((-6, -0.35), (0, 0.0), (7, 0.35)):
            f = hz(m) * 2 ** (cents / 1200)
            tone(buf, hit + 0.004 * k, f, amp / 3, attack=0.014, tau=2.2 - 0.12 * k,
                 pan=max(-1, min(1, pan + p * 0.4)), partials=warm, shimmer=0.03 if m >= 75 else 0.0)
    # a warm body an octave under the chord, through its overtones
    tone(buf, hit, hz(37), 0.2, attack=0.01, tau=1.4, partials=((2, 1), (3, 0.6), (4, 0.4), (5, 0.2)))
    # the bloom: high notes swelling out of the chord, rising into tune
    for k, (m, amp, pan) in enumerate(((85, 0.14, -0.5), (89, 0.12, 0.5), (92, 0.09, -0.2), (97, 0.06, 0.3))):
        tone(buf, hit + 0.3 + 0.1 * k, hz(m), amp, attack=0.6, tau=1.3, pan=pan,
             partials=((1, 1), (2, 0.12)), shimmer=0.07, glide=0.985, glide_time=0.45)
    reverb(buf, 3.0, 0.3)
    saturate(buf, 1.25)
    highpass(buf, 130, poles=3)  # (after the saturation, which adds low difference tones)
    finish(buf, "startup", -0.8)
    random.setstate(rng_state)
    _SIGNATURE = dest


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
    signature_startup()
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
    ringtone_glass()


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


class Keys(Pack):
    # a felt electric piano: soft tine, a little bell (7x) and a slow tremolo
    name, shift, gain, room = "keys", -9, 0.0, 2.2
    tine = ((1, 1), (2, 0.3), (3, 0.06), (7.1, 0.04))
    def note(self, buf, t, m, amp, pan=0.0, ring=1.0):
        tone(buf, t, hz(m + self.shift), amp, attack=0.006, tau=0.35 * ring, pan=pan, partials=self.tine, shimmer=0.04)
    def tick(self, buf, t, kind):
        f = {"lock": hz(64), "unlock": hz(71), "volume": hz(76), "error": hz(52)}[kind]
        tone(buf, t, f, 1.0, attack=0.004, tau=0.06 if kind != "error" else 0.15, partials=self.tine)


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


def phrase(pack, name, length, notes, peak):
    """notes: (time, midi, amp, pan, ring)"""
    buf = buffer(length * pack.room)
    for t, m, amp, pan, ring in notes:
        pack.note(buf, t, m, amp, pan, ring)
    finish(buf, name, peak + pack.gain)


def build(pack):
    signature_startup()
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
    ringtone(pack)


# An incoming call (Calls.qml plays it every 2.6 s while the island rings):
# two quick rising rolls, like a phone's ring but softer, about 1.3 s long
RING_STEPS = tuple((0.6 * r + 0.1 * k, m, 0.42 + 0.06 * k, -0.2 + 0.15 * k, 0.7 + 0.2 * k)
                   for r in range(2) for k, m in enumerate((76, 79, 84, 79)))


def ringtone(pack):
    buf = buffer(1.5 * min(pack.room, 1.3))
    for t, m, amp, pan, ring in RING_STEPS:
        pack.note(buf, t, m, amp, pan, ring)
    finish(buf, "ringtone", -9 + pack.gain)


def ringtone_glass():
    buf = buffer(1.5)
    glass = ((1, 1), (2.76, 0.12), (5.4, 0.03))
    for t, m, amp, pan, ring in RING_STEPS:
        tone(buf, t, hz(m + 12), amp * 0.8, attack=0.004, tau=0.1 + 0.05 * ring, pan=pan, partials=glass)
    finish(buf, "ringtone", -9)


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


# ── Material: the Material look's own set ───────────────────────────────
# Round and quiet, like Android's: pure sines with a breath of the octave,
# short soft attacks, all in B-flat major (F, B-flat, D), so any two that
# happen together agree. Clicks are low, pitch-dropping "tocks".
MATERIAL_OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "material", "assets", "sounds", "material")
ROUND = ((1, 1), (2, 0.06))
SHORT = 0.55  # (the set's notes die away quickly: each tau below, shortened)
QUIET = -2  # dB under the other packs


def material_note(buf, t, m, amp, pan=0.0, tau=0.16, attack=0.012):
    tone(buf, t, hz(m), amp, attack=attack, tau=tau * SHORT, pan=pan, partials=ROUND)


def material_tock(buf, t, m, amp, tau=0.035):
    tone(buf, t, hz(m), amp, attack=0.002, tau=tau, partials=((1, 1), (2.4, 0.08)), glide=1.5, glide_time=0.012)


def material_pack():
    def notes(name, length, steps, peak, tau=0.16):
        buf = buffer(length)
        for t, m, amp, pan in steps:
            material_note(buf, t, m, amp, pan, tau)
        finish(buf, name, peak + QUIET)
    signature_startup()
    # charging: up a fourth, F to B-flat
    notes("charging", 0.6, [(0, 77, 0.5, -0.1), (0.09, 82, 0.6, 0.1)], -9)
    # displays: a fifth up, or down (B-flat and F)
    notes("display-connect", 0.6, [(0, 70, 0.55, -0.15), (0.11, 77, 0.6, 0.15)], -9, tau=0.18)
    notes("display-disconnect", 0.6, [(0, 77, 0.55, -0.15), (0.11, 70, 0.6, 0.15)], -9, tau=0.18)
    # notification: B-flat, then the third below, softer
    notes("notification", 0.8, [(0, 82, 0.6, -0.1), (0.12, 78, 0.42, 0.1)], -10, tau=0.24)
    # low battery: D down to B, a minor third
    notes("battery", 0.8, [(0, 74, 0.55, -0.1), (0.16, 71, 0.6, 0.1)], -9, tau=0.24)
    # USB: like displays, higher and quicker
    notes("usb-connect", 0.4, [(0, 82, 0.45, -0.1), (0.06, 89, 0.45, 0.1)], -12, tau=0.09)
    notes("usb-disconnect", 0.4, [(0, 89, 0.45, -0.1), (0.06, 82, 0.45, 0.1)], -12, tau=0.09)
    # received: up the chord, B-flat D F B-flat, settling
    notes("received", 0.9, [(0.05 * k, m, 0.42, -0.3 + 0.2 * k) for k, m in enumerate((70, 74, 77, 82))], -10, tau=0.22)
    # error: two low knocks on E-flat
    buf = buffer(0.6)
    for t in (0, 0.11):
        material_note(buf, t, 63, 0.6, 0.0, tau=0.08, attack=0.004)
    finish(buf, "error", -8 + QUIET)
    # recording: a tock to start, B-flat to F to stop
    buf = buffer(0.35)
    material_tock(buf, 0, 70, 0.7)
    finish(buf, "record-start", -11 + QUIET)
    notes("record-stop", 0.45, [(0, 82, 0.5, 0), (0.09, 77, 0.55, 0)], -11, tau=0.12)
    # lock lower, unlock higher; volume the quietest tick
    for name, m, length, peak in (("lock", 62, 0.3, -8), ("unlock", 69, 0.3, -9), ("volume", 74, 0.2, -12)):
        buf = buffer(length)
        material_tock(buf, 0, m, 0.7, tau=0.03 if name == "volume" else 0.04)
        finish(buf, name, peak + QUIET)
    # ringtone: two rolls up and back, B-flat major
    buf = buffer(1.5)
    for r in range(2):
        for k, m in enumerate((74, 77, 82, 77)):
            material_note(buf, 0.6 * r + 0.1 * k, m, 0.4 + 0.06 * k, -0.2 + 0.15 * k, tau=0.16 + 0.04 * k)
    finish(buf, "ringtone", -9 + QUIET)
    # Today: climbs F A C F to the panel's peak, a soft swell beneath
    buf = buffer(2.0)
    tone(buf, 0.0, hz(53), 0.18, attack=TODAY_PEAK, tau=0.9, partials=((1, 1), (2, 0.1)))
    for t, m, amp, pan, tau in ((0.0, 65, 0.3, -0.3, 0.3), (0.22, 69, 0.36, -0.12, 0.32), (0.44, 72, 0.43, 0.08, 0.36),
                                (TODAY_PEAK, 77, 0.56, 0.0, 0.6), (0.86, 81, 0.16, 0.22, 0.3), (1.02, 84, 0.12, 0.34, 0.34)):
        material_note(buf, t, m, amp, pan, tau)
    finish(buf, "today", -9 + QUIET)


# ── Startup sounds: more to log in to ───────────────────────────────────
# shell/assets/sounds/startup/<style>-<motif>.ogg, chosen in Settings →
# Notifications → Logging in (any of them, whichever style is on; the
# signature above stays the default). The same four motifs on each style's
# own instrument: Rise runs up the chord and
# opens into it; Hello greets with a fifth that blooms into the chord; Dawn
# swells slowly with notes sparkling over its peak; Bell is a three-note
# chime, down then a leap up, in a long room. All in D-flat, like the
# signature, under 4.5 s.
STARTUP_MOTIFS = ("rise", "hello", "dawn", "bell")
CHORD = (61, 65, 68, 72, 75)  # D-flat major with the seventh and ninth


def glass_note(buf, t, m, amp, pan=0.0, ring=1.0):
    tone(buf, t, hz(m + 12), amp * 0.8, attack=0.004, tau=0.25 * ring, pan=pan,
         partials=((1, 1), (2.76, 0.12), (5.4, 0.04)), shimmer=0.04)


def instrument(style):
    """How one note sounds in a style: (note function, its semitone shift
    already applied, whether it sustains on its own)"""
    packs = {p.name: p for p in (Soft(), Marimba(), Keys(), Taps(), Minimal())}
    if style in packs:
        p = packs[style]
        return lambda buf, t, m, amp, pan=0.0, ring=1.0: p.note(buf, t, m, amp, pan, ring), style in ("soft", "keys")
    if style == "glass":
        return glass_note, False
    return (lambda buf, t, m, amp, pan=0.0, ring=1.0: material_note(buf, t, m, amp, pan, tau=0.3 * ring)), False


def pad(buf, start, notes, amp, attack, tau):
    """A soft held chord under the motif (sines, a little detuned, wide)"""
    for k, m in enumerate(notes):
        for cents, pan in ((-5, -0.5), (5, 0.5)):
            tone(buf, start + 0.01 * k, hz(m) * 2 ** (cents / 1200), amp / 2, attack=attack, tau=tau, pan=pan,
                 partials=((1, 1), (2, 0.15), (3, 0.04)))


def startup_option(style, motif):
    note, sustains = instrument(style)
    random.seed(sum(map(ord, style + motif)))  # (the same file every run)
    buf = buffer(4.5)
    if motif == "rise":
        for k, m in enumerate(CHORD + (77, 80)):
            note(buf, 0.06 * k, m, 0.5 - 0.03 * k, -0.6 + 0.2 * k, 1.2)
        hit = 0.06 * 7 + 0.05
        for k, m in enumerate(CHORD):
            note(buf, hit, m, 0.32, -0.4 + 0.2 * k, 3.0)
        pad(buf, hit - 0.1, (49, 61, 68), 0.16, 0.25, 1.6)
        room, wet = 2.4, 0.3
    elif motif == "hello":
        note(buf, 0.0, 68, 0.6, -0.25, 1.6)
        note(buf, 0.22, 75, 0.7, 0.25, 2.4)
        for k, m in enumerate((61, 65, 68, 72)):
            note(buf, 0.5 + 0.03 * k, m, 0.22, -0.3 + 0.2 * k, 3.0)
        pad(buf, 0.4, (49, 56, 61, 65), 0.2, 0.5, 1.8)
        room, wet = 2.2, 0.28
    elif motif == "dawn":
        pad(buf, 0.0, (49, 56, 61, 65, 68), 0.3, 1.1, 1.5)
        for k, (t, m) in enumerate(((0.95, 80), (1.1, 84), (1.28, 87), (1.5, 92))):
            note(buf, t, m, 0.22 - 0.03 * k, -0.5 + 0.33 * k, 1.4)
        room, wet = 3.0, 0.35
    else:  # bell
        for k, (t, m) in enumerate(((0.0, 73), (0.32, 68), (0.64, 80))):
            note(buf, t, m, 0.6, (-0.3, 0.3, 0.0)[k], 3.5 if sustains else 2.5)
            note(buf, t, m - 12, 0.18, 0.0, 2.0)
        pad(buf, 0.64, (56, 61, 68), 0.12, 0.3, 1.6)
        room, wet = 3.2, 0.4
    reverb(buf, room, wet)
    highpass(buf, 130, poles=3)
    finish(buf, style + "-" + motif, -1.5)


if __name__ == "__main__":
    import sys
    only_today = sys.argv[1:] == ["today"]  # tools/make-sounds.py today: just today.ogg in every pack
    only_ring = sys.argv[1:] == ["ringtone"]  # tools/make-sounds.py ringtone: just ringtone.ogg
    only_startup = sys.argv[1:] == ["startup"]  # tools/make-sounds.py startup: just startup.ogg
    if sys.argv[1:2] == ["startups"]:  # tools/make-sounds.py startups [STYLE…]: the startup sounds to choose from
        OUT = os.path.join(ROOT, "startup")
        os.makedirs(OUT, exist_ok=True)
        for style in sys.argv[2:] or ("soft", "marimba", "keys", "taps", "minimal", "glass", "material"):
            print("──", style)
            for motif in STARTUP_MOTIFS:
                startup_option(style, motif)
        sys.exit(0)
    if sys.argv[1:] == ["material"]:  # tools/make-sounds.py material: just the Material look's set
        random.seed(7)
        OUT = os.path.abspath(MATERIAL_OUT)
        os.makedirs(OUT, exist_ok=True)
        material_pack()
        sys.exit(0)
    for pack in (Soft(), Marimba(), Keys(), Taps(), Minimal(), None):
        random.seed(7)  # the same files every run
        OUT = os.path.join(ROOT, pack.name if pack else "glass")
        os.makedirs(OUT, exist_ok=True)
        print("──", os.path.basename(OUT))
        if only_today:
            today(pack) if pack else today_glass()
        elif only_ring:
            ringtone(pack) if pack else ringtone_glass()
        elif only_startup:
            signature_startup()
        elif pack:
            build(pack)
        else:
            glass_pack()
    if not (only_today or only_ring or only_startup):
        random.seed(7)
        OUT = os.path.abspath(MATERIAL_OUT)
        print("── material")
        material_pack()
