#!/usr/bin/env python3
# speed-table.py DIR [TAG…]: tools/speed-suite.sh's results (DIR/<tag>-<look>.tsv)
# as one Markdown table per look, medians over the repetitions, in ms:
#   first: trigger → the first compositor frame after it (open: the panel's
#          first frame, or the first change the trigger caused)
#   map:   trigger → the panel's layer mapped (open) / unmapped (close)
#   anim:  first → last frame of the motion
#   late/gap: frames more than 20 ms apart / the longest gap (stutter)
# TAG defaults to "qml rust"; e.g. `speed-table.py DIR qml rust rust2` adds a build.
import csv, statistics, sys
from pathlib import Path

d = Path(sys.argv[1])
tags = sys.argv[2:] or ["qml", "rust"]


def load(p):
    rows = {}
    if p.exists():
        for r in csv.DictReader(open(p), delimiter="\t"):
            rows.setdefault((r["panel"], r["phase"]), []).append(r)
    return rows


def med(rs, f):
    v = []
    for r in rs:
        try:
            v.append(f(r))
        except (ValueError, TypeError, KeyError):
            pass
    return round(statistics.median(v)) if v else None


fields = {
    "first": lambda r: int(r["first"]),
    "map": lambda r: int(r["map"] if r["phase"] == "open" else r["unmap"]),
    "anim": lambda r: int(r["last"]) - int(r["first"]),
    "late": lambda r: int(r["late"]),
    "gap": lambda r: int(r["gap"]),
}
for look in ("glass", "material"):
    data = {t: load(d / f"{t}-{look}.tsv") for t in tags}
    if not any(data.values()):
        continue
    print(f"\n### {look}\n")
    cols = [(ph, f) for ph in ("open", "close") for f in ("first", "map", "anim", "late", "gap")]
    print("| panel | " + " | ".join(f"{ph} {f}" for ph, f in cols) + " |")
    print("|---|" + "---|" * len(cols))
    panels = list(dict.fromkeys(p for t in tags for (p, _) in data[t]))
    for p in panels:
        cells = []
        for ph, f in cols:
            vals = [med(data[t].get((p, ph), []), fields[f]) for t in tags]
            cells.append(" / ".join("–" if v is None else str(v) for v in vals))
        print(f"| {p} | " + " | ".join(cells) + " |")
    print(f"\n(each cell: {' / '.join(tags)})")
