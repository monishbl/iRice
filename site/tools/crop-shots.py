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
    # Tessera's (shots.sh: the same moment in each look, over tiles; the
    # page splits the Quick Settings pair, so both halves get the same crop)
    # (the split: the right half, where Quick Settings is; its divider starts
    # over them)
    "looks-g": ((1920, 0, 3840, 1080), 1920, "qs-g"), "looks-m": ((1920, 0, 3840, 1080), 1920, "qs-m"),
    "island-g": ((1440, 0, 2400, 480), 960),
    "share-g": ((1328, 791, 2528, 1391), 1200),
}
# The Wi-Fi network's name, wherever Quick Settings is open (the sandbox
# shows this computer's real network): blurred before cropping
BLUR = {
    "looks-g": [(3240, 170, 3420, 206)], "hero": [(3240, 170, 3420, 206)],
    "looks-m": [(3156, 166, 3315, 214)],
}


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


if __name__ == "__main__":
    main(sys.argv[1])
