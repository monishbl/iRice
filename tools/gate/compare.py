#!/usr/bin/env python3
"""The gate's verdict (tools/gate): candidate vs baseline, budget by budget.
  compare.py <baseline run.json[,run.json…]> <candidate run.json[,…]> <out dir> [labels base cand]
Several runs of a side (run.sh's two rounds, the arenas swapped) are merged:
counts, times, CPU and memory averaged, panics and restarts summed, the
worst stall the worst, alive only if alive in every run.
Writes <out>/report.json and <out>/report.md; exit 0 = pass, 1 = fail,
3 = unreliable (the machine's load above GATE_MAX_LOAD, 8, during a run).
Budgets and tolerances: thresholds.json beside this file (docs/gate.md says
how they were calibrated). A budget with "gate": false is reported, never fails.
"""
import json
import os
import sys

base_p, cand_p, out = sys.argv[1:4]
labels = sys.argv[4:6] if len(sys.argv) >= 6 else ["baseline", "candidate"]
here = os.path.dirname(os.path.abspath(__file__))
T = json.load(open(os.path.join(here, "thresholds.json")))


def merge(paths):
    """One side's runs as one: the first run's details, the gated numbers
    averaged (summed for panics and restarts)"""
    runs = [json.load(open(p)) for p in paths.split(",") if p]
    if len(runs) == 1:
        return runs[0]
    m = json.loads(json.dumps(runs[0]))
    avg = lambda f: round(sum(f(r) for r in runs) / len(runs), 3)

    def avg_opt(f):
        v = [f(r) for r in runs]
        return None if any(x is None for x in v) else round(sum(v) / len(v), 3)

    m["runs"] = len(runs)
    m["idle"]["cpu_pct_median"] = avg(lambda r: r["idle"]["cpu_pct_median"])
    m["idle"]["cpu_pct"] = [x for r in runs for x in r["idle"]["cpu_pct"]]
    m["idle"]["wakeups_per_s_median"] = avg(lambda r: r["idle"]["wakeups_per_s_median"])
    m["idle"]["frames_per_s_median"] = avg(lambda r: r["idle"]["frames_per_s_median"])
    m["script"]["cpu_pct"] = avg(lambda r: r["script"]["cpu_pct"])
    for k in ("count", "total_ms", "long_count", "long_total_ms"):
        if all(k in r["stalls"] for r in runs):
            m["stalls"][k] = avg(lambda r: r["stalls"][k])
    m["stalls"]["worst_ms"] = max(r["stalls"]["worst_ms"] for r in runs)
    m["frames"] = avg(lambda r: r["frames"])
    m["fcb_lost"]["count"] = avg(lambda r: r["fcb_lost"]["count"])
    m["panics"]["count"] = sum(r["panics"]["count"] for r in runs)
    m["panics"]["lines"] = [l for r in runs for l in r["panics"]["lines"]][:10]
    m["restarts"] = sum(r["restarts"] for r in runs)
    m["alive_at_end"] = all(r["alive_at_end"] for r in runs)
    for k in ("idle_rss_mb", "end_rss_mb", "end_pss_mb", "idle_anon_mb", "end_anon_mb", "script_anon_mb", "rss_slope_mb_per_rep"):
        m["mem"][k] = avg_opt(lambda r: r["mem"].get(k))
    for k in ("running", "load1"):
        # (the busier run counts)
        vals = [r.get(k) or {} for r in runs]
        m[k] = max(vals, key=lambda v: (v.get("mean") or 0))
    m["startup_ms"] = avg_opt(lambda r: r.get("startup_ms"))
    return m


B, C = merge(base_p), merge(cand_p)
rows = []


def check(name, b, c, unit, tol_key=None, lower_ok=True, note=""):
    t = T.get(tol_key or name, {})
    allow = max(t.get("abs", 0.0), t.get("rel", 0.0) * abs(b))
    ok = c <= b + allow
    gate = t.get("gate", True)
    rows.append({"budget": name, "baseline": b, "candidate": c, "delta": round(c - b, 3), "allowed": round(allow, 3),
                 "unit": unit, "pass": ok, "gates": gate, "note": note})


check("idle_cpu_pct", B["idle"]["cpu_pct_median"], C["idle"]["cpu_pct_median"], "% of a core",
      note="median of %d idle windows" % len(C["idle"]["windows"]))
# (memory: RssAnon, the heap and GPU buffers the shell made. VmRSS adds
# RssFile, which differs by tens of MB between two runs of one binary: shown, not budgeted)
mem = lambda R, k: R["mem"][k] if R["mem"].get(k.replace("rss", "anon")) is None else R["mem"][k.replace("rss", "anon")]
check("idle_rss_mb", mem(B, "idle_rss_mb"), mem(C, "idle_rss_mb"), "MB", "idle_rss_mb", note="RssAnon; 10 MB + 5 MB of measured noise")
if B["mem"].get("script_anon_mb") is not None and C["mem"].get("script_anon_mb") is not None:
    check("script_rss_mb", B["mem"]["script_anon_mb"], C["mem"]["script_anon_mb"], "MB", "rss_mb", note="RssAnon, median at the ends of reps 2..N")
check("end_rss_mb", mem(B, "end_rss_mb"), mem(C, "end_rss_mb"), "MB", "end_rss_mb", note="RssAnon, after the lock: noisy, not gating")
check("stall_count", B["stalls"]["count"], C["stalls"]["count"], "stalls")
check("stall_total_ms", B["stalls"]["total_ms"], C["stalls"]["total_ms"], "ms")
if "long_count" in B["stalls"] and "long_count" in C["stalls"]:
    check("long_stall_count", B["stalls"]["long_count"], C["stalls"]["long_count"], "stalls", note="over 33 ms")
    check("long_stall_total_ms", B["stalls"]["long_total_ms"], C["stalls"]["long_total_ms"], "ms", note="over 33 ms")
check("fcb_lost", B["fcb_lost"]["count"], C["fcb_lost"]["count"], "losses")
check("script_cpu_pct", B["script"]["cpu_pct"], C["script"]["cpu_pct"], "% of a core")
check("idle_wakeups_per_s", B["idle"]["wakeups_per_s_median"], C["idle"]["wakeups_per_s_median"], "/s")
check("worst_stall_ms", B["stalls"]["worst_ms"], C["stalls"]["worst_ms"], "ms")

# no new panics or restarts, the shell still there at the end: no tolerance
for name, b, c in [("panics", B["panics"]["count"], C["panics"]["count"]),
                   ("restarts", B["restarts"], C["restarts"])]:
    rows.append({"budget": name, "baseline": b, "candidate": c, "delta": c - b, "allowed": 0, "unit": "",
                 "pass": c <= b, "gates": True, "note": ""})
rows.append({"budget": "alive_at_end", "baseline": B["alive_at_end"], "candidate": C["alive_at_end"], "delta": "",
             "allowed": "", "unit": "", "pass": bool(C["alive_at_end"]) or not B["alive_at_end"], "gates": True, "note": ""})

# the leak check: RSS growth per repetition (reps 2..N). Over the budget
# fails, unless the baseline grows as much (then it's a warning: not this change)
ts = T["rss_slope_mb_per_rep"]
bs, cs = B["mem"]["rss_slope_mb_per_rep"], C["mem"]["rss_slope_mb_per_rep"]
over = cs > ts["max"]
pre = over and cs <= bs + ts.get("noise", 0.0)
rows.append({"budget": "rss_slope_mb_per_rep", "baseline": bs, "candidate": cs, "delta": round(cs - bs, 3), "allowed": ts["max"],
             "unit": "MB/rep", "pass": not over or pre, "gates": True,
             "note": "baseline grows too (not this change)" if pre else "RssAnon by rep %s MB" % C["mem"].get("anon_by_rep_mb", C["mem"]["rss_by_rep_mb"])})

failed = [r for r in rows if r["gates"] and not r["pass"]]
warned = [r for r in rows if not r["gates"] and not r["pass"]]
verdict = "PASS" if not failed else "FAIL"
# A busy build machine makes the numbers unreliable: no verdict, retry later.
# Busy = on average more than GATE_MAX_LOAD (8) tasks running while either
# side measured (paused builds don't run; this arena's own count). The
# 1-minute load average lags minutes behind a paused build: used only
# when the running count is missing.
max_load = float(os.environ.get("GATE_MAX_LOAD", "8"))


def busy_of(R):
    r = (R.get("running") or {}).get("mean")
    return r if r is not None else (R.get("load1") or {}).get("max")


loads = {side: busy_of(R) for side, R in (("baseline", B), ("candidate", C))}
busy = {lab: l for lab, l in loads.items() if l is None or l > max_load}
if busy:
    verdict = "UNRELIABLE"
report = {"verdict": verdict, "busy": loads, "max_load": max_load, "baseline": labels[0], "candidate": labels[1], "budgets": rows,
          "failed": [r["budget"] for r in failed], "warnings": [r["budget"] for r in warned],
          "baseline_run": B, "candidate_run": C}
os.makedirs(out, exist_ok=True)
json.dump(report, open(os.path.join(out, "report.json"), "w"), indent=1)


def fmt(v):
    if isinstance(v, float):
        return f"{v:.2f}"
    return str(v)


md = [f"# Gate: {verdict}", "", f"baseline `{labels[0]}` vs candidate `{labels[1]}`", "",
      f"Machine busy (tasks running on average while measuring, builds paused): {', '.join(f'{k} {v}' for k, v in loads.items())}"
      + (f" — above {max_load:g}: the numbers below are not trusted; run again when the machine is quiet" if busy else ""), "",
      "| budget | baseline | candidate | delta | allowed | result |", "|---|---:|---:|---:|---:|---|"]
for r in rows:
    res = "pass" if r["pass"] else ("FAIL" if r["gates"] else "warn")
    md.append(f"| {r['budget']} | {fmt(r['baseline'])} | {fmt(r['candidate'])} | {fmt(r['delta'])} | {fmt(r['allowed'])} {r['unit']} | {res}{' (' + r['note'] + ')' if r['note'] else ''} |")
md += ["", "## Details", ""]
for lab, R in ((labels[0], B), (labels[1], C)):
    md.append(f"**{lab}**: idle CPU {R['idle']['cpu_pct']} %, wakeups/s {R['idle']['wakeups_per_s_median']}, "
              f"idle frames/s {R['idle']['frames_per_s_median']}; script CPU {R['script']['cpu_pct']} %, "
              f"frames {R['frames']}; stalls {R['stalls']['count']} ({R['stalls']['total_ms']} ms, worst {R['stalls']['worst_ms']} ms); "
              f"RssAnon idle {R['mem'].get('idle_anon_mb')} MB, end {R['mem'].get('end_anon_mb')} MB; VmRSS idle {R['mem']['idle_rss_mb']} MB, end {R['mem']['end_rss_mb']} MB, PSS end {R['mem']['end_pss_mb']} MB; "
              f"start-up {R['startup_ms']} ms")
    md.append("")
    top = list(R["stalls"]["by_path"].items())[:6]
    if top:
        md.append("| stall path | n | ms | worst |")
        md.append("|---|---:|---:|---:|")
        for k, v in top:
            md.append(f"| `{k}` | {v['count']} | {v['ms']} | {v['worst']} |")
        md.append("")
for lab, R in ((labels[0], B), (labels[1], C)):
    if R["panics"]["lines"]:
        md.append(f"Panics ({lab}):")
        md += ["    " + l[:200] for l in R["panics"]["lines"]]
open(os.path.join(out, "report.md"), "w").write("\n".join(md) + "\n")
print(verdict + (": " + ", ".join(report["failed"]) if failed else "") + (f" (load {busy})" if busy else ""))
sys.exit(3 if busy else 0 if not failed else 1)
