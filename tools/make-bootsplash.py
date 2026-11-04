#!/usr/bin/env python3
"""Makes Tessera's boot animations: Plymouth script themes in
shell/assets/plymouth/tessera-<id>/ (Settings → Lock & Login Screen → Boot
animation; the root helper installs the one you pick, tessera-root bootsplash).

  tools/make-bootsplash.py                  the themes, and Settings' thumbnails
                                            and names (thumbnails/, designs.json)
  tools/make-bootsplash.py --preview DIR    also DIR/<id>.webm (VP9, 1280x800,
                                            a seamless loop), DIR/<id>.png and
                                            DIR/designs.json

The designs (DESIGNS), each on near-black with the Tessera mark (Phosphor's
filled circle-wavy, the badge, as in the menu bar and the look switch):
  breathe  the badge with a soft glow that breathes, and a thin progress line
  orbit    the badge inside a thin ring; two arcs chase round it, so the lit
           part grows and shrinks (like a circular progress indicator)
  frost    Glass: the badge on a frosted squircle, two soft lights drifting
           behind it, and now and then a light sweeping across the glass
  bloom    Material: the badge on a shape that morphs through Material's
           loading shapes (cookie, pentagon, pill, sunny…) on its spring,
           turning as it goes
Plymouth calls the script 50 times a second. Everything that moves is a table
with one entry per call, written into the script; the script only looks the
values up, and the preview draws the same images by the same tables (and the
same sizes and positions), so the preview is what boots. Images come in two
sizes, @1x (the mark 160 px) and @2x (320 px); the script scales the nearer
one so the mark is 14% of the screen's shorter side.

Every theme also shows Plymouth's prompts: a password or a question (a field,
its prompt, Caps Lock), messages, and what it's doing when that isn't
starting up (shutting down, restarting, installing updates).

Needs Pillow (no numpy); the previews need ffmpeg. Deterministic.
"""
import argparse
import functools
import json
import math
import os
import shutil
import subprocess
import tempfile

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "shell", "assets", "plymouth")
BADGE_FONT = os.path.join(ROOT, "shell", "assets", "fonts", "Phosphor-Fill.ttf")
BADGE = ""  # (Phosphor's circle-wavy: the Tessera badge)

FPS = 50  # Plymouth's refresh rate (the script sets it)
MARK = 0.14  # the mark, as a share of the screen's shorter side
SETS = {"@1x": 160, "@2x": 320}  # the mark's size each set of images is drawn for
SS = 4  # supersampling for edges

BG = (10, 10, 11)  # #0A0A0B
INK = (237, 237, 239)  # #EDEDEF: never pure white
SECONDARY = (161, 161, 170)  # #A1A1AA
WARNING = (251, 191, 36)  # #FBBF24 (Caps Lock)
SURFACE = (26, 26, 29)  # #1A1A1D: the field
INPUT = (117, 117, 127)  # #75757F: the field's border
INDIGO = (129, 140, 248)  # #818CF8: frost's one coloured light
# bloom: Material's dark scheme for its baseline seed (#6750A4)
M3_PRIMARY = (208, 188, 255)  # #D0BCFF
M3_ON_PRIMARY = (56, 30, 114)  # #381E72


# ── Drawing ─────────────────────────────────────────────────────────────
def smooth(size, draw):
    """A coverage mask ('L', size×size or (w, h)), drawn at SS× and reduced."""
    w, h = (size, size) if isinstance(size, int) else size
    big = Image.new("L", (w * SS, h * SS), 0)
    draw(ImageDraw.Draw(big), SS)
    return big.reduce(SS)


def paint(mask, rgb, alpha=1.0):
    im = Image.new("RGBA", mask.size, rgb + (0,))
    im.putalpha(mask.point(lambda v: round(v * alpha)))
    return im


def badge_mask(height, canvas):
    """The badge, its ink `height` px tall (a little less: it's round, where
    the mark it replaced was a drop), centred on a canvas×canvas mask."""
    height = height * 0.86
    probe = ImageFont.truetype(BADGE_FONT, 1000)
    x0, y0, x1, y1 = probe.getbbox(BADGE)
    font = ImageFont.truetype(BADGE_FONT, round(1000 * height * SS / (y1 - y0)))
    x0, y0, x1, y1 = font.getbbox(BADGE)
    big = Image.new("L", (canvas * SS, canvas * SS), 0)
    ImageDraw.Draw(big).text(((canvas * SS - (x1 - x0)) / 2 - x0, (canvas * SS - (y1 - y0)) / 2 - y0), BADGE, font=font, fill=255)
    return big.reduce(SS)


def glow(canvas, radius, blur):
    """A soft round light: a disc, blurred."""
    m = Image.new("L", (canvas, canvas), 0)
    c = canvas / 2
    ImageDraw.Draw(m).ellipse((c - radius, c - radius, c + radius, c + radius), fill=255)
    return m.filter(ImageFilter.GaussianBlur(blur))


def ring(canvas, radius, width, start=0.0, sweep=360.0):
    """A stroke of a circle (an arc from `start`°, clockwise from the top, with
    round ends when it isn't whole)."""
    def draw(d, s):
        c, r, w = canvas * s / 2, radius * s, width * s
        if sweep >= 360:
            d.ellipse((c - r - w / 2, c - r - w / 2, c + r + w / 2, c + r + w / 2), fill=255)
            d.ellipse((c - r + w / 2, c - r + w / 2, c + r - w / 2, c + r - w / 2), fill=0)
            return
        pts = []
        steps = max(8, int(sweep * 2))
        for i in range(steps + 1):
            a = math.radians(start + sweep * i / steps - 90)
            pts.append((c + r * math.cos(a), c + r * math.sin(a)))
        d.line(pts, fill=255, width=round(w), joint="curve")
        for x, y in (pts[0], pts[-1]):
            d.ellipse((x - w / 2, y - w / 2, x + w / 2, y + w / 2), fill=255)
    return smooth(canvas, draw)


def squircle_points(cx, cy, half, n=5.0, steps=720):
    """A superellipse |x|^n + |y|^n = 1, Glass's squircle."""
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        pts.append((cx + half * math.copysign(abs(c) ** (2 / n), c), cy + half * math.copysign(abs(s) ** (2 / n), s)))
    return pts


# ── Material's shapes: common/materialshape.frag's fields, in Python ────
# [kind, n, a, b, rotation°, scale], as common/MaterialShapes.js
SHAPES = {
    "circle": [0, 1, 0, 1, 0, 1],
    "pentagon": [1, 5, 0.16, 0, 0, 1.02],
    "pill": [2, 0, 1, 0.6, -45, 1],
    "oval": [3, 0, 0.64, 0, -45, 1],
    "cookie4": [0, 4, 0.16, 1, 45, 1],
    "cookie9": [0, 9, 0.08, 1, -90, 1],
    "sunny": [0, 8, 0.11, 0.55, 0, 1],
    "softBurst": [0, 10, 0.13, 0.75, 0, 1],
    "flower": [0, 8, 0.16, 1.6, 0, 1],
}


def sd_polygon(x, y, n, r):
    an = math.pi / n
    a = math.atan2(x, y)
    bn = (a + an) % (2 * an) - an
    l = math.hypot(x, y)
    qx, qy = l * math.cos(bn) - r * math.cos(an), l * abs(math.sin(bn)) - r * math.sin(an)
    qy += min(max(-qy, 0.0), r * math.sin(an))
    return math.hypot(qx, qy) * math.copysign(1, qx)


def field(px, py, shape):
    kind, n, a, b, rot, sc = shape
    t = math.radians(rot)
    c, s = math.cos(t), math.sin(t)
    x, y = (c * px + s * py) / sc, (-s * px + c * py) / sc
    if kind == 0:
        r = 1 - a * max(0.0, 0.5 - 0.5 * math.cos(n * math.atan2(y, x))) ** b
        d = (math.hypot(x, y) - r) * r
    elif kind == 1:
        d = sd_polygon(x, y, n, 1 - a) - a
    elif kind == 2:
        r = min(a, b)
        qx, qy = abs(x) - 1 + r, abs(y) - b + r
        d = math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - r
    else:
        k0 = math.hypot(x, y / a)
        k1 = math.hypot(x, y / (a * a))
        d = k0 * (k0 - 1) / max(k1, 1e-4)
    return d * sc


@functools.lru_cache(maxsize=None)
def outline(a, b, morph):
    """The edge of the morph of two shapes (the shader's mix of their
    fields), in unit coordinates. Each ray from the centre crosses it once,
    so it's found ray by ray."""
    A, B = SHAPES[a], SHAPES[b]
    pts = []
    for i in range(1440):
        th = 2 * math.pi * i / 1440
        c, s = math.cos(th), math.sin(th)
        lo, hi = 0.0, 1.6
        for _ in range(30):
            mid = (lo + hi) / 2
            px, py = c * mid, s * mid
            if (1 - morph) * field(px, py, A) + morph * field(px, py, B) < 0:
                lo = mid
            else:
                hi = mid
        pts.append((lo * c, lo * s))
    return pts


def shape_mask(a, b, morph, size, canvas):
    """A morph of two shapes, `size` px across, centred on a canvas×canvas mask."""
    pts = outline(a, b, morph)

    def draw(d, sc):
        o, u = canvas * sc / 2, size * sc / 2
        d.polygon([(o + x * u, o + y * u) for x, y in pts], fill=255)
    return smooth(canvas, draw)


def spring(t, zeta=0.6, k=200.0):
    """Material's expressive spatial spring (ζ .6, k 200, from 0 to 1) at t s."""
    w0 = math.sqrt(k)
    wd = w0 * math.sqrt(1 - zeta * zeta)
    return 1 - math.exp(-zeta * w0 * t) * (math.cos(wd * t) + zeta / math.sqrt(1 - zeta * zeta) * math.sin(wd * t))


def wave(i, period):
    """0 → 1 → 0 over a period, eased (a sine)."""
    return 0.5 - 0.5 * math.cos(2 * math.pi * i / period)


# ── The designs ─────────────────────────────────────────────────────────
# Each has: images(L) → {name: RGBA} for a mark of L px; tables → {name:
# [value per tick]}; a loop length in ticks; `extent`, how far below the
# centre its picture reaches (in marks: text and prompts go under it); its
# script (setup and frame, using the shared part's helpers), and the same
# frame for the preview (sprites: [(z, name, dx, dy, opacity, angle)]).
class Breathe:
    id, name = "breathe", "Breathe"
    description = "The badge with a soft glow that breathes, and a thin line that fills as your computer starts."
    period = 250  # 5 s
    extent = 1.15
    line = 1.6  # the progress line's width, in marks

    def images(self, L):
        c = round(1.2 * L)
        lw, lh = round(self.line * L), max(2, round(L / 64))
        line = smooth((lw, lh * 3), lambda d, s: d.rounded_rectangle((0, lh * s, lw * s - 1, 2 * lh * s - 1), radius=lh * s / 2, fill=255))
        return {
            "mark": paint(badge_mask(L, c), INK),
            "glow": paint(glow(round(3.8 * L), 0.5 * L, 0.36 * L), INK),
            "track": paint(line, INK, 0.14),
            "fill": paint(line, INK, 0.8),
        }

    def tables(self):
        w = [wave(i, self.period) for i in range(self.period)]
        return {"glow": [round(0.08 + 0.3 * v, 4) for v in w], "markop": [round(0.84 + 0.16 * v, 4) for v in w]}

    script = r'''
mark.image = scaled("mark"); mark.sprite = Sprite(mark.image);
glow.image = scaled("glow"); glow.sprite = Sprite(glow.image);
track.image = scaled("track"); track.sprite = Sprite(track.image);
fill.full = scaled("fill"); fill.sprite = Sprite();
global.shown = 0; global.target = 0; global.cut = -1;
fun place() {
  centre(mark.sprite, mark.image, 0, 0, 2);
  centre(glow.sprite, glow.image, 0, 0, 1);
  centre(track.sprite, track.image, 0, Math.Int(global.L * 0.95), 1);
  global.linex = global.cx - Math.Int(fill.full.GetWidth() / 2);
  global.liney = global.cy + Math.Int(global.L * 0.95) - Math.Int(fill.full.GetHeight() / 2);
  fill.sprite.SetPosition(global.linex, global.liney, 2);
  global.cut = -1;
}
fun frame(i) {
  glow.sprite.SetOpacity(t_glow[i]);
  mark.sprite.SetOpacity(t_markop[i]);
  # the line: only while starting up, appearing as it starts to fill
  local.show = 0;
  if (global.mode == "boot") show = Math.Min(1, global.age / 15);
  track.sprite.SetOpacity(show);
  fill.sprite.SetOpacity(show);
  global.shown = global.shown + (global.target - global.shown) * 0.08;
  local.w = Math.Int(fill.full.GetWidth() * global.shown);
  if (w != global.cut && w > 0) {
    fill.sprite.SetImage(fill.full.Crop(0, 0, w, fill.full.GetHeight()));
    global.cut = w;
  }
}
fun progress(duration, p) { global.target = Math.Clamp(p, 0, 1); }
Plymouth.SetBootProgressFunction(progress);
'''

    def sprites(self, i, t, L, state):
        ly = int(L * 0.95)
        show = min(1, state["age"] / 15) * state.get("lineop", 1)
        out = [(1, "glow", 0, 0, t["glow"][i], 0), (2, "mark", 0, 0, t["markop"][i], 0), (1, "track", 0, ly, show, 0)]
        if state["cut"] > 0:
            out.append((2, ("fill", state["cut"]), "left", ly, show, 0))
        return out


class Orbit:
    id, name = "orbit", "Orbit"
    description = "The badge inside a thin ring, with a light chasing round it that grows and shrinks."
    period = 120  # 2.4 s
    extent = 1.15

    def images(self, L):
        c = round(2.2 * L)
        r, w = 0.95 * L, max(2, L / 48)
        return {
            "mark": paint(badge_mask(round(0.62 * L), round(0.8 * L)), INK),
            "track": paint(ring(c, r, w), INK, 0.12),
            "arc": paint(ring(c, r, w, -45, 90), INK, 0.9),
        }

    def tables(self):
        # one turn a period; the two 90° arcs part by up to 88° (so the lit
        # part grows from a quarter to half the ring, never in two) and
        # closes again, the leading one ahead, the trailing one catching up
        head, tail = [], []
        for i in range(self.period):
            base = 2 * math.pi * i / self.period
            gap = math.radians(88) * wave(i, self.period) ** 1.3
            head.append(round(base + gap / 2, 5))
            tail.append(round(base - gap / 2, 5))
        return {"head": head, "tail": tail}

    script = r'''
mark.image = scaled("mark"); mark.sprite = Sprite(mark.image);
track.image = scaled("track"); track.sprite = Sprite(track.image);
arc.image = scaled("arc"); head.sprite = Sprite(); tail.sprite = Sprite();
fun place() {
  centre(mark.sprite, mark.image, 0, 0, 3);
  centre(track.sprite, track.image, 0, 0, 1);
  centre(head.sprite, arc.image, 0, 0, 2);
  centre(tail.sprite, arc.image, 0, 0, 2);
}
fun frame(i) {
  head.sprite.SetImage(arc.image.Rotate(t_head[i]));
  tail.sprite.SetImage(arc.image.Rotate(t_tail[i]));
}
'''

    def sprites(self, i, t, L, state):
        return [(1, "track", 0, 0, 1, 0), (2, "arc", 0, 0, 1, t["head"][i]), (2, "arc", 0, 0, 1, t["tail"][i]),
                (3, "mark", 0, 0, 1, 0)]


class Frost:
    id, name = "frost", "Frost"
    description = "For Glass: the badge on a frosted glass tile, soft lights drifting behind it and a light sweeping across."
    period = 240  # 4.8 s
    extent = 1.05
    tile = 1.55  # the tile's size, in marks
    sweep_at, sweep_len = 150, 48  # the sweep: from tick 150, 48 ticks (one frame every 2)

    def tile_mask(self, L, c, inset=0.0):
        half = self.tile * L / 2 - inset
        return smooth(c, lambda d, s: d.polygon(squircle_points(c * s / 2, c * s / 2, half * s), fill=255))

    def images(self, L):
        c = round((self.tile + 0.1) * L)
        outer = self.tile_mask(L, c)
        inner = self.tile_mask(L, c, max(1.0, L / 110))
        # the glass: a milky fill, lighter at the top, and a rim of light that's
        # brightest along the top edge
        grad = Image.new("L", (c, c))
        grad.putdata([round(255 * (0.13 - 0.07 * y / c)) for y in range(c) for _ in range(c)])
        rimlight = Image.new("L", (c, c))
        rimlight.putdata([round(255 * (0.55 - 0.4 * min(1, y / (0.6 * c)))) for y in range(c) for _ in range(c)])
        fill = ImageChops.multiply(inner, grad)
        rim = ImageChops.multiply(ImageChops.subtract(outer, inner), rimlight)
        glass = Image.new("RGBA", (c, c), INK + (0,))
        glass.putalpha(ImageChops.add(fill, rim))
        out = {
            "glass": glass,
            "mark": paint(badge_mask(round(0.72 * L), round(0.9 * L)), INK),
            "light": paint(glow(round(2.4 * L), 0.5 * L, 0.38 * L), INDIGO, 0.55),
            "white": paint(glow(round(2.0 * L), 0.4 * L, 0.32 * L), INK, 0.3),
        }
        # the sweep, clipped to the tile: a soft diagonal band crossing it
        for f in range(self.sweep_len // 2):
            u = -1.3 + 2.6 * f / (self.sweep_len // 2 - 1)
            band = Image.new("L", (c, c), 0)
            d = ImageDraw.Draw(band)
            mid, w = u * c * 0.75, 0.11 * c
            d.polygon([(c / 2 + mid - w - c, c / 2 - c), (c / 2 + mid + w - c, c / 2 - c),
                       (c / 2 + mid + w + c, c / 2 + c), (c / 2 + mid - w + c, c / 2 + c)], fill=46)
            band = band.filter(ImageFilter.GaussianBlur(0.06 * c))
            out[f"sweep{f:02d}"] = paint(ImageChops.multiply(band, outer), INK)
        return out

    def tables(self):
        P = self.period
        # two lights on slow ellipses behind the tile, opposite each other
        t = {"ax": [], "ay": [], "bx": [], "by": [], "sweep": []}
        for i in range(P):
            a = 2 * math.pi * i / P
            t["ax"].append(round(0.62 * math.cos(a), 4))
            t["ay"].append(round(0.34 * math.sin(a) - 0.1, 4))
            t["bx"].append(round(0.55 * math.cos(a + math.pi), 4))
            t["by"].append(round(0.3 * math.sin(a + math.pi) + 0.12, 4))
            k = i - self.sweep_at
            t["sweep"].append(k // 2 if 0 <= k < self.sweep_len else -1)
        return t

    def sweep_names(self):
        return [f"sweep{f:02d}" for f in range(self.sweep_len // 2)]

    @property
    def script(self):
        names = "".join(f'sweepnames[{f}] = "{n}"; ' for f, n in enumerate(self.sweep_names()))
        return r'''
glass.image = scaled("glass"); glass.sprite = Sprite(glass.image);
mark.image = scaled("mark"); mark.sprite = Sprite(mark.image);
lighta.image = scaled("light"); lighta.sprite = Sprite(lighta.image);
lightb.image = scaled("white"); lightb.sprite = Sprite(lightb.image);
''' + names + r'''
for (f = 0; sweepnames[f]; f++) sweeps[f] = scaled(sweepnames[f]);
sweep.sprite = Sprite();
fun place() {
  centre(glass.sprite, glass.image, 0, 0, 2);
  centre(mark.sprite, mark.image, 0, 0, 4);
  global.sweepx = global.cx - Math.Int(sweeps[0].GetWidth() / 2);
  global.sweepy = global.cy - Math.Int(sweeps[0].GetHeight() / 2);
  sweep.sprite.SetPosition(global.sweepx, global.sweepy, 3);
}
fun frame(i) {
  centre(lighta.sprite, lighta.image, Math.Int(t_ax[i] * global.L), Math.Int(t_ay[i] * global.L), 1);
  centre(lightb.sprite, lightb.image, Math.Int(t_bx[i] * global.L), Math.Int(t_by[i] * global.L), 1);
  if (t_sweep[i] < 0) sweep.sprite.SetOpacity(0);
  else {
    sweep.sprite.SetImage(sweeps[t_sweep[i]]);
    sweep.sprite.SetOpacity(1);
  }
}
'''

    def sprites(self, i, t, L, state):
        out = [(1, "light", int(t["ax"][i] * L), int(t["ay"][i] * L), 1, 0),
               (1, "white", int(t["bx"][i] * L), int(t["by"][i] * L), 1, 0),
               (2, "glass", 0, 0, 1, 0), (4, "mark", 0, 0, 1, 0)]
        if t["sweep"][i] >= 0:
            out.append((3, self.sweep_names()[t["sweep"][i]], 0, 0, 1, 0))
        return out


class Bloom:
    id, name = "bloom", "Bloom"
    description = "For Material: the badge on a shape that morphs through Material's loading shapes, turning as it goes."
    # Material's loading indicator: a new shape every 600 ms on the expressive
    # spring, a quarter turn with each, and a whole turn a cycle (eight shapes:
    # 8 × 90° + 360° = three whole turns, so the cycle loops seamlessly)
    cycle = ["softBurst", "cookie9", "pentagon", "pill", "sunny", "cookie4", "oval", "flower"]
    step = 30  # ticks a shape
    period = len(cycle) * step  # 4.8 s
    extent = 0.85
    size = 1.25  # the shape across, in marks

    def frames(self):
        """[(from, to, morph)] the morph images (one every 2 ticks while a
        morph moves, the settled shape after), and the frame per tick."""
        keys, per_tick = [], []
        for i in range(self.period):
            j, k = divmod(i, self.step)
            a, b = self.cycle[j - 1], self.cycle[j]
            m = round(spring((k - k % 2) / FPS), 2)
            key = (b, b, 1.0) if abs(m - 1) < 0.01 else (a, b, m)
            if key not in keys:
                keys.append(key)
            per_tick.append(keys.index(key))
        return keys, per_tick

    def images(self, L):
        c = round(self.size * L * 1.25)  # (room to turn: no shape reaches past its circle's 1.2)
        out = {"mark": paint(badge_mask(round(0.5 * L), round(0.7 * L)), M3_ON_PRIMARY)}
        for n, (a, b, m) in enumerate(self.frames()[0]):
            out[f"shape{n:03d}"] = paint(shape_mask(a, b, m, self.size * L, c), M3_PRIMARY)
        return out

    def tables(self):
        turn = []
        for i in range(self.period):
            j, k = divmod(i, self.step)
            quarter = (j + spring(k / FPS)) * math.pi / 2
            turn.append(round(quarter + 2 * math.pi * i / self.period, 5))
        return {"shape": self.frames()[1], "turn": turn}

    @property
    def script(self):
        names = "".join(f'shapenames[{n}] = "shape{n:03d}"; ' for n in range(len(self.frames()[0])))
        return r'''
mark.image = scaled("mark"); mark.sprite = Sprite(mark.image);
''' + names + r'''
for (f = 0; shapenames[f]; f++) shapes[f] = scaled(shapenames[f]);
shape.sprite = Sprite();
fun place() {
  centre(mark.sprite, mark.image, 0, 0, 2);
  centre(shape.sprite, shapes[0], 0, 0, 1);
}
fun frame(i) {
  shape.sprite.SetImage(shapes[t_shape[i]].Rotate(t_turn[i]));
}
'''

    def sprites(self, i, t, L, state):
        return [(1, f"shape{t['shape'][i]:03d}", 0, 0, 1, t["turn"][i]), (2, "mark", 0, 0, 1, 0)]


DESIGNS = [Breathe(), Orbit(), Frost(), Bloom()]


# ── The shared part of every script: sizes, prompts, messages, modes ────
COMMON = r'''
Window.SetBackgroundTopColor(@BG@);
Window.SetBackgroundBottomColor(@BG@);
Plymouth.SetRefreshRate(50);

# An image at this screen's size: the nearer set, scaled so the mark is L
fun scaled(name) {
  local.img = Image(name + global.suffix);
  return img.Scale(Math.Int(img.GetWidth() * global.k + 0.5), Math.Int(img.GetHeight() * global.k + 0.5));
}
# A sprite's image centred at (dx, dy) from the screen's centre
fun centre(sprite, img, dx, dy, z) {
  sprite.SetPosition(global.cx + dx - Math.Int(img.GetWidth() / 2), global.cy + dy - Math.Int(img.GetHeight() / 2), z);
}
fun measure() {
  global.W = Window.GetWidth(); global.H = Window.GetHeight();
  global.L = Math.Int(Math.Min(global.W, global.H) * @MARK@);
  if (global.L > 200) { global.suffix = "@2x.png"; global.ref = 320; global.ui = 2; global.font = "Inter 20"; }
  else { global.suffix = "@1x.png"; global.ref = 160; global.ui = 1; global.font = "Inter 11"; }
  global.k = global.L / global.ref;
  global.cx = Window.GetX() + Math.Int(global.W / 2);
  global.cy = Window.GetY() + Math.Int(global.H / 2);
  global.below = global.cy + Math.Int(global.L * @EXTENT@) + 24 * global.ui;
}
measure();

# A whole number as text
digit[0] = "0"; digit[1] = "1"; digit[2] = "2"; digit[3] = "3"; digit[4] = "4";
digit[5] = "5"; digit[6] = "6"; digit[7] = "7"; digit[8] = "8"; digit[9] = "9";
fun number(n) {
  local.n = Math.Int(n); local.s = "";
  if (n <= 0) return "0";
  while (n > 0) { s = digit[n - Math.Int(n / 10) * 10] + s; n = Math.Int(n / 10); }
  return s;
}
fun text(s, rgb) {
  if (rgb == "warning") return Image.Text(s, @WARNING@, 1, global.font);
  if (rgb == "ink") return Image.Text(s, @INK@, 1, global.font);
  return Image.Text(s, @SECONDARY@, 1, global.font);
}

# What it's doing, under the picture, when it isn't starting up
global.mode = Plymouth.GetMode();
caption.sprite = Sprite(); caption.text = "";
if (global.mode == "shutdown") caption.text = "Shutting down";
if (global.mode == "reboot") caption.text = "Restarting";
if (global.mode == "updates" || global.mode == "system-upgrade") caption.text = "Installing updates. Keep your computer on.";
if (global.mode == "firmware-upgrade") caption.text = "Updating firmware. Keep your computer on.";
fun update_progress(p) {
  if (caption.text != "") caption.sprite.SetImage(text(caption.text + " " + number(p) + "%", "secondary"));
  stack();
}
Plymouth.SetSystemUpdateFunction(update_progress);

# Prompts: a password (bullets), or a question (its answer shown)
dialog.on = 0;
prompt.sprite = Sprite(); field.sprite = Sprite(); answer.sprite = Sprite(); caps.sprite = Sprite();
message.sprite = Sprite();
fun show_dialog(label, bullets, typed) {
  global.dialog.on = 1;
  if (!field.image) { field.image = Image("field" + global.suffix); global.dot.image = Image("dot" + global.suffix); }
  prompt.sprite.SetImage(text(label, "secondary"));
  field.sprite.SetImage(field.image);
  for (b = 0; global.bullet[b].sprite || b < bullets; b++) {
    if (!global.bullet[b].sprite) global.bullet[b].sprite = Sprite(global.dot.image);
    global.bullet[b].sprite.SetOpacity(0);
  }
  global.bullets = bullets;
  if (typed != "") answer.sprite.SetImage(text(typed, "ink"));
  else answer.sprite = Sprite();
  stack();
}
fun hide_dialog() {
  global.dialog.on = 0;
  stack();
}
fun password(label, bullets) {
  if (label == "") label = "Enter the password to unlock this disk";
  show_dialog(label, bullets, "");
}
fun question(label, typed) { show_dialog(label, 0, typed); }
Plymouth.SetDisplayPasswordFunction(password);
Plymouth.SetDisplayQuestionFunction(question);
Plymouth.SetDisplayNormalFunction(hide_dialog);
fun show_message(s) { message.sprite.SetImage(text(s, "secondary")); stack(); }
fun hide_message(s) { message.sprite = Sprite(); stack(); }
Plymouth.SetDisplayMessageFunction(show_message);
Plymouth.SetHideMessageFunction(hide_message);

# Everything under the picture, top to bottom: the caption, the prompt and
# its field (with Caps Lock's warning), the latest message
fun row(sprite, y, op) {
  local.img = sprite.GetImage();
  if (!img) return y;
  sprite.SetPosition(global.cx - Math.Int(img.GetWidth() / 2), y, 20);
  sprite.SetOpacity(op);
  return y + img.GetHeight() + 8 * global.ui;
}
fun stack() {
  local.y = global.below;
  local.d = global.dialog.on;
  caption.sprite.SetOpacity(0); prompt.sprite.SetOpacity(0); field.sprite.SetOpacity(0);
  answer.sprite.SetOpacity(0); caps.sprite.SetOpacity(0);
  for (b = 0; global.bullet[b].sprite; b++) global.bullet[b].sprite.SetOpacity(0);
  if (caption.text != "" && !d) y = row(caption.sprite, y, 1);
  if (d) {
    y = row(prompt.sprite, y, 1);
    local.fx = global.cx - Math.Int(field.image.GetWidth() / 2);
    field.sprite.SetPosition(fx, y, 20);
    field.sprite.SetOpacity(1);
    local.fh = field.image.GetHeight();
    # the latest bullets that fit (a long password scrolls)
    local.step = 14 * global.ui;
    local.fit = Math.Int((field.image.GetWidth() - 28 * global.ui) / step);
    local.first = Math.Max(0, global.bullets - fit);
    for (b = first; b < global.bullets; b++)
      global.bullet[b].sprite.SetPosition(fx + 14 * global.ui + (b - first) * step, y + Math.Int((fh - global.dot.image.GetHeight()) / 2), 21);
    for (b = first; b < global.bullets; b++) global.bullet[b].sprite.SetOpacity(1);
    if (answer.sprite.GetImage()) {
      answer.sprite.SetPosition(fx + 14 * global.ui, y + Math.Int((fh - answer.sprite.GetImage().GetHeight()) / 2), 21);
      answer.sprite.SetOpacity(1);
    }
    y = y + fh + 8 * global.ui;
    if (Plymouth.GetCapslockState()) {
      caps.sprite.SetImage(text("Caps Lock is on", "warning"));
      y = row(caps.sprite, y, 1);
    }
  }
  y = row(message.sprite, y, 1);
}
if (caption.text != "") caption.sprite.SetImage(text(caption.text, "secondary"));
'''

LOOP = r'''
place();
stack();
global.tick = 0; global.age = 0;
fun refresh() {
  frame(global.tick);
  global.tick = global.tick + 1;
  if (global.tick >= @PERIOD@) global.tick = 0;
  global.age = global.age + 1;
  # (Caps Lock can change while the field shows)
  if (global.dialog.on && Plymouth.GetCapslockState() != global.caps) { global.caps = Plymouth.GetCapslockState(); stack(); }
}
Plymouth.SetRefreshFunction(refresh);
# Another display: everything sized and placed again
fun hotplug() { measure(); @RESCALE@ place(); stack(); }
Plymouth.SetDisplayHotplugFunction(hotplug);
'''


def rgb(c):
    return ", ".join(f"{v / 255:.3f}" for v in c)


def script_for(d):
    tables = d.tables()
    lines = [f"# Tessera's boot animation: {d.name} (tools/make-bootsplash.py; made, not edited)",
             f"# {d.description}"]
    common = COMMON.replace("@BG@", rgb(BG)).replace("@MARK@", str(MARK)).replace("@EXTENT@", str(d.extent))
    common = common.replace("@WARNING@", rgb(WARNING)).replace("@INK@", rgb(INK)).replace("@SECONDARY@", rgb(SECONDARY))
    lines.append(common)
    lines.append("# The tables: one value per refresh")
    for name, values in tables.items():
        lines.append(" ".join(f"t_{name}[{i}] = {v};" for i, v in enumerate(values)))
    lines.append(d.script)
    # after a hotplug the images are made again at the new size (the design's
    # setup is its top lines, up to its first function)
    setup = d.script.split("fun ")[0].strip().replace("\n", " ")
    lines.append(LOOP.replace("@PERIOD@", str(d.period)).replace("@RESCALE@", setup))
    return "\n".join(lines)


def plymouth_file(d):
    tid = "tessera-" + d.id
    return f"""[Plymouth Theme]
Name={tid}
Description=Tessera: {d.description}
ModuleName=script

[script]
ImageDir=/usr/share/plymouth/themes/{tid}
ScriptFile=/usr/share/plymouth/themes/{tid}/{tid}.script
ConsoleLogBackgroundColor=0x0a0a0b
Font=Inter 11
"""


def ui_images(L):
    """The field and a bullet, for a set (L: its mark size, 160 or 320)."""
    u = L // 160
    w, h = 280 * u, 36 * u

    def field(d, s):
        d.rounded_rectangle((0, 0, w * s - 1, h * s - 1), radius=10 * u * s, fill=255)
    outer = smooth((w, h), field)
    inner = smooth((w, h), lambda d, s: d.rounded_rectangle((u * s, u * s, (w - u) * s - 1, (h - u) * s - 1), radius=(10 - 1) * u * s, fill=255))
    f = Image.new("RGBA", (w, h), INPUT + (0,))
    f.putalpha(outer)
    f.alpha_composite(paint(inner, SURFACE))
    dot = smooth(8 * u, lambda d, s: d.ellipse((0, 0, 8 * u * s - 1, 8 * u * s - 1), fill=255))
    return {"field": f, "dot": paint(dot, INK)}


def write_theme(d, out):
    folder = os.path.join(out, "tessera-" + d.id)
    shutil.rmtree(folder, ignore_errors=True)
    os.makedirs(folder)
    sets = {}
    for suffix, L in SETS.items():
        ims = dict(d.images(L), **ui_images(L))
        sets[suffix] = ims
        for name, im in ims.items():
            im.save(os.path.join(folder, name + suffix + ".png"), optimize=True)
    with open(os.path.join(folder, f"tessera-{d.id}.script"), "w") as f:
        f.write(script_for(d))
    with open(os.path.join(folder, f"tessera-{d.id}.plymouth"), "w") as f:
        f.write(plymouth_file(d))
    return sets


# ── Previews: the script's frame, drawn the same way ────────────────────
class Screen:
    """One screen's worth of the script: the same sizes and positions
    (measure, scaled, centre) and its sprites drawn in their order."""

    def __init__(self, d, sets, W, H, L=None):
        self.d, self.W, self.H = d, W, H
        self.L = L or int(min(W, H) * MARK)
        self.suffix, ref = ("@2x", 320) if self.L > 200 else ("@1x", 160)
        self.k = self.L / ref
        self.cx, self.cy = W // 2, H // 2
        self.images = {}
        for name, im in sets[self.suffix].items():
            # (Plymouth's Image.Scale samples bilinearly)
            self.images[name] = im.resize((int(im.width * self.k + 0.5), int(im.height * self.k + 0.5)), Image.BILINEAR)
        self.tables = d.tables()
        self.state = {"age": 0, "cut": -1, "shown": 0.0, "target": 0.0}

    def frame(self, i):
        canvas = Image.new("RGBA", (self.W, self.H), BG + (255,))
        for z, name, dx, dy, op, angle in sorted(self.d.sprites(i, self.tables, self.L, self.state), key=lambda s: s[0]):
            if isinstance(name, tuple):  # (a cropped image: the progress line's fill)
                im = self.images[name[0]].crop((0, 0, name[1], self.images[name[0]].height))
                full = self.images[name[0]]
                x, y = self.cx - full.width // 2, self.cy + dy - full.height // 2
            else:
                im = self.images[name]
                x, y = self.cx + dx - im.width // 2, self.cy + dy - im.height // 2
            if angle:
                # Plymouth's Image.Rotate turns clockwise on screen (y down) and
                # keeps the size; Pillow's turns the other way
                im = im.rotate(-math.degrees(angle), resample=Image.BILINEAR)
            if op < 1:
                im = im.copy()
                im.putalpha(im.getchannel("A").point(lambda v: round(v * op)))
            if op > 0:
                layer = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
                layer.paste(im, (x, y))
                canvas = Image.alpha_composite(canvas, layer)
        return canvas.convert("RGB")

    def progress(self, target):
        """What the script does before drawing a frame: the line eases toward
        the boot's progress (Plymouth's progress callback sets it)."""
        s = self.state
        s["target"] = target
        s["shown"] += (s["target"] - s["shown"]) * 0.08
        if isinstance(self.d, Breathe):
            w = int(self.images["fill"].width * s["shown"])
            if w > 0:
                s["cut"] = w


def boot_progress(t, length):
    """A stand-in for Plymouth's boot progress, in steps (as it comes)."""
    for at, p in ((0.85, 1.0), (0.62, 0.78), (0.4, 0.52), (0.2, 0.3), (0.04, 0.12)):
        if t >= at * length:
            return p
    return 0.0


def preview(d, sets, out, W=1280, H=800, seconds=4.8):
    ticks = round(seconds * FPS) // d.period * d.period or d.period
    if ticks / FPS < 4:
        ticks *= 2
    scr = Screen(d, sets, W, H)
    tmp = tempfile.mkdtemp(prefix="bootsplash-")
    try:
        for i in range(ticks):
            # (the preview ends as a boot would: the line fades as the desktop comes)
            scr.state["lineop"] = min(1.0, (ticks - 1 - i) / 20)
            scr.progress(boot_progress(i, ticks))
            scr.frame(i % d.period).save(os.path.join(tmp, f"f{i:04d}.png"))
            scr.state["age"] += 1
        poster_tick = {"breathe": int(ticks * 0.5), "orbit": int(d.period * 0.45), "frost": d.sweep_at + 20 if isinstance(d, Frost) else 0, "bloom": 75}[d.id]
        shutil.copy(os.path.join(tmp, f"f{poster_tick:04d}.png"), os.path.join(out, d.id + ".png"))
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-framerate", str(FPS), "-i", os.path.join(tmp, "f%04d.png"),
                        "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "24", "-pix_fmt", "yuv420p", "-row-mt", "1",
                        "-deadline", "good", os.path.join(out, d.id + ".webm")], check=True)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return ticks


def thumbnail(d, sets, path):
    """Settings' picture of a design: its middle, 320×200 (the mark 84 px)."""
    scr = Screen(d, sets, 640, 400, L=84)
    for i in range(60):
        scr.progress(0.6)
        scr.state["age"] += 1
    tick = {"breathe": 125, "orbit": 54, "frost": 170, "bloom": 75}[d.id]
    scr.frame(tick).crop((160, 100, 480, 300)).save(path, optimize=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--preview", metavar="DIR", help="also render the previews into DIR")
    ap.add_argument("--only", metavar="ID", help="one design")
    a = ap.parse_args()
    os.makedirs(os.path.join(OUT, "thumbnails"), exist_ok=True)
    for d in DESIGNS:
        if a.only and d.id != a.only:
            continue
        sets = write_theme(d, OUT)
        thumbnail(d, sets, os.path.join(OUT, "thumbnails", d.id + ".png"))
        print("made tessera-" + d.id)
        if a.preview:
            os.makedirs(a.preview, exist_ok=True)
            n = preview(d, sets, a.preview)
            print(f"  preview: {n / FPS:.1f} s")
    names = [{"id": d.id, "name": d.name, "description": d.description} for d in DESIGNS]
    for folder in [OUT] + ([a.preview] if a.preview else []):
        with open(os.path.join(folder, "designs.json"), "w") as f:
            json.dump(names, f, indent=1)
            f.write("\n")


if __name__ == "__main__":
    main()
