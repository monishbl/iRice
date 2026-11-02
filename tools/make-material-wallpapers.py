#!/usr/bin/env python3
"""Makes the Material look's default wallpapers (original artwork):
shell/assets/wallpapers/material-light.jpg and material-dark.jpg, 3840x2160.

  tools/make-material-wallpapers.py [--size WxH] [--out DIR]

Large soft shapes from Material 3 Expressive's shape library (a nine-scallop
cookie, an eight-petal flower, a pill, a four-leaf clover and a circle),
overlapping on a tonal ground: one lavender-to-violet family with a warm
peach accent, so Material You draws a calm violet palette with a peach
tertiary from it. Each shape has a gentle diagonal light, a soft shadow
underneath, and the whole picture a fine grain (no banding, and it reads as
printed rather than flat). The light and dark pictures are the same
composition in light and dark tones; Wallpaper.qml shows them in the
Material look while you're on the bundled default.

Needs Pillow (no numpy). Deterministic: the same files every run.
"""
import argparse
import math
import os
import random

from PIL import Image, ImageChops, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "shell", "assets", "wallpapers")

# Shapes in 3840x2160 design units: (kind, centre x, centre y, size, rotation degrees, colour key)
# Back to front. The middle stays fairly open for windows and desktop icons.
LAYOUT = [
    ("pill", 2420, 1720, (2300, 760), -28, "pill"),
    ("flower", 3330, 360, 930, 8, "flower"),
    ("cookie", 560, 1830, 1080, 12, "cookie"),
    ("clover", 1520, 330, 360, 20, "clover"),
    ("circle", 3020, 1330, 175, 0, "dot"),
    ("cookie", 120, 260, 300, 0, "small"),
]

# Each colour is (lit, shaded): the shape's light comes from the top left
PALETTES = {
    "light": {
        "ground": ("#F1ECF8", "#E3DCF1"),
        "pill": ("#D8E3F6", "#C3D3F0"),
        "flower": ("#FAD3C4", "#F3B7A2"),
        "cookie": ("#CEC6F4", "#B4AAEC"),
        "clover": ("#8F80E3", "#7564D4"),
        "dot": ("#F39A7E", "#E67F62"),
        "small": ("#E7DFFA", "#D6CBF6"),
        "shadow": (0.10, "#2B2340"),
        "grain": 0.035,
    },
    "dark": {
        "ground": ("#1B1726", "#121019"),
        "pill": ("#26324A", "#1E283D"),
        "flower": ("#7E4C45", "#663B36"),
        "cookie": ("#3A3368", "#2E2856"),
        "clover": ("#7565CF", "#5E4FB8"),
        "dot": ("#C9765D", "#AE604A"),
        "small": ("#2A2440", "#231E36"),
        "shadow": (0.42, "#000000"),
        "grain": 0.04,
    },
}


def polar(kind, r, steps=720):
    """The outline as (angle, radius) pairs, radius r at its widest."""
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        if kind == "cookie":  # nine soft scallops
            k = 1 + 0.075 * math.cos(9 * t)
            k /= 1.075
        elif kind == "flower":  # eight round petals
            k = 0.8 + 0.2 * abs(math.cos(4 * t)) ** 0.7
        elif kind == "clover":  # four round leaves
            k = 0.62 + 0.38 * abs(math.cos(2 * t)) ** 0.55
        else:  # circle
            k = 1.0
        pts.append((t, r * k))
    return pts


def outline(kind, cx, cy, size, rot, s):
    a = math.radians(rot)
    if kind == "pill":
        w, h = size[0] * s, size[1] * s
        rad = h / 2
        pts = []
        # a capsule: two half circles joined by straight sides
        for i in range(181):
            t = math.pi / 2 + math.pi * i / 180
            pts.append((-w / 2 + rad + rad * math.cos(t), rad * math.sin(t)))
        for i in range(181):
            t = -math.pi / 2 + math.pi * i / 180
            pts.append((w / 2 - rad + rad * math.cos(t), rad * math.sin(t)))
    else:
        pts = [(rr * math.cos(t), rr * math.sin(t)) for t, rr in polar(kind, size * s)]
    ca, sa = math.cos(a), math.sin(a)
    return [(cx * s + x * ca - y * sa, cy * s + x * sa + y * ca) for x, y in pts]


def hexrgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def lit(size, c1, c2, angle=35):
    """A colour field lit from the top left: c1 there, c2 at the far corner
    (the light comes from `angle` degrees below the horizontal)."""
    n = 64
    dx, dy = math.cos(math.radians(angle)), math.sin(math.radians(angle))
    lo, hi = min(0, dx) + min(0, dy), max(0, dx) + max(0, dy)
    g = Image.new("L", (n, n))
    g.putdata([int(255 * ((x / (n - 1)) * dx + (y / (n - 1)) * dy - lo) / (hi - lo)) for y in range(n) for x in range(n)])
    g = g.resize(size, Image.BICUBIC)
    return Image.composite(Image.new("RGB", size, hexrgb(c2)), Image.new("RGB", size, hexrgb(c1)), g)


def render(pal, w, h):
    s = w / 3840
    img = lit((w, h), *pal["ground"], angle=60)
    alpha, shade = pal["shadow"]
    shadow = Image.new("RGB", (w, h), hexrgb(shade))
    for kind, cx, cy, size, rot, key in LAYOUT:
        pts = outline(kind, cx, cy, size, rot, s)
        # a soft shadow a little below, then the shape (edges smoothed by a
        # hair of blur: Pillow's polygons aren't antialiased)
        sh = Image.new("L", (w, h), 0)
        ImageDraw.Draw(sh).polygon([(x, y + 26 * s) for x, y in pts], fill=255)
        sh = sh.filter(ImageFilter.GaussianBlur(56 * s)).point(lambda v: int(v * alpha))
        img = Image.composite(shadow, img, sh)
        mask = Image.new("L", (w, h), 0)
        ImageDraw.Draw(mask).polygon(pts, fill=255)
        mask = mask.filter(ImageFilter.GaussianBlur(1.2 * s))
        # the shape's own light, across its own extent
        x0 = max(0, int(min(p[0] for p in pts)) - 4)
        y0 = max(0, int(min(p[1] for p in pts)) - 4)
        x1 = min(w, int(max(p[0] for p in pts)) + 4)
        y1 = min(h, int(max(p[1] for p in pts)) + 4)
        if x1 <= x0 or y1 <= y0:
            continue
        box = (x0, y0, x1, y1)
        img.paste(lit((x1 - x0, y1 - y0), *pal[key]), box, mask.crop(box))
    # grain: fine monochrome noise, lightly overlaid
    rng = random.Random(11)
    noise = Image.frombytes("L", (w, h), rng.randbytes(w * h)).convert("RGB")
    img = Image.blend(img, ImageChops.overlay(img, noise), pal["grain"] * 4)
    return img


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--size", default="3840x2160")
    ap.add_argument("--out", default=OUT)
    a = ap.parse_args()
    w, h = (int(v) for v in a.size.lower().split("x"))
    os.makedirs(a.out, exist_ok=True)
    for mode, pal in PALETTES.items():
        path = os.path.join(a.out, "material-%s.jpg" % mode)
        render(pal, w, h).save(path, quality=90, subsampling=0, optimize=True, progressive=True)
        print("wrote", path, "%.1f MB" % (os.path.getsize(path) / 1e6))


if __name__ == "__main__":
    main()
