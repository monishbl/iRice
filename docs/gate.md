# The gate: smoothness and stability before Beta

`tools/gate/` decides whether a change to the Rust shell (native/tessera-ui)
may go to the Beta branch. It runs on the build machine (server1), builds the
baseline and the candidate, plays the same scripted session on each in a hidden
sandbox arena, and compares the two against fixed budgets. Exit code 0 = pass.

## How to run it

On server1, from a checkout that has `tools/gate` (e.g. `~/tb-d`):

    tools/gate/run.sh <baseline-ref> <candidate-ref>
    # e.g. tools/gate/run.sh rust-rewrite gate-candidate

- Refs are anything `~/tessera-build` knows (a sha, or a branch pushed there:
  `git push server1 mybranch`); branches are also tried as `tb/<name>`.
- Each side is built in its own copy, `~/tb-gate-base` and `~/tb-gate-cand`
  (`git clone --shared ~/tessera-build`; the target is warmed once from
  `~/tessera-build/native/tessera-ui/target/fast` with reflinks; only `fast`
  is copied, the 71 GB `debug` is no use here), with
  `cargo build --profile fast --locked` through `~/bin/tbuild` (a low-weight
  cgroup scope that `~/bin/tgovernor` watches; plain
  `nice -n 19 ionice -c 3 cargo` if tbuild is missing). Binaries are kept by commit in
  `~/tb-gate-bins/<sha>/tessera-ui`: a commit is built once, so the baseline
  usually costs nothing.
- It prints the report's path (`~/tb-gate-reports/<date>-<base>-<cand>/report.md`,
  with `report.json`, and each side's raw `events.tsv`, `samples.tsv`,
  `log.tsv`, `run.json`) and the verdict.
- Exit: 0 pass, 1 fail, 2 couldn't run (build or sandbox failed),
  3 unreliable (the machine was busy, see below).
- Knobs (env): `GATE_ROUNDS` (2), `GATE_PARALLEL` (1: both sides at once),
  `GATE_REPS` (4), `GATE_IDLE` (`20:3`: three 20 s idle windows),
  `GATE_MALLOC_ARENAS` (1; 0 = glibc's default), `GATE_MALLOC_MMAP`
  (65536; 0 = glibc's growing threshold), `GATE_MAX_LOAD` (8),
  `GATE_WAIT` (60 s), `GATE_HOLD_MAX` (600 s), `GATE_RETRIES` (1),
  `GATE_OUT`, `GATE_TB`.
- One side alone: `tools/gate/gate.sh <arena> <binary> <out>`; runs
  compared: `tools/gate/compare.py base1/run.json,base2/run.json cand1/run.json,cand2/run.json <out>`.
- A real session's timing log summed up: `tools/gate/reallog.py <tessera-ui.log>`.

Runtime: one round (both sides side by side) takes ~5.5 minutes (arena
start and settle ~1 min, idle 1 min, script ~3 min). The default two rounds
take ~5.5 + pause + 5.5 minutes: builds are held for one round at a time,
and the second waits as long as the first held (see below), so ~17 minutes
in all, plus the candidate's build (incremental `fast`: a few minutes).
`GATE_ROUNDS=1` is ~6 minutes but keeps the arena bias (see the noise
section). The ~10-minute target holds for builds alone, not for a gate that
must leave the machine to others between its holds.

### A busy machine

server1 is also used for work and for other agents' builds. While measuring
a round, run.sh creates `~/.cache/tessera-governor.hold`: `~/bin/tgovernor`
then freezes every Tessera build's cgroup within ~2 s (run.sh waits until
`~/.cache/tessera-governor.json` says `"hold": true` with no build running)
and the file is removed when the round ends (also on exit or Ctrl-C). A hold
never lasts more than `GATE_HOLD_MAX` (600 s; a watchdog removes the file),
and never follows another directly: the next hold, this run's or another
run's, waits at least as long as the last one held
(`~/.cache/tessera-gate-hold.last`). Builds started outside tbuild aren't
paused, so inside the hold it also waits (up to `GATE_WAIT`, 60 s) until
fewer than `GATE_MAX_LOAD - 2` tasks are running
(`/proc/loadavg`'s instantaneous count, averaged over 5 s: the 1-minute
average lags a paused build by minutes). Every sample records both the
1-minute load and the running count; if either side averaged more than
`GATE_MAX_LOAD` (8) running tasks while measuring (its own arena included),
the verdict is **UNRELIABLE** (exit 3), the run is kept as `<out>-busy-N` and
measured again (`GATE_RETRIES`). Never read an unreliable run as a pass or a
fail. `gate.sh` alone doesn't hold the builds.

## What it measures, and how

Both sides use this checkout's sandbox (dev.sh with `TESSERA_NO_BOX=1`, its
Hyprland config, glass plugin, virtual pointer, the script): only the
tessera-ui binary differs. Round 1: baseline in arena 13, candidate in 14,
side by side; round 2: swapped. Each side's two runs are averaged.
Both sides run with `MALLOC_ARENA_MAX=1` and `MALLOC_MMAP_THRESHOLD_=65536`
(see the noise section).

**Same start every time** (`gate.sh`): the arena's home is deleted and made
again; `config.py` writes the gate's settings (Rust standalone, welcome done,
Glass dark liquid, the repo's `wall-tiles.jpg`, no weather/stocks/morning
greeting/update checks, desktop clock widget only, island events limited to
timers and no temperature alert, no idle lock), the binary is installed in
the arena home, `lib.sh setup glass … 1` seeds the look and puts the test
screen at 1920x1080@60, scale 1; the shell's timing log is switched on with
`$XDG_RUNTIME_DIR/tessera-wakes`. Then it waits until the shell has drawn
nothing for 3 s.

**Recording** (`sampler.py`, on the host): the shell's process (found by comm
`tessera-ui`, `--standalone`, and the arena's `XDG_RUNTIME_DIR` in its
environ; never by a pattern kill) every 0.5 s: CPU ticks from
`/proc/<pid>/stat`, VmRSS, RssAnon and VmSwap; every 3 s Rss/Pss from `/proc/<pid>/smaps_rollup`;
the machine's load. It follows `tessera-ui.log` across launch-shell.sh's
rotation and stamps every line with the time it was read. A pid change is a
restart.

**The script** (`scenario.sh`, run inside the arena so steps are local calls;
each step stamps `events.tsv`):

1. Idle: three 20 s windows, nothing touched.
2. `GATE_REPS` (4) repetitions of: Control Center, Notification Center,
   Launchpad, Wi-Fi menu, Mission Control (each toggled, held 1.1 s, closed);
   Spotlight with typed queries (`wtype`); Settings opened and switched
   through five pages, closed; the dock hovered and magnified (virtual
   pointer glides along it); two terminals (alacritty at 90x28, else kitty or
   foot) opened, one minimized (`special:minimized`) and restored, workspaces
   2, 3, 1, both closed by address.
3. 120 Hz pass: the output at 1920x1080@120; Control Center, Launchpad,
   Mission Control, dock hover, a terminal opened and closed.
4. Scale 1.5 pass: the same at 1920x1080@60 scale 1.5.
5. Lock: `ipc lock lock`, the sandbox PAM password `mock` typed, Return.

**Numbers** (`analyze.py` → `run.json`), from the shell's timing log
(core/src/wayland.rs, perf.rs):

Both generations of the timing log are read (`timinglog.py`, shared with
`reallog.py`), so an old baseline compares with a new candidate:

- `wakes: {reasons} [sleeps N cpu MS] drew {surface: frames} …` about once
  a second: wakeups/s (the loop's turns) and frames drawn per window. Since
  63121b6 the line also has the UI thread's real sleeps (`sleeps_per_s`,
  reported) and wp_presentation's frames and missed refreshes (`presented`,
  reported); the loop sleeps to its next deadline, so a line may cover more
  than a second (rates are turns over the window's seconds).
- `debug: stall N ms: path` — old logs: every loop piece over 16 ms logs,
  innermost first, so a chain of lines is one stall. Since 5b1bde3 (the
  8 ms tripwire) only the step that holds the time logs: the innermost one
  over 8 ms, an outer one when its own code took over 8 ms beyond its logged
  pieces, and the loop's outermost piece when it is over 16 ms
  (`… (own 3.1 ms)`; its time includes its pieces). Either way one stall =
  one outermost piece: an `(own …)` line closes the chains logged just
  before it whose times fit in its pieces' time (ms − own), an old line
  closes every chain below it, a new line without `(own …)` closes nothing
  (nothing inside it logged). Stalls are the chains over 16 ms, timed by
  the outermost piece, named by the longest innermost path: count, total,
  worst, by path. A tripwire log's 8-16 ms steps are counted apart
  (`steps_8_16`, reported, never budgeted: the old log has no such lines).
  Logs from 5b1bde3..63121b6 (round2) lack the outermost `(own …)` line, so
  a turn spent in several 8-16 ms steps doesn't show as a stall there: their
  stall count is a lower bound.
- `debug: frame callback lost on <surface>`: count by surface.
- `panicked at`: panics. Pid changes: restarts. The shell gone at the end.
- `first-frame <surface> … drew N ms` and the start-up time: reported only.
- CPU % of one core for each window from the ticks; RssAnon at the end of
  each repetition (the script's memory: their median over reps 2..N; the
  leak trend: least-squares MB per repetition over reps 2..N, rep 1 builds
  what first opens make); idle RssAnon; VmRSS and PSS reported. "RssAnon"
  in the budgets is RssAnon + VmSwap when the sampler recorded VmSwap
  (`swap_mb_max` says how much was in swap): an idle shell's pages are the
  first the kernel swaps out when builds press on memory (17 Nov: 54 MB of
  a 69 MB RssAnon in swap at idle, with two builds running), and RssAnon
  alone then reads tens of MB low.
- The machine: the 1-minute load and the running-task count per sample.

## Budgets (`thresholds.json`, `compare.py`)

A metric fails when candidate > baseline + max(abs, rel × baseline).

| budget | rule |
|---|---|
| idle CPU | median of the 3 idle windows; +max(0.5 pp, 25 %) |
| script memory (RssAnon, median of reps 2..N) | +10 MB |
| idle memory (RssAnon) | +10 MB |
| panics, restarts | none new; the shell alive at the end |
| stalls > 16 ms (count, total ms) | +max(15, 40 %); +max(800 ms, 40 %) |
| stalls > 33 ms (count, total ms) | +max(8, 30 %); +max(400 ms, 30 %) |
| frame-callback losses | +max(3, 30 %) |
| RSS trend | > 2 MB per repetition fails, unless the baseline grows as much (within 1 MB/rep: a warning, not this change) |
| reported, never failing | script CPU, idle wakeups/s (and sleeps/s), worst stall, RssAnon after the lock, 8-16 ms steps, presented/missed frames |
| machine busy | more than 8 tasks running on average on either side: UNRELIABLE, no verdict |

## Noise: five A/A runs (baseline = candidate = 41539ac)

16 Nov 2026 on server1, one run each (raw data in `~/tb-gate-reports/aa1..aa5`;
aa1 went through a retry, the first attempt flagged UNRELIABLE at 9.8 tasks
running). The A/A differences, candidate − baseline:

| run | how | idle CPU pp | idle RssAnon MB | script RssAnon MB | stalls | stall ms | stalls > 33 ms | > 33 ms total | busy (tasks) |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| aa1 | one after the other | +0.35 | (VmRSS −21.6) | (VmRSS −11.1) | +20 | +604 | +7 | +394 | 4.5 / 2.7 |
| aa2 | one after the other | +0.20 | −12.9 | −8.5 | −5 | −115 | +4 | +75 | 2.0 / 2.1 |
| aa3 | side by side | +0.05 | −4.2 | −28.0 | −17 | −673 | −8 | −481 | 0.7 |
| aa4 | side by side | −0.05 | −4.8 | −2.2 | −16 | −726 | −10 | −531 | 0.5 |
| aa5 | side by side, malloc capped | −0.10 | +9.5 | +1.4 | −10 | −291 | +1 | −51 | 0.5 |
| aa3+aa4 | two rounds, arenas swapped, averaged | −0.05 | +0.3 | −12.9 | −0.5 | +26 | +1 | +25 | |

What it showed, and what the gate does about it:
- **Idle CPU** is steady: ±0.35 pp at most (0.5-0.95 % of a core). The third
  idle window is always higher (1.4-1.95 %: a minute boundary, the clock
  redraws), so the median of three is compared. Budget +max(0.5 pp, 25 %).
- **Arena 13 draws slower than arena 14 side by side** (aa3, aa4: 14-17 vs
  4-9 long stalls, the same binary), and one after the other the later run
  differs. Hence two rounds with the arenas swapped, averaged: aa3+aa4
  combined that way agree within 1 long stall and 26 ms.
- **Stalls of 16-33 ms are GPU jitter** here (server1's NVIDIA under a hidden
  KWin): ±20 between runs of one binary. Over 33 ms is steadier. Budgets:
  all stalls +max(15, 40 %) and +max(800 ms, 40 %); over 33 ms +max(8, 30 %)
  and +max(400 ms, 30 %).
- **Memory**: VmRSS includes ~140 MB of file pages (the 180 MB binary, the
  GPU driver) that differ by 20+ MB between runs, so the budgets use RssAnon.
  Even that differed by up to 28 MB with glibc's default malloc arenas (one
  per thread, ~60 threads). With `MALLOC_ARENA_MAX=2` (both sides alike,
  `GATE_MALLOC_ARENAS`) the script memory fell from ~210-240 MB to ~80 MB and
  the A/A difference to 1.4 MB (aa5: one run). That one run was lucky:
  17 Nov, with two arenas, one binary (6e86735) read 78-96 MB of script
  RssAnon and another (63121b6) 80-98 MB, and the round's base+change
  builds 76-108 MB, one run each: which threads share the second arena is
  luck (2.5-32 MB of it in use at the end of the reps). The round's gate
  run (round2: +19.5 MB, a FAIL) was that luck. Since then one arena
  (`GATE_MALLOC_ARENAS=1`) and a fixed mmap threshold of 64 KiB
  (`GATE_MALLOC_MMAP`: big buffers, decoded images, go back to the system
  when freed; glibc's own threshold grows with each such free and keeps
  them in the heap): two runs of 6e86735 read 47.3 / 47.4 MB idle and
  68.7 / 69.0 MB script, of 63121b6 47.7 / 47.7 and 69.6 / 69.5. Stalls,
  frames and CPU didn't change with it. Budgets: script RssAnon +10 MB,
  idle RssAnon +10 MB (the requested budgets; the noise is now ~0.5 MB).
  Real sessions keep glibc's defaults (an arena per thread): there the shell
  holds 200-240 MB, and the gate's number is what it uses, not what it keeps.
- **Leak trend**: up to 1.4 MB/rep in A/A runs (rep 1 excluded: it builds
  what first opens make); budget 2 MB/rep.
- **Frame-callback losses**: 0-1 per run; budget +3.
- Under load (aa1's first attempt, 9.8 tasks running on the candidate's
  side) the same binary showed 67 vs 137 stalls: why busy runs get no verdict.

Not yet run, because every round holds everyone's builds: an A/A with the
final defaults (two rounds, swapped arenas, malloc capped). Run
`tools/gate/run.sh 41539ac 41539ac` once on a quiet evening; if it fails a
budget, widen that one in `thresholds.json` and note it here. Until then
treat a memory FAIL within a few MB of the line, or a single-round run
(`GATE_ROUNDS=1`), with care.

## Baseline: 41539ac

Per run (aa1-aa5; aa5 is with the final malloc setting):

| | |
|---|---|
| idle CPU (median of 3 × 20 s) | 0.5-0.95 % of a core |
| idle wakeups/s, frames/s | 20-23 /s, 0.4-0.55 frames/s (the bar's clock, a desktop redraw at the minute) |
| script CPU (4 reps + 120 Hz + scale 1.5 + lock, ~3 min) | 8.5-11.4 % of a core |
| frames drawn in the script | ~6,300-6,500 |
| stalls > 16 ms | 38-123 per run (38-57 side by side, quiet) |
| stalls > 33 ms | 4-25 per run, 184-1,347 ms in all |
| worst stall | 70-133 ms (`femtovg draw tessera:bar`, `controlcenter`, `settings`) |
| frame-callback losses | 0-1 |
| panics, restarts | 0, 0 |
| RssAnon idle / script | 57-67 / ~80 MB malloc capped (70-84 / 200-240 MB uncapped) |
| VmRSS idle / end, PSS end | ~213-239 / ~390-430 MB, ~185-360 MB |
| RSS trend over reps 2-4 | 0.0-1.4 MB/rep |
| start-up (main → first frame) | 280-510 ms |
| first frame after a show (aa5) | Control Center 11-16 ms, Settings 27-39 ms, lock 8-30 ms, others 0.5-3 ms |

Stall paths are almost all `render > render tessera:<surface> > femtovg draw`
of the bar, Settings, Control Center, Launchpad and Mission Control; the
loop's own work (`msg host …`) shows 0-2 stalls per run.

## The sandbox next to real sessions

`tools/gate/reallog.py <tessera-ui.log>` sums up a real session's timing log
the same way (stall chains grouped as in the gate; wakes lines give the
seconds covered). Read-only, 16 Nov 2026, ~22:00:

| | laptop, current session | laptop, the session before (`.log.1`) | server1's own session | gate sandbox (A/A, per run) |
|---|---|---|---|---|
| time covered (wake-log seconds) | 1.0 h | 7.1 h | 0.3 h (locked the whole time) | 5 min, mostly scripted |
| quiet seconds (nothing drawn) | 65 % | 67 % | 0 % (the lock screen animates) | idle windows: 0.4-0.55 frames/s |
| loop wakeups/s | 34.8 | 24.5 | 21.4 | 20-25 idle, ~105 scripted |
| stalls (> 16 ms) | 53, 51.7/h | 245, 34.5/h | 81, 270/h | 40-137 in 5 min |
| stall median / worst | 28.7 / 415 ms | 28.8 / 953 ms | 24.2 / 2782 ms | worst 70-116 ms |
| stalls > 50 ms | 8 | 16 | 18 | ~8-11 |
| biggest stall path | `msg host > part host > host Wm` (32 of 53) | `host Wm` (215 of 245, 8.0 s in all) | `render tessera:lock` (one of 2.8 s), `damage probe tessera:desktop` (1.0 s) | `render > femtovg draw` of bar, settings, lock, launchpad, Mission Control |
| frame-callback losses | 14 | 14,407 (bar 12,155, desktop 2,208) | 5,597 (lock 4,430, bar 1,020) | 0-2 |
| start-up (main → first frame) | 818 ms | 870 ms | 348 ms | 280-510 ms |
| panics | 0 | 0 | 0 | 0 |

What it says:
- Real sessions stall on **host Wm** (the window-management service on the
  loop: Hyprland events) far more than the sandbox, which has two terminals
  and few window events. The gate's window part (open, minimize, restore,
  workspaces, close) exercises it, but lightly; real stalls there come
  from many windows and long uptime.
- The sandbox's stalls are almost all **draws** (`femtovg draw` of a
  surface on first open or a page switch). On server1's NVIDIA GPU under a
  hidden KWin, draws of 16-33 ms jitter run to run, so the gate budgets
  stalls over 33 ms on their own (steadier) and the 16-33 ms ones loosely.
- **Frame-callback losses** are frequent in real sessions (the bar on the
  laptop, the lock on server1: an output that stops sending frame callbacks
  while dimmed or locked) and nearly absent in the sandbox, whose headless
  output always answers. The gate still counts them, but a regression there
  would mostly show in real use.
- Real worst stalls (0.4-2.8 s: the emoji picker's first draw, a bar draw,
  the lock) are far longer than anything the script provokes; those are
  one-offs a 5-minute script can't promise to catch.

## Limits and what's left

- The sandbox draws through a hidden KWin on server1's NVIDIA GPU, not the
  laptop's Intel one: absolute numbers (draw times, stall counts) differ from
  the laptop; only the comparison is meant to be trusted.
- The glass plugin, Hyprland config and sandbox scripts are the gate
  checkout's for both sides: a change to those isn't measured.
- Menus that only open from a pointer click on the bar aren't in the script
  (the Wi-Fi menu is opened over IPC); typing goes through `wtype`.
- The idle windows include the timing log's own once-a-second line.
