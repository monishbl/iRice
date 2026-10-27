#!/usr/bin/env python3
"""Makes iRice's own UI sounds (shell/assets/sounds/*.ogg) from sine partials.

  tools/make-sounds.py

Pure Python (no numpy); ffmpeg encodes the result as Ogg Vorbis. The sounds
are original: a soft major-chord bloom for logging in, a glassy two-note blip
for the charger, rising / falling two-tones for displays and USB, a latch for
locking, glass taps for notifications, a pop for volume, and a low knock for
"no" (a wrong password).
"""
import math, os, random, struct, subprocess, tempfile, wave

RATE = 48000
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "shell", "assets", "sounds")


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


if __name__ == "__main__":
    random.seed(7)  # the same files every run
    os.makedirs(OUT, exist_ok=True)
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
