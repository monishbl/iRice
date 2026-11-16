#!/usr/bin/env python3
"""One gate run's numbers (tools/gate): reads <run>/events.tsv, samples.tsv
and log.tsv (gate.sh, sampler.py) and prints run.json.
  analyze.py <run dir>

The shell's timing log (on while $XDG_RUNTIME_DIR/tessera-wakes exists):
  wakes: {"why": n, …} drew {"surface": n, …} animating {…} msgs {…} ms [(…)]
      once a second (core/src/wayland.rs Wakes::note): the loop's turns by
      reason, and the frames each surface drew
  debug: stall 42.1 ms: msg host > host Panels
      a piece of a loop turn longer than 16 ms (core/src/perf.rs); nested
      pieces each log, innermost first, so one stall is a chain of lines whose
      paths are prefixes of the first: counted once, as its longest (outer) time
  debug: frame callback lost on <surface>: drawing on        (wayland.rs)
  first-frame <surface> at <ms> drew <ms> ms                 (a show's first frame)
  tessera-ui: first-frame wall=… since-main-ms=<ms>          (start-up)
  … panicked at …                                            (a panic)
"""
import json
import os
import re
import statistics
import sys

run = sys.argv[1]
HZ = os.sysconf("SC_CLK_TCK")


def events():
    ev = []
    for line in open(os.path.join(run, "events.tsv")):
        p = line.rstrip("\n").split("\t")
        if len(p) >= 2 and p[0]:
            ev.append((float(p[0]), p[1], p[2] if len(p) > 2 else ""))
    return sorted(ev)


LOADS = []  # (t, the machine's 1-minute load average)


def samples():
    out = []
    with open(os.path.join(run, "samples.tsv")) as f:
        next(f)
        for line in f:
            p = line.rstrip("\n").split("\t")
            t = float(p[0])
            if len(p) > 6 and p[6]:
                LOADS.append((t, float(p[6]), int(p[7]) if len(p) > 7 and p[7] else None))
            if not p[1]:
                out.append((t, None, None, None, None, None, None))
                continue
            out.append((t, int(p[1]), int(p[2]), int(p[3]), int(p[4]) if p[4] else None, int(p[5]) if p[5] else None,
                        int(p[8]) if len(p) > 8 and p[8] else None))
    return out


def loglines():
    out = []
    with open(os.path.join(run, "log.tsv"), "rb") as f:
        for raw in f:
            t, _, msg = raw.decode("utf-8", "replace").rstrip("\n").partition("\t")
            try:
                out.append((float(t), msg))
            except ValueError:
                pass
    return out


EV, SM, LG = events(), samples(), loglines()


def first(kind, detail=None):
    return next((t for t, k, d in EV if k == kind and (detail is None or d == detail)), None)


def windows(start, end):
    """[(detail, t0, t1)] for each start/end pair"""
    out, open_ = [], {}
    for t, k, d in EV:
        if k == start:
            open_[d] = t
        elif k == end and d in open_:
            out.append((d, open_.pop(d), t))
    return out


M0, M1 = first("measure-start"), first("measure-end")
if M0 is None or M1 is None:
    sys.exit("no measure-start/measure-end in events.tsv: the run didn't finish")

# ── the log, parsed ──
STALL = re.compile(r"^debug: stall ([0-9.]+) ms: (.*)$")
FCB = re.compile(r"^debug: frame callback lost on (.+?): drawing")
FIRST = re.compile(r"^first-frame (\S+) at \d+ drew ([0-9.]+) ms")
START = re.compile(r"first-frame wall=\S+ since-main-ms=([0-9.]+)")
NUM = re.compile(r'"[^"]*": (\d+)')


def wake_counts(msg):
    """(turns, frames) of one wakes line"""
    m = re.match(r"^wakes: (\{.*?\}) drew (\{.*?\}) animating ", msg)
    if not m:
        return None
    return sum(map(int, NUM.findall(m.group(1)))), sum(map(int, NUM.findall(m.group(2))))


def stall_events(lines):
    """[(t, ms, innermost path, outermost path)]: one per stall. An outer
    piece (its path a strict prefix of inner ones logged just before, within
    its own duration) closes them into one stall of the outer's time."""
    done = []
    for t, msg in lines:
        m = STALL.match(msg)
        if not m:
            continue
        # (a stdin line's JSON in the path: just "line", so paths group)
        ms, path = float(m.group(1)), re.sub(r"\bline \{.*?(?= > |$)", "line", m.group(2).strip())
        lo = t - ms / 1000.0 - 0.25
        # (open chains only: one already closed by its outermost piece isn't
        # taken again by the next stall's outer piece)
        inner = [c for c in done if c[0] >= lo and len(c[3]) > len(path) and c[3].startswith(path + " > ")]
        if inner:
            for c in inner:
                done.remove(c)
            done.append([inner[0][0], max(ms, max(c[1] for c in inner)), inner[0][2], path])
        else:
            done.append([t, ms, path, path])
    return [tuple(c) for c in sorted(done)]


def in_win(t, a, b):
    return a <= t <= b


def cpu_pct(a, b):
    """CPU % of one core of the shell between a and b (summed over pids if it restarted)"""
    by_pid = {}
    for t, pid, tk, *_ in SM:
        if pid is None or not (a - 0.6 <= t <= b + 0.6):
            continue
        by_pid.setdefault(pid, []).append((t, tk))
    tot = 0.0
    for pts in by_pid.values():
        pts = [p for p in pts if a - 0.6 <= p[0] <= b + 0.6]
        if len(pts) >= 2:
            tot += pts[-1][1] - pts[0][1]
    span = b - a
    return 100.0 * tot / HZ / span if span > 0 else 0.0


def vmrss_at(t, after=True, field=3):
    """VmRSS MB (field 6: RssAnon) of the nearest sample at/after t (or before)"""
    pts = [s for s in SM if s[1] is not None and s[field] is not None and (s[0] >= t if after else s[0] <= t)]
    if not pts:
        return None
    s = pts[0] if after else pts[-1]
    return s[field] / 1024.0


def anon_at(t, after=True):
    """RssAnon MB: the budgets' memory (RssFile, the binary's and the GPU
    driver's mapped pages, differs by tens of MB between runs of one binary)"""
    return vmrss_at(t, after, 6)


def rollup_near(t):
    pts = [s for s in SM if s[1] is not None and s[5] is not None]
    if not pts:
        return None, None
    s = min(pts, key=lambda s: abs(s[0] - t))
    return s[4] / 1024.0, s[5] / 1024.0


def window_stats(a, b):
    turns = frames = n = 0
    for t, msg in LG:
        if a < t <= b + 0.3:
            w = wake_counts(msg)
            if w:
                turns += w[0]
                frames += w[1]
                n += 1
    st = [s for s in stall_events([(t, m) for t, m in LG if in_win(t, a, b)])]
    fcb = sum(1 for t, m in LG if in_win(t, a, b) and FCB.match(m))
    span = b - a
    return {
        "seconds": round(span, 2),
        "cpu_pct": round(cpu_pct(a, b), 3),
        "wakeups_per_s": round(turns / span, 2) if span else 0,
        "frames": frames,
        "frames_per_s": round(frames / span, 2) if span else 0,
        "stalls": len(st),
        "stall_ms": round(sum(s[1] for s in st), 1),
        "fcb_lost": fcb,
    }


# ── idle ──
idle = [(d, a, b) for d, a, b in windows("idle-start", "idle-end")]
idle_ws = [window_stats(a, b) for _, a, b in idle]
med = lambda xs: statistics.median(xs) if xs else 0.0
idle_end = max((b for _, _, b in idle), default=M0)

# ── the script, by part ──
S0 = first("script-start") or idle_end
S1 = first("script-end") or M1
script = window_stats(S0, S1)
parts = {}
for name, (k0, k1) in {"hz120": ("hz120-start", "hz120-end"), "scale15": ("scale15-start", "scale15-end"),
                       "lock": ("lock-start", "lock-end")}.items():
    a, b = first(k0), first(k1)
    if a and b:
        parts[name] = window_stats(a, b)
reps = windows("rep-start", "rep-end")
rep_rows = []
for d, a, b in reps:
    row = window_stats(a, b)
    row["rep"] = int(d)
    row["rss_mb_end"] = round(vmrss_at(b) or 0.0, 2)
    row["anon_mb_end"] = round(anon_at(b) or 0.0, 2)
    rep_rows.append(row)


def slope(pts):
    """least squares MB per rep"""
    if len(pts) < 2:
        return 0.0
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    den = sum((x - mx) ** 2 for x in xs)
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / den if den else 0.0


# (rep 1 builds what the first opens make: the trend is over reps 2..N)
trend_key = "anon_mb_end" if any(r["anon_mb_end"] for r in rep_rows) else "rss_mb_end"
trend_pts = [(r["rep"], r[trend_key]) for r in rep_rows if r["rep"] >= 2]

# ── whole measured span ──
mlines = [(t, m) for t, m in LG if in_win(t, M0, M1)]
stalls = stall_events(mlines)
by_path = {}
for _, ms, inner, outer in stalls:
    key = inner if len(inner) <= 140 else inner[:137] + "..."
    e = by_path.setdefault(key, {"count": 0, "ms": 0.0, "worst": 0.0})
    e["count"] += 1
    e["ms"] = round(e["ms"] + ms, 1)
    e["worst"] = max(e["worst"], ms)
by_top = {}
for _, ms, inner, outer in stalls:
    top = outer.split(" > ")[0]
    e = by_top.setdefault(top, {"count": 0, "ms": 0.0})
    e["count"] += 1
    e["ms"] = round(e["ms"] + ms, 1)
fcb = {}
for t, m in mlines:
    x = FCB.match(m)
    if x:
        fcb[x.group(1)] = fcb.get(x.group(1), 0) + 1
panics = [m for t, m in LG if t >= M0 - 0.5 and "panicked at" in m]
errors = [m for t, m in mlines if re.search(r"render failed|can't draw|thread .* panicked", m)]
first_frames = {}
for t, m in mlines:
    x = FIRST.match(m)
    if x:
        first_frames.setdefault(x.group(1), []).append(float(x.group(2)))
startup = [float(START.search(m).group(1)) for t, m in LG if START.search(m)]

# restarts: pid changes after the measured start (the setup's reload is before it)
pids = []
for t, pid, *_ in SM:
    if t >= M0 - 1 and pid is not None and (not pids or pids[-1] != pid):
        pids.append(pid)
gone = [t for t, pid, *_ in SM if M0 <= t <= M1 and pid is None]
alive_end = any(s[1] is not None for s in SM if s[0] >= M1 - 1.0)

rss_idle = [vmrss_at(b, after=False) for _, _, b in idle]
rss_idle = [x for x in rss_idle if x is not None]
anon_idle = [x for x in (anon_at(b, after=False) for _, _, b in idle) if x is not None]
r_end, p_end = rollup_near(M1)
r_idle, p_idle = rollup_near(idle_end)
total_frames = sum(w[1] for t, m in mlines for w in [wake_counts(m)] if w)

load = [l for t, l, _ in LOADS if M0 <= t <= M1]
running = [r for t, _, r in LOADS if M0 <= t <= M1 and r is not None]
out = {
    # (the build machine while measuring: the 1-minute load average, and the
    # tasks running at each sample, this arena's own included. A busy machine
    # makes the numbers unreliable: compare.py says so instead of a verdict)
    "load1": {"mean": round(sum(load) / len(load), 2) if load else None, "max": max(load) if load else None},
    "running": {"mean": round(sum(running) / len(running), 2) if running else None,
                "p90": sorted(running)[int(0.9 * (len(running) - 1))] if running else None},
    "run": os.path.basename(os.path.abspath(run)),
    "measured_s": round(M1 - M0, 1),
    "idle": {
        "windows": idle_ws,
        "cpu_pct": [w["cpu_pct"] for w in idle_ws],
        "cpu_pct_median": round(med([w["cpu_pct"] for w in idle_ws]), 3),
        "wakeups_per_s_median": round(med([w["wakeups_per_s"] for w in idle_ws]), 2),
        "frames_per_s_median": round(med([w["frames_per_s"] for w in idle_ws]), 3),
    },
    "script": script,
    "parts": parts,
    "reps": rep_rows,
    "mem": {
        "idle_rss_mb": round(med(rss_idle), 2),
        "idle_rollup_rss_mb": round(r_idle, 2) if r_idle else None,
        "idle_pss_mb": round(p_idle, 2) if p_idle else None,
        "end_rss_mb": round(vmrss_at(M1, after=False) or 0.0, 2),
        "end_pss_mb": round(p_end, 2) if p_end else None,
        # (None: samples from before RssAnon was recorded)
        "idle_anon_mb": round(med(anon_idle), 2) if anon_idle else None,
        "end_anon_mb": round(anon_at(M1, after=False), 2) if anon_idle else None,
        "rss_by_rep_mb": [r["rss_mb_end"] for r in rep_rows],
        "anon_by_rep_mb": [r["anon_mb_end"] for r in rep_rows],
        # (the script's memory: the median at the ends of reps 2..N, settled;
        # the very end, after the lock, varies with what the lock left behind)
        "script_anon_mb": round(med([p[1] for p in trend_pts]), 2) if trend_pts else None,
        # (the leak trend, on RssAnon)
        "rss_slope_mb_per_rep": round(slope(trend_pts), 3),
    },
    "stalls": {
        "count": len(stalls),
        "total_ms": round(sum(s[1] for s in stalls), 1),
        "worst_ms": round(max((s[1] for s in stalls), default=0.0), 1),
        # (over two 60 Hz frames: a hitch anyone sees. Steadier than the
        # 16-33 ms ones, which on the build machine are mostly GPU jitter)
        "long_count": sum(1 for s in stalls if s[1] > 33.4),
        "long_total_ms": round(sum(s[1] for s in stalls if s[1] > 33.4), 1),
        "by_path": dict(sorted(by_path.items(), key=lambda kv: -kv[1]["ms"])[:25]),
        "by_top": dict(sorted(by_top.items(), key=lambda kv: -kv[1]["ms"])),
    },
    "frames": total_frames,
    "fcb_lost": {"count": sum(fcb.values()), "by_surface": fcb},
    "panics": {"count": len(panics), "lines": panics[:10]},
    "errors": {"count": len(errors), "lines": errors[:10]},
    "restarts": max(0, len(pids) - 1),
    "pids": pids,
    "shell_gone_samples": len(gone),
    "alive_at_end": alive_end,
    "startup_ms": startup[-1] if startup else None,
    "first_frame_ms": {k: round(med(v), 1) for k, v in sorted(first_frames.items())},
    "notes": [d for _, k, d in EV if k == "note"],
}
json.dump(out, sys.stdout, indent=1)
print()
