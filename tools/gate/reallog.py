#!/usr/bin/env python3
"""A real session's timing log, summed up (tools/gate; read-only):
  reallog.py <tessera-ui.log> [more logs…]
The same lines the gate reads (analyze.py's docstring), without timestamps:
the wakes lines (one a second while the timing log is on) give the seconds
covered, the loop's turns and the frames; stalls are grouped per chain as in
analyze.py (timinglog.py: both log formats; stalls are the outermost pieces
over 16 ms, a tripwire log's 8-16 ms steps are counted apart). A new log's
wakes line can cover more than a second (the loop sleeps to its deadline):
wake_seconds counts lines, so it is a lower bound there.
Prints JSON, for docs/gate.md's real-session comparison.
"""
import json
import os
import re
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import timinglog as TL  # noqa: E402

FCB = re.compile(r"^debug: frame callback lost on (.+?): drawing")
FIRST = re.compile(r"^first-frame (\S+) at \d+ drew ([0-9.]+) ms")
START = re.compile(r"first-frame wall=\S+ since-main-ms=([0-9.]+)")


def summarize(path):
    lines = open(path, "rb").read().decode("utf-8", "replace").splitlines()
    secs = turns = frames = quiet = 0
    fps = []
    stall_lines = []  # (index, line)
    fcb, first, starts, panics = {}, {}, [], []
    for i, ln in enumerate(lines):
        w = TL.wake(ln)
        if w:
            secs += 1
            t, f = w["turns"], w["frames"]
            turns += t
            frames += f
            if f == 0:
                quiet += 1
            else:
                fps.append(f)
            continue
        if TL.stall_line(ln):
            stall_lines.append((i, ln))
            continue
        m = FCB.match(ln)
        if m:
            fcb[m.group(1)] = fcb.get(m.group(1), 0) + 1
            continue
        m = FIRST.match(ln)
        if m:
            first.setdefault(m.group(1), []).append(float(m.group(2)))
            continue
        m = START.search(ln)
        if m:
            starts.append(float(m.group(1)))
        if "panicked at" in ln:
            panics.append(ln[:200])
    all_chains = TL.chains(stall_lines, timed=False)
    chains = TL.stalls(all_chains)
    by = {}
    for _, ms, inner, outer in chains:
        e = by.setdefault(inner[:140], {"count": 0, "ms": 0.0, "worst": 0.0})
        e["count"] += 1
        e["ms"] = round(e["ms"] + ms, 1)
        e["worst"] = max(e["worst"], ms)
    big = sorted(c[1] for c in chains)
    return {
        "log": path,
        "lines": len(lines),
        "wake_seconds": secs,
        "quiet_seconds": quiet,
        "wakeups_per_s": round(turns / secs, 2) if secs else None,
        "frames": frames,
        "frames_per_busy_s_median": statistics.median(fps) if fps else None,
        "stalls": len(chains),
        "steps_8_16": len(all_chains) - len(chains),
        "stall_total_ms": round(sum(big), 1),
        "stall_ms_p50": statistics.median(big) if big else None,
        "stall_worst_ms": big[-1] if big else None,
        "stalls_over_50ms": sum(1 for x in big if x > 50),
        "stalls_per_wake_hour": round(len(chains) / secs * 3600, 1) if secs else None,
        "stall_by_path": dict(sorted(by.items(), key=lambda kv: -kv[1]["ms"])[:12]),
        "fcb_lost": fcb,
        "startup_ms": starts,
        "first_frame_ms_median": {k: round(statistics.median(v), 1) for k, v in sorted(first.items())},
        "panics": panics[:5],
    }


json.dump([summarize(p) for p in sys.argv[1:]], sys.stdout, indent=1)
print()
