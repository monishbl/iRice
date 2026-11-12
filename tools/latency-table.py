#!/usr/bin/env python3
# latency-table.py OUT [OUT_BEFORE]: the latency suite's tables (live-scenarios.sh
# --suite latency) as one Markdown table: per panel, the median over the
# repetitions of time to first motion, animation length and the longest gap
# between frames, opening and closing, QML beside Rust (and Rust before, when
# a second folder is given: the same suite on the old build).
import csv, statistics, sys
from pathlib import Path


def load(folder, pas):
    f = Path(folder) / f"latency-{pas}.tsv"
    rows = {}
    if not f.exists():
        return rows
    for r in csv.DictReader(open(f), delimiter="\t"):
        rows.setdefault(r["panel"], []).append(r)
    return rows


def med(rows, key):
    v = [int(r[key]) for r in rows if r.get(key, "").strip().lstrip("-").isdigit()]
    return round(statistics.median(v)) if v else None


def cell(x):
    return "–" if x is None else str(x)


out = sys.argv[1]
before = sys.argv[2] if len(sys.argv) > 2 else None
q, r = load(out, "qml"), load(out, "rust")
b = load(before, "rust") if before else {}
panels = list(dict.fromkeys(list(q) + list(r)))
head = ["panel", "QML open", "Rust open"] + (["Rust before"] if b else []) + ["QML anim", "Rust anim", "QML gap", "Rust gap",
        "QML close", "Rust close", "QML close anim", "Rust close anim", "QML close gap", "Rust close gap"]
print("Times in ms, medians of the repetitions. open/close: command → first moving frame; anim: first → last moving frame; gap: the longest time between two frames while it moves.\n")
print("| " + " | ".join(head) + " |")
print("|" + "---|" * len(head))
for p in panels:
    qq, rr = q.get(p, []), r.get(p, [])
    row = [p, cell(med(qq, "open_ms")), cell(med(rr, "open_ms"))]
    if b:
        row.append(cell(med(b.get(p, []), "open_ms")))
    row += [cell(med(qq, "open_dur_ms")), cell(med(rr, "open_dur_ms")), cell(med(qq, "open_gap_ms")), cell(med(rr, "open_gap_ms")),
            cell(med(qq, "close_ms")), cell(med(rr, "close_ms")), cell(med(qq, "close_dur_ms")), cell(med(rr, "close_dur_ms")),
            cell(med(qq, "close_gap_ms")), cell(med(rr, "close_gap_ms"))]
    print("| " + " | ".join(row) + " |")
