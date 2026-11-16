"""The shell's timing log, read the same way by analyze.py and reallog.py
(tools/gate). Two generations of the log are understood:

wakes lines, once a second (core/src/wayland.rs Wakes::note):
  old: wakes: {"why": n, …} drew {"surface": n, …} animating … msgs {…} ms […]
  new (63121b6, pacing): wakes: {"why": n} sleeps N cpu MS drew {…}
       animating N hidden-asks {…} frames {"frames":n,…,"missed":n,…} msgs {…} ms […]
  (the new loop sleeps to its next deadline: a line may cover more than a
  second, so rates are turns over the window's seconds, never lines)

stall lines (core/src/perf.rs):
  old: every piece of a loop turn longer than 16 ms logs, innermost first;
       a chain of lines is one stall, timed by its outermost piece
  new (5b1bde3, the 8 ms tripwire): only the step that holds the time logs:
       the innermost one over 8 ms, and an outer one only when its own code
       beyond its logged pieces took over 8 ms, or (gatefix) when it is the
       loop's outermost piece and over 16 ms: `… (own 3.1 ms)`; its time is
       the whole piece's, inner pieces included

One stall = one outermost piece; its time is that piece's. A line with
`(own X ms)` closes the open chains logged just before it whose times fit in
its pieces' time (ms − own); an old-format line closes every open chain
below it (old logs have no orphans: every level over 16 ms logs); a new
line without `(own …)` had nothing logged inside it, so it closes nothing.
Stalls are the chains over STALL_MS (16 ms, a 60 Hz frame), as the
budgets were calibrated; new logs' 8-16 ms steps are counted apart ("steps").
"""
import json
import re

STALL_MS = 16.0
LONG_MS = 33.4

STALL = re.compile(r"^debug: stall ([0-9.]+) ms: (.*?)(?: \(own ([0-9.]+) ms\))?$")
WAKES = re.compile(r"^wakes: (\{.*?\})(?: sleeps (-?\d+) cpu ([0-9.]+))? drew (\{.*?\}) animating ")
FRAMES = re.compile(r" frames (\{[^{}]*\}) msgs ")
NUM = re.compile(r'"[^"]*": (\d+)')
LINE_JSON = re.compile(r"\bline \{.*?(?= > |$)")


def wake(msg):
    """One wakes line: {turns, frames, sleeps, cpu_ms, presented, missed}
    (sleeps, cpu and presentation only in new logs: None in old ones), or None"""
    m = WAKES.match(msg)
    if not m:
        return None
    out = {
        "turns": sum(map(int, NUM.findall(m.group(1)))),
        "frames": sum(map(int, NUM.findall(m.group(4)))),
        "sleeps": int(m.group(2)) if m.group(2) is not None else None,
        "cpu_ms": float(m.group(3)) if m.group(3) is not None else None,
        "presented": None,
        "missed": None,
    }
    f = FRAMES.search(msg)
    if f:
        try:
            j = json.loads(f.group(1))
            out["presented"] = j.get("frames", 0)
            out["missed"] = j.get("missed", 0)
        except ValueError:
            pass
    return out


def stall_line(msg):
    """(ms, path, own ms or None) of a stall line, or None"""
    m = STALL.match(msg)
    if not m:
        return None
    # (a stdin line's JSON in the path: just "line", so paths group)
    path = LINE_JSON.sub("line", m.group(2).strip())
    return float(m.group(1)), path, float(m.group(3)) if m.group(3) is not None else None


def is_tripwire_log(msgs):
    """Whether these lines come from the tripwire (new) format: an `(own …)`
    line, or any stall line under 16 ms (the old format logs only above)"""
    for msg in msgs:
        s = stall_line(msg)
        if s and (s[2] is not None or s[0] < STALL_MS - 0.05):
            return True
    return False


def chains(lines, tripwire=None, timed=True):
    """[(t, ms, innermost path, outermost path)]: one per outermost piece
    logged, from [(t, msg)] in log order (t: seconds; with timed=False a line
    index, and pieces within 40 lines count as just before). See the
    module's doc for the rules."""
    if tripwire is None:
        tripwire = is_tripwire_log(m for _, m in lines)
    done = []  # [t, ms, inner, outer]
    for t, msg in lines:
        s = stall_line(msg)
        if not s:
            continue
        ms, path, own = s
        if own is not None:
            budget = ms - own
        elif not tripwire:
            budget = ms
        else:
            budget = 0.0
        taken, used = [], 0.0
        # (newest first: the pieces inside this one were logged just before it,
        # each within its time; stop at the first that isn't one of them)
        lo = t - ms / 1000.0 - 0.25 if timed else t - 40
        for c in reversed(done):
            if budget <= 0.0:
                break
            if c[0] < lo or not c[3].startswith(path + " > "):
                break
            if used + c[1] > budget + 0.15 + 0.05 * len(taken):
                break
            taken.append(c)
            used += c[1]
        if taken:
            for c in taken:
                done.remove(c)
            first = min(taken, key=lambda c: c[0])
            longest = max(taken, key=lambda c: c[1])
            done.append([first[0], max(ms, longest[1]), longest[2], path])
        else:
            done.append([t, ms, path, path])
    return [tuple(c) for c in done]


def stalls(ch):
    """The chains that are stalls (over 16 ms)"""
    return [c for c in ch if c[1] > STALL_MS]


def steps(ch):
    """The tripwire's 8-16 ms steps (new logs only), reported, not budgeted"""
    return [c for c in ch if c[1] <= STALL_MS]
