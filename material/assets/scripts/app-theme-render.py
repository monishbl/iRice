#!/usr/bin/env python3
"""Fills Match apps' colour templates (assets/theming) from the shell's own
Material colour roles, for the Material look:

  app-theme-render.py <roles json> <extra json> <template>=<output> ...

roles: Theme.m3 as JSON ({"primary": "#rrggbb", "surfaceContainerHigh": …}).
extra: the same {"irice": {…}} values app-theme.sh gives matugen.

Why not matugen: matugen 4.2 makes its scheme with the 2021 Material spec,
and the shell's (common/MaterialColor.js) is the 2025 one, so even from the
same seed the surfaces, containers and outlines come out different and apps
wouldn't match the shell. The templates only use matugen's plain
substitutions, so they're filled here with the shell's own colours:
  {{colors.<role>.<default|light|dark>.<hex|hex_stripped|red|green|blue|rgb>}}
  {{irice.<key>}}
The scheme is already the current mode's, so light, dark and default are the
same colour. The templates write pure white for text on the accent (Glass's
accent fills carry white text); Material's primary is light in dark mode,
so white there becomes onPrimary. A placeholder this can't fill is an error
(nothing is written), never a half-filled file.
"""
import json
import os
import re
import sys

PLACEHOLDER = re.compile(r"\{\{\s*([^{}]*?)\s*\}\}")
COLOR = re.compile(r"^colors\.([a-z0-9_]+)\.(?:default|light|dark)\.(hex|hex_stripped|red|green|blue|rgb)$")
IRICE = re.compile(r"^irice\.([a-z0-9_]+)$")
# Pure white as the templates spell it: #ffffff, a bare ffffff value (foot) or 255,255,255 (KDE)
WHITE = re.compile(r"#ffffff\b|(?<![0-9a-fA-F#])ffffff\b|\b255,\s*255,\s*255\b", re.IGNORECASE)


def snake(name):
    return re.sub(r"([A-Z])", lambda m: "_" + m.group(1).lower(), name)


def parse_roles(text):
    roles = {}
    for k, v in json.loads(text).items():
        if isinstance(v, str) and re.fullmatch(r"#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?", v):
            roles[snake(k)] = v[:7].lower()
    if "primary" not in roles or "surface" not in roles:
        raise ValueError("roles: no primary or surface")
    return roles


def fill(template, roles, extra):
    on_accent = roles.get("on_primary", "#ffffff")

    def white(m):
        s = m.group(0)
        if s.startswith("#"):
            return on_accent
        if "," in s:
            return ",".join(str(int(on_accent[i:i + 2], 16)) for i in (1, 3, 5))
        return on_accent[1:]

    # White first, so a placeholder's value (an accent of #ffffff, say) is
    # never mistaken for template text
    parts = PLACEHOLDER.split(template)
    out = []
    for i, part in enumerate(parts):
        if i % 2 == 0:
            out.append(WHITE.sub(white, part))
            continue
        m = COLOR.match(part)
        if m:
            role, fmt = m.groups()
            if role not in roles:
                raise KeyError("no colour role " + role)
            h = roles[role]
            out.append({"hex": h, "hex_stripped": h[1:], "red": str(int(h[1:3], 16)), "green": str(int(h[3:5], 16)),
                        "blue": str(int(h[5:7], 16)), "rgb": "rgb(%d, %d, %d)" % (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16))}[fmt])
            continue
        m = IRICE.match(part)
        if m and m.group(1) in extra.get("irice", {}):
            out.append(str(extra["irice"][m.group(1)]))
            continue
        raise KeyError("unknown placeholder {{" + part + "}}")
    return "".join(out)


def main(argv):
    if len(argv) < 3:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    roles = parse_roles(argv[1])
    extra = json.loads(argv[2] or "{}")
    # Fill every template before writing any, so a bad one changes nothing
    done = []
    for pair in argv[3:]:
        tpl, _, dest = pair.partition("=")
        with open(tpl, encoding="utf-8") as f:
            done.append((dest, fill(f.read(), roles, extra)))
    for dest, text in done:
        os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
        tmp = dest + ".irice-tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
        os.replace(tmp, dest)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except (ValueError, KeyError, OSError, json.JSONDecodeError) as e:
        print("app-theme-render:", e, file=sys.stderr)
        sys.exit(1)
