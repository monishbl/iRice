#!/usr/bin/env python3
"""Draws the device pictures in shell/assets/devices (the island's
Bluetooth cards: Devices.pictureFor): matte, softly lit, neutral, on a
128 x 128 canvas they mostly fill, readable at 52 px on the black island and
on light surfaces. No outlines, no text (fonts differ between machines).

  tools/device-pictures.py     rewrites the .svg files
"""
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "shell/assets/devices"

DEFS = """<defs>
  <linearGradient id="shell" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FCFCFD"/><stop offset="0.6" stop-color="#E7E7EB"/><stop offset="1" stop-color="#CACAD1"/></linearGradient>
  <linearGradient id="shellH" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#C9C9D0"/><stop offset="0.3" stop-color="#F6F6F8"/><stop offset="0.7" stop-color="#ECECEF"/><stop offset="1" stop-color="#BEBEC6"/></linearGradient>
  <linearGradient id="lid" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#D9D9DF"/><stop offset="1" stop-color="#F3F3F5"/></linearGradient>
  <linearGradient id="graphite" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#46464D"/><stop offset="1" stop-color="#1E1E22"/></linearGradient>
  <linearGradient id="fabric" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#E2E2E6"/><stop offset="1" stop-color="#B9B9C1"/></linearGradient>
  <linearGradient id="screen" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3A4260"/><stop offset="0.55" stop-color="#1D2030"/><stop offset="1" stop-color="#121318"/></linearGradient>
  <linearGradient id="sheen" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#FFFFFF" stop-opacity="0.16"/><stop offset="0.45" stop-color="#FFFFFF" stop-opacity="0"/></linearGradient>
  <linearGradient id="top" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFFFFF" stop-opacity="0.95"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/></linearGradient>
  <linearGradient id="well" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8E8E97"/><stop offset="1" stop-color="#B9B9C1"/></linearGradient>
  <radialGradient id="cavity" cx="0.5" cy="0.35" r="0.65"><stop offset="0" stop-color="#2E2E34"/><stop offset="1" stop-color="#18181B"/></radialGradient>
  <radialGradient id="shadow" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#000000" stop-opacity="0.38"/><stop offset="1" stop-color="#000000" stop-opacity="0"/></radialGradient>
</defs>"""


def svg(name, body):
    (OUT / f"{name}.svg").write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="128" height="128">\n'
        + DEFS + "\n" + body.strip() + "\n</svg>\n")


def shadow(cx, cy, rx, ry=4.5):
    return f'<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="url(#shadow)"/>'


def dots(x0, x1, y0, y1, step, r, inside=lambda x, y: True, fill="#000000", opacity=0.13):
    out = []
    y, row = y0, 0
    while y <= y1:
        x = x0 + (step / 2 if row % 2 else 0)
        while x <= x1:
            if inside(x, y):
                out.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r}"/>')
            x += step
        y += step * 0.87
        row += 1
    return f'<g fill="{fill}" opacity="{opacity}">' + "".join(out) + "</g>"


def earbuds():
    # An open case, the lid tipped back, the buds resting in it, ear tips
    # down (only their backs show)
    bud = """<path d="M-13 3 C-13 -8 -5 -13 3 -12 C11 -11 14 -3 12 5 C10 11 3 14 -4 13 C-10 12 -13 8 -13 3 Z" fill="url(#shell)"/>
      <path d="M-8 -5 C-6 -9 -1 -10 3 -9.5" stroke="#FFFFFF" stroke-width="2.6" stroke-linecap="round" fill="none"/>
      <ellipse cx="3" cy="6" rx="5" ry="2.4" fill="#000000" opacity="0.08"/>"""
    svg("earbuds", f"""
{shadow(64, 114, 46)}
<path d="M22 60 C22 32 34 18 64 18 C94 18 106 32 106 60 Z" fill="url(#lid)"/>
<path d="M29 58 C30 38 40 27 64 27 C88 27 98 38 99 58 Z" fill="#000000" opacity="0.06"/>
<rect x="16" y="56" width="96" height="56" rx="28" fill="url(#shell)"/>
<ellipse cx="64" cy="61" rx="41" ry="9.5" fill="url(#well)"/>
<g transform="translate(45 57) rotate(-8)">{bud}</g>
<g transform="translate(83 57) scale(-1 1) rotate(-8)">{bud}</g>
<rect x="28" y="76" width="72" height="12" rx="6" fill="url(#top)" opacity="0.6"/>
<circle cx="64" cy="100" r="1.6" fill="#9A9AA3"/>
""")


def headphones():
    svg("headphones", f"""
{shadow(64, 116, 48)}
<path d="M27 72 C25 26 103 26 101 72" fill="none" stroke="url(#shellH)" stroke-width="8" stroke-linecap="round"/>
<path d="M34 58 C36 34 92 34 94 58" fill="none" stroke="url(#graphite)" stroke-width="7" stroke-linecap="round"/>
<rect x="24.5" y="60" width="5" height="16" rx="2.5" fill="#B3B3BB"/>
<rect x="98.5" y="60" width="5" height="16" rx="2.5" fill="#B3B3BB"/>
<rect x="10" y="66" width="34" height="50" rx="17" fill="url(#shell)"/>
<rect x="34" y="70" width="14" height="42" rx="7" fill="url(#graphite)"/>
<rect x="84" y="66" width="34" height="50" rx="17" fill="url(#shell)"/>
<rect x="80" y="70" width="14" height="42" rx="7" fill="url(#graphite)"/>
<rect x="15" y="70" width="8" height="26" rx="4" fill="url(#top)"/>
<rect x="89" y="70" width="8" height="26" rx="4" fill="url(#top)"/>
""")


def speaker():
    inside = lambda x, y: (x - 64) ** 2 / 44 ** 2 + (y - 74) ** 2 / 24 ** 2 < 1
    svg("speaker", f"""
{shadow(64, 108, 54)}
<rect x="8" y="46" width="112" height="56" rx="28" fill="url(#fabric)"/>
{dots(14, 114, 50, 98, 3.6, 0.75, inside)}
<path d="M8 74 C8 58 20 46 36 46 V102 C20 102 8 90 8 74 Z" fill="url(#graphite)"/>
<path d="M120 74 C120 58 108 46 92 46 V102 C108 102 120 90 120 74 Z" fill="url(#graphite)"/>
<rect x="40" y="49" width="48" height="8" rx="4" fill="url(#top)" opacity="0.7"/>
<rect x="50" y="40" width="28" height="6" rx="3" fill="url(#graphite)"/>
""")


def soundbar():
    svg("soundbar", f"""
{shadow(64, 88, 60, 3.5)}
<rect x="4" y="56" width="120" height="28" rx="14" fill="url(#fabric)"/>
{dots(10, 118, 60, 80, 3.2, 0.7, lambda x, y: 18 < x < 110 or 62 < y < 78)}
<rect x="20" y="58" width="88" height="5" rx="2.5" fill="url(#top)" opacity="0.7"/>
<rect x="56" y="81" width="16" height="2" rx="1" fill="#8E8E96"/>
""")


def phone():
    svg("phone", f"""
{shadow(64, 120, 30, 3.5)}
<rect x="36" y="6" width="56" height="112" rx="14" fill="url(#shellH)"/>
<rect x="38.5" y="8.5" width="51" height="107" rx="11.5" fill="#0C0C0E"/>
<rect x="40.5" y="10.5" width="47" height="103" rx="9.5" fill="url(#screen)"/>
<rect x="40.5" y="10.5" width="47" height="103" rx="9.5" fill="url(#sheen)"/>
<rect x="56" y="15" width="16" height="5" rx="2.5" fill="#000000"/>
<rect x="34.5" y="32" width="2" height="12" rx="1" fill="#A9A9B1"/>
<rect x="91.5" y="38" width="2" height="18" rx="1" fill="#A9A9B1"/>
""")


def watch():
    svg("watch", f"""
{shadow(64, 122, 24, 3)}
<path d="M46 30 L50 6 H78 L82 30 Z" fill="url(#graphite)"/>
<path d="M46 98 L50 122 H78 L82 98 Z" fill="url(#graphite)"/>
<rect x="32" y="24" width="64" height="80" rx="22" fill="url(#shellH)"/>
<rect x="35.5" y="27.5" width="57" height="73" rx="18.5" fill="#0C0C0E"/>
<rect x="38" y="30" width="52" height="68" rx="16" fill="url(#screen)"/>
<circle cx="64" cy="64" r="20" fill="none" stroke="#FFFFFF" stroke-opacity="0.12" stroke-width="4"/>
<path d="M64 44 A20 20 0 1 1 46.7 74" fill="none" stroke="#FFFFFF" stroke-opacity="0.85" stroke-width="4" stroke-linecap="round"/>
<path d="M64 64 L64 52" stroke="#FFFFFF" stroke-width="2.6" stroke-linecap="round"/>
<path d="M64 64 L73 69" stroke="#FFFFFF" stroke-width="2.6" stroke-linecap="round"/>
<circle cx="64" cy="64" r="2.2" fill="#FFFFFF"/>
<rect x="38" y="30" width="52" height="68" rx="16" fill="url(#sheen)"/>
<rect x="95" y="50" width="4" height="14" rx="2" fill="#B3B3BB"/>
""")


def controller():
    svg("controller", f"""
{shadow(64, 112, 52)}
<path d="M36 40 H92 C108 40 116 52 119 70 L123 96 C125 108 112 114 103 106 L90 94 H38 L25 106 C16 114 3 108 5 96 L9 70 C12 52 20 40 36 40 Z" fill="url(#shell)"/>
<path d="M36 44 H92 C102 44 108 49 111 56 H17 C20 49 26 44 36 44 Z" fill="url(#top)" opacity="0.7"/>
<path d="M28 64 h6 v-6 h6 v6 h6 v6 h-6 v6 h-6 v-6 h-6 Z" fill="url(#graphite)"/>
<circle cx="92" cy="58" r="4" fill="url(#graphite)"/>
<circle cx="92" cy="76" r="4" fill="url(#graphite)"/>
<circle cx="83" cy="67" r="4" fill="url(#graphite)"/>
<circle cx="101" cy="67" r="4" fill="url(#graphite)"/>
<circle cx="50" cy="86" r="9" fill="url(#graphite)"/>
<circle cx="78" cy="86" r="9" fill="url(#graphite)"/>
<circle cx="50" cy="85" r="5" fill="#55555D"/>
<circle cx="78" cy="85" r="5" fill="#55555D"/>
""")


def keyboard():
    keys = []
    for y, n, x0, w in [(50, 13, 12, 6.6), (59, 12, 15, 7.1), (68, 11, 18, 7.6), (77, 10, 21, 8.1)]:
        gap = (128 - 2 * x0 - n * w) / (n - 1)
        for i in range(n):
            keys.append((x0 + i * (w + gap), y, w))
    keys += [(37, 86, 54), (12, 86, 20), (96, 86, 20)]
    shade = "".join(f'<rect x="{x:.1f}" y="{y + 0.9}" width="{w}" height="6.6" rx="1.8"/>' for x, y, w in keys)
    face = "".join(f'<rect x="{x:.1f}" y="{y}" width="{w}" height="6.6" rx="1.8"/>' for x, y, w in keys)
    svg("keyboard", f"""
{shadow(64, 102, 60, 3.5)}
<rect x="6" y="44" width="116" height="54" rx="9" fill="url(#shell)"/>
<g fill="#000000" opacity="0.14">{shade}</g>
<g fill="#FDFDFD">{face}</g>
""")


def mouse():
    svg("mouse", f"""
{shadow(64, 114, 28, 4)}
<path d="M64 14 C84 14 90 30 90 56 V82 C90 102 80 112 64 112 C48 112 38 102 38 82 V56 C38 30 44 14 64 14 Z" fill="url(#shellH)"/>
<path d="M64 15 V50" stroke="#000000" stroke-opacity="0.12" stroke-width="1.2"/>
<rect x="61" y="28" width="6" height="13" rx="3" fill="url(#graphite)"/>
<path d="M46 34 C48 24 54 19 60 18" stroke="#FFFFFF" stroke-width="4" stroke-linecap="round" fill="none" opacity="0.9"/>
""")


def laptop():
    svg("laptop", f"""
{shadow(64, 104, 62, 3.5)}
<rect x="18" y="18" width="92" height="66" rx="6" fill="url(#shellH)"/>
<rect x="20.5" y="20.5" width="87" height="61" rx="4" fill="#0C0C0E"/>
<rect x="23" y="23" width="82" height="56" rx="2.5" fill="url(#screen)"/>
<rect x="23" y="23" width="82" height="56" rx="2.5" fill="url(#sheen)"/>
<rect x="58" y="20.5" width="12" height="3.5" rx="1.75" fill="#000000"/>
<path d="M6 84 H122 L119 94 C118.5 96 117 97 115 97 H13 C11 97 9.5 96 9 94 Z" fill="url(#shell)"/>
<rect x="52" y="84" width="24" height="3" rx="1.5" fill="#000000" opacity="0.15"/>
""")


def tag():
    # Anything else: a round Bluetooth tag
    svg("bluetooth", f"""
{shadow(64, 108, 38, 4.5)}
<circle cx="64" cy="62" r="42" fill="url(#shell)"/>
<circle cx="64" cy="62" r="34" fill="#000000" opacity="0.05"/>
<path d="M56 52 L72 68 L64 76 V48 L72 56 L56 72" fill="none" stroke="#8E8E96" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round"/>
<path d="M34 46 C40 32 52 25 64 24" stroke="#FFFFFF" stroke-width="5" stroke-linecap="round" fill="none" opacity="0.9"/>
""")


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for draw in (earbuds, headphones, speaker, soundbar, phone, watch, controller, keyboard, mouse, laptop, tag):
        draw()
    print("wrote", OUT)
