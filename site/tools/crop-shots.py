#!/usr/bin/env python3
"""Turns 2x sandbox screenshots (3840 x 2160, taken over the backgrounds in
site/tools/wallpapers) into the site's images (site/public/media/shots):
cropped, private details blurred, WebP.

  site/tools/crop-shots.py SHOTS_DIR
"""
import sys
from pathlib import Path

from PIL import Image, ImageFilter

OUT = Path(__file__).resolve().parent.parent / "public/media/shots"
# (shots are taken with public placeholder names, eduroam for the Wi-Fi:
# nothing personal to hide; blur() is there for one that slips through)
SSID = (3240, 173, 3440, 214)
CC_OPEN = set()
# name: (crop box, width it's saved at[, the shot it's cut from])
CROPS = {
    "hero": ((0, 0, 3840, 2160), 2400),
    "cc-leaves": ((2640, 0, 3840, 1500), 1200),
    "island-connect": ((1320, 0, 2520, 600), 1200),
    "island-connect-tall": ((1490, 0, 2350, 1075), 860, "island-connect"),
    "island-parts": ((1320, 0, 2520, 600), 1200),
    "island-volume": ((1320, 0, 2520, 600), 1200),
    "quickshare-ask": ((1320, 620, 2520, 1520), 1200),
    "quickshare-progress": ((1320, 620, 2520, 1520), 1200),
    "nc-petals": ((2840, 0, 3840, 960), 1000),
    "spotlight-flower": ((1100, 300, 2740, 1500), 1640),
    "spotlight-time": ((1100, 300, 2740, 1500), 1640),
    "spotlight-days": ((1100, 300, 2740, 1500), 1640),
    "settings-update": ((900, 380, 2940, 1820), 2040),
    "appstore": ((0, 0, 3840, 2160), 2400, "g-appstore"),
    # Tessera's (shots-rust.sh: the Rust shell; the same moment in each look,
    # over tiles; the page splits the Quick Settings pair, so both halves get
    # the same crop: the right half, where Quick Settings is)
    "looks-g": ((1920, 0, 3840, 1080), 1920, "qs-g"), "looks-m": ((1920, 0, 3840, 1080), 1920, "qs-m"),
    "island-g": ((1440, 0, 2400, 480), 960),
    "share-g": ((1320, 740, 2520, 1340), 1200),
    # the battery's live entry ("3 updates") in Notification Center
    "nc-live": ((2800, 0, 3840, 960), 1040, "nc-g"),
    "overview": ((0, 120, 3840, 2040), 2400, "overview-g"),
    "settings-health": ((1060, 410, 2780, 1650), 1290, "set-health"),
    "settings-power": ((1560, 1010, 2780, 1650), 1220, "set-power"),
    "settings-face": ((1540, 730, 2700, 1115), 1160, "set-security"),
}
# Anything private to blur before cropping: (the Rust shell's sandbox shows
# Wi-Fi off, so no network name shows; a name that slips through goes here)
BLUR = {}
# The lock screen's island, Glass | Material, side by side (the camera looking)
LOCK_PAIR = ("lock-halo", (1580, 1340, 2260, 2105), ["lock-face-g", "lock-face-m"])


def blur(img, box):
    region = img.crop(box).filter(ImageFilter.BoxBlur(40)).filter(ImageFilter.GaussianBlur(20))
    img.paste(region, box[:2])


def main(src):
    OUT.mkdir(parents=True, exist_ok=True)
    for name, (box, width, *shot) in CROPS.items():
        path = Path(src) / f"{shot[0] if shot else name}.png"
        if not path.exists():
            print("missing", path)
            continue
        img = Image.open(path).convert("RGB")
        if name in CC_OPEN:
            blur(img, SSID)
        for b in BLUR.get(name, []):
            blur(img, b)
        img = img.crop(box)
        if img.width != width:
            img = img.resize((width, round(img.height * width / img.width)), Image.LANCZOS)
        img.save(OUT / f"{name}.webp", quality=84, method=6)
        print(name, img.size)
    name, box, shots = LOCK_PAIR
    paths = [Path(src) / f"{s}.png" for s in shots]
    if all(p.exists() for p in paths):
        w, h = box[2] - box[0], box[3] - box[1]
        out = Image.new("RGB", (2 * w + 4, h), (24, 24, 27))
        for i, p in enumerate(paths):
            out.paste(Image.open(p).convert("RGB").crop(box), (i * (w + 4), 0))
        out.save(OUT / f"{name}.webp", quality=84, method=6)
        print(name, out.size)


if __name__ == "__main__":
    main(sys.argv[1])
