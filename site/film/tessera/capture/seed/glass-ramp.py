#!/usr/bin/env python3
"""Ramps the Glass look's glass from flat to its strongest, live, for the
film's "All in glass" take: Custom material, with Glass (the lens),
Refraction and Dispersion rising together from their least to their most,
and the index of refraction from water's (1.33) to the most, eased, in small
steps (the shell reads its config file as it changes), over 25% frost and
full tint. Only ever touches the arena home given.
  glass-ramp.py <arena home> [seconds] [steps]
  glass-ramp.py <arena home> flat     (sets the start and returns)
"""
import json
import os
import sys
import time

home = sys.argv[1]
assert "/.sandbox/" in os.path.abspath(home), "not an arena home: " + home
path = os.path.join(home, ".config/irice/config.json")

# (key, from, to): the settings page's own ranges
RAMP = [("glassLens", 0.0, 1.0), ("glassIor", 1.33, 2.0), ("glassRefraction", 0.4, 2.0), ("glassDispersion", 0.0, 1.0)]


def write(p):
    c = json.load(open(path))
    a = c.setdefault("appearance", {})
    a.update(glass="custom", glassDepth="depth", glassSolid=0.0, glassFrost=0.25, glassTint=2.0)
    for k, lo, hi in RAMP:
        a[k] = round(lo + (hi - lo) * p, 4)
    # (in place, as seed.py writes it: the shell watches this file)
    with open(path, "w") as f:
        json.dump(c, f, indent=4)


if len(sys.argv) > 2 and sys.argv[2] == "flat":
    write(0)
    sys.exit()
secs = float(sys.argv[2]) if len(sys.argv) > 2 else 2.4
steps = int(sys.argv[3]) if len(sys.argv) > 3 else 36
for i in range(steps + 1):
    u = i / steps
    write(u * u * (3 - 2 * u))  # (eased: slow off the start, slow into the end)
    time.sleep(secs / steps)
