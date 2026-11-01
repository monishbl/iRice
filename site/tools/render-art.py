#!/usr/bin/env python3
"""Renders the site's abstract art (site/public/media/art): soft colour
fields with film grain, and frosted-glass pills and orbs over them, in four
families (violet, green, orange, aqua), one per section. Also writes the
same fields at 1920 x 1080 as wallpapers for screenshots of iRice
(site/tools/wallpapers, not published). Pillow only.

  site/tools/render-art.py
  site/tools/render-art.py --loop FRAMES_DIR   (then ffmpeg: media/brand-loop.mp4)
"""
import random
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
ART = HERE.parent / "public/media/art"
WALL = HERE / "wallpapers"
FONT = "/usr/share/fonts/inter/InterVariable.ttf"

# base, then blobs: (colour, x, y, rx, ry) in fractions of the canvas
FIELDS = {
    "violet": ("#EEF0FA", [("#5C66BD", 0.5, 0.52, 0.26, 0.34), ("#77A9E8", 0.62, 0.7, 0.2, 0.2), ("#B9A7F0", 0.22, 0.2, 0.22, 0.2), ("#8E9AF0", 0.86, 0.18, 0.18, 0.22)]),
    "green": ("#E6EFE2", [("#2E7D55", 0.82, 0.55, 0.26, 0.42), ("#F4F7EC", 0.42, 0.6, 0.34, 0.16), ("#5FB3A3", 0.08, 0.35, 0.22, 0.38), ("#A9CF8E", 0.55, 0.12, 0.3, 0.14), ("#1E5E3E", 0.95, 0.95, 0.2, 0.2)]),
    "orange": ("#F3EEE4", [("#E8962A", 0.8, 0.22, 0.26, 0.3), ("#C9C25C", 0.18, 0.42, 0.18, 0.34), ("#4B5531", 0.55, 0.86, 0.3, 0.18), ("#F6F1E8", 0.48, 0.4, 0.2, 0.26), ("#7A8A3A", 0.08, 0.9, 0.14, 0.14)]),
    "aqua": ("#DDF1F6", [("#5ED0E8", 0.12, 0.18, 0.3, 0.3), ("#B79CF0", 0.7, 0.62, 0.34, 0.4), ("#8EC5F5", 0.36, 0.7, 0.24, 0.24), ("#E6CBF6", 0.9, 0.2, 0.2, 0.22)]),
}
# The same families after dark, for the dark page
DARK = {
    "violet": ("#14152A", [("#3B44A6", 0.5, 0.52, 0.28, 0.36), ("#4E7FD0", 0.64, 0.72, 0.2, 0.2), ("#6A4FB0", 0.2, 0.2, 0.22, 0.2)]),
    "green": ("#0E1A14", [("#1E6B47", 0.78, 0.5, 0.3, 0.42), ("#3E8F80", 0.12, 0.36, 0.24, 0.36), ("#2F4F2A", 0.5, 0.1, 0.3, 0.16)]),
    "orange": ("#1A140C", [("#B8661A", 0.78, 0.24, 0.26, 0.3), ("#6E6A2A", 0.2, 0.44, 0.2, 0.34), ("#3A4224", 0.55, 0.86, 0.3, 0.18)]),
    "aqua": ("#0E1820", [("#1F8FA8", 0.14, 0.2, 0.3, 0.3), ("#6B4FB0", 0.7, 0.62, 0.34, 0.4), ("#3E6FA8", 0.36, 0.72, 0.24, 0.24)]),
}


def hexrgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def field(name, w, h, dark=False, seed=1):
    """A soft colour field: blurred blobs on a small canvas, enlarged, grained."""
    base, blobs = (DARK if dark else FIELDS)[name]
    sw, sh = max(64, w // 8), max(64, h // 8)
    img = Image.new("RGB", (sw, sh), hexrgb(base))
    for colour, x, y, rx, ry in blobs:
        layer = Image.new("RGB", (sw, sh), hexrgb(colour))
        mask = Image.new("L", (sw, sh), 0)
        ImageDraw.Draw(mask).ellipse([(x - rx) * sw, (y - ry) * sh, (x + rx) * sw, (y + ry) * sh], fill=235)
        mask = mask.filter(ImageFilter.GaussianBlur(min(sw, sh) * 0.16))
        img = Image.composite(layer, img, mask)
    img = img.filter(ImageFilter.GaussianBlur(min(sw, sh) * 0.03)).resize((w, h), Image.BICUBIC)
    return grain(img, seed, 9 if dark else 7)


def grain(img, seed, amount):
    random.seed(seed)
    noise = Image.frombytes("L", img.size, random.randbytes(img.size[0] * img.size[1]))
    noise = noise.point(lambda v: 128 + (v - 128) * amount // 64)
    return ImageChops.overlay(img, Image.merge("RGB", (noise, noise, noise)))


def rounded(size, radius):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=radius, fill=255)
    return m


def glass(bg, box, radius, tint=(255, 255, 255), tint_amount=0.14, text=None, text_size=0, dark=False):
    """Frosted glass over `bg` at box (x0, y0, x1, y1): what's behind blurred
    and lifted, a light top edge, a thin rim and a soft shadow."""
    x0, y0, x1, y1 = box
    w, h = x1 - x0, y1 - y0
    out = bg.copy()
    # shadow
    sh = Image.new("L", bg.size, 0)
    ImageDraw.Draw(sh).rounded_rectangle([x0, y0 + h * 0.06, x1, y1 + h * 0.06], radius=radius, fill=70 if not dark else 120)
    sh = sh.filter(ImageFilter.GaussianBlur(max(w, h) * 0.05))
    out = Image.composite(Image.new("RGB", bg.size, (20, 20, 30)), out, sh.point(lambda v: v // 3))
    # the pane: blurred, lifted, tinted
    pane = bg.crop(box).filter(ImageFilter.GaussianBlur(max(w, h) * 0.06))
    pane = Image.blend(pane, Image.new("RGB", pane.size, tint), tint_amount)
    # light from the top edge, fading down
    hi = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(hi)
    for i in range(h):
        d.line([(0, i), (w, i)], fill=int(max(0, 70 * (1 - i / (h * 0.55)))))
    pane = Image.composite(Image.new("RGB", pane.size, (255, 255, 255)), pane, hi)
    mask = rounded((w, h), radius)
    out.paste(pane, (x0, y0), mask)
    # rim: bright above, fainter below
    rim = Image.new("RGBA", bg.size, (0, 0, 0, 0))
    rd = ImageDraw.Draw(rim)
    rd.rounded_rectangle([x0, y0, x1 - 1, y1 - 1], radius=radius, outline=(255, 255, 255, 150), width=max(1, w // 260 + 1))
    fade = Image.new("L", bg.size, 0)
    ImageDraw.Draw(fade).rectangle([x0, y0, x1, y0 + h * 0.6], fill=255)
    fade = fade.filter(ImageFilter.GaussianBlur(h * 0.25))
    rim.putalpha(ImageChops.multiply(rim.getchannel("A"), fade.point(lambda v: 70 + v * 185 // 255)))
    out = Image.alpha_composite(out.convert("RGBA"), rim).convert("RGB")
    if text:
        f = ImageFont.truetype(FONT, text_size)
        f.set_variation_by_axes([500])
        td = ImageDraw.Draw(out)
        tw = td.textlength(text, font=f)
        td.text((x0 + (w - tw) / 2, y0 + h / 2), text, font=f, fill=(255, 255, 255), anchor="lm")
    return out


def wordmark(img, text, size, y=None, colour=(255, 255, 255)):
    f = ImageFont.truetype(FONT, size)
    f.set_variation_by_axes([600])
    d = ImageDraw.Draw(img)
    tw = d.textlength(text, font=f)
    d.text(((img.width - tw) / 2, y if y is not None else img.height / 2), text, font=f, fill=colour, anchor="lm")
    return img


def squircle_tile(img, box, colour):
    x0, y0, x1, y1 = box
    m = rounded((x1 - x0, y1 - y0), int((x1 - x0) * 0.3)).filter(ImageFilter.GaussianBlur(1))
    img.paste(Image.new("RGB", m.size, hexrgb(colour)), (x0, y0), m)
    return img


def save(img, name, quality=86):
    ART.mkdir(parents=True, exist_ok=True)
    img.save(ART / f"{name}.webp", quality=quality, method=6)


def loop(frames_dir, n=144, w=1280, h=720):
    """The brand loop's frames: the violet blobs circling slowly (one full
    turn, so it loops seamlessly) under a still glass pill."""
    import math
    base, blobs = FIELDS["violet"]
    for i in range(n):
        t = 2 * math.pi * i / n
        moved = [(c, x + 0.06 * math.cos(t + k), y + 0.05 * math.sin(t * (1 if k % 2 else -1) + k), rx, ry) for k, (c, x, y, rx, ry) in enumerate(blobs)]
        FIELDS["_loop"] = (base, moved)
        img = field("_loop", w, h, False, 40 + i % 3)
        img = glass(img, (int(w * 0.29), int(h * 0.39), int(w * 0.71), int(h * 0.61)), int(h * 0.11), text="iRice", text_size=int(h * 0.12))
        img.save(Path(frames_dir) / f"f{i:03d}.png")


def main():
    import sys
    if sys.argv[1:2] == ["--loop"]:
        return loop(sys.argv[2])
    W, H = 1600, 1000
    # Brand renders, each its own family
    for dark in (False, True):
        s = "-dark" if dark else ""
        img = field("violet", W, H, dark, 11)
        save(glass(img, (330, 390, 1270, 610), 110, text="iRice", text_size=120, dark=dark), "brand-violet" + s)
        img = field("orange", W, H, dark, 12)
        img = glass(img, (530, 230, 1070, 770), 270, dark=dark)
        save(squircle_tile(img, (735, 435, 865, 565), "#E86A2C"), "brand-orange" + s)
        img = field("green", W, H, dark, 13)
        save(glass(img, (560, 420, 1040, 580), 80, tint=(60, 120, 90), tint_amount=0.22, text="Calm", text_size=76, dark=dark), "brand-green" + s)
        img = field("aqua", W, H, dark, 14)
        save(wordmark(img, "Made to disappear.", 96), "brand-aqua" + s)
        # The material card: one soft orb
        img = field("violet", 1600, 900, dark, 21)
        save(img, "material" + s)
    # Wallpapers for screenshots
    WALL.mkdir(parents=True, exist_ok=True)
    for name in FIELDS:
        field(name, 1920, 1080, False, 31).save(WALL / f"{name}.png")
        field(name, 1920, 1080, True, 32).save(WALL / f"{name}-dark.png")
    print("wrote", ART, "and", WALL)


if __name__ == "__main__":
    main()
