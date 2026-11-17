#!/usr/bin/env bash
# One measured run of the gate (tools/gate): one tessera-ui binary in one
# hidden arena, the same seeded home, wallpaper, settings and script every
# time. run.sh calls it once per side; compare.py compares two runs.
#   tools/gate/gate.sh <arena> <tessera-ui binary> <out dir>
# Env: GATE_REPS (4) interaction repetitions, GATE_IDLE ("20:3": three 20 s
# idle windows), TESSERA_NO_BOX (1 on the build machine).
# Writes <out>/{events.tsv,samples.tsv,log.tsv,run.json,radios.txt}.
# Never kills by a pattern: the sampler by its own pid, the arena by dev.sh.
set -uo pipefail
ARENA=$1 BIN=$(realpath "$2") OUT=$3
GATE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TESSERA_NO_BOX=${TESSERA_NO_BOX:-1} IRICE_ARENA=$ARENA TESSERA_ARENA=$ARENA
REPS=${GATE_REPS:-4} IDLE=${GATE_IDLE:-20:3}
# (glibc's malloc: one arena per thread by default, and with ~60 threads
# which ones get touched varies run to run: RssAnon of one binary differed
# by up to 40 MB between runs. GATE_MALLOC_ARENAS (2) caps them for both
# sides alike, so the memory budget measures the change, not the luck;
# 0 leaves glibc's default. It reaches the shell through dev.sh reload's env.
# Two arenas still left it to luck: which threads share the second one
# (17 Nov, one binary: 2.5-32 MB there, script RssAnon 78-108 MB). One arena
# (GATE_MALLOC_ARENAS=1) and a fixed mmap threshold (GATE_MALLOC_MMAP,
# 64 KiB: big buffers, decoded images, go back to the system when freed,
# instead of glibc's threshold growing with each free and keeping them in
# the heap) measure what the shell holds, within ~1 MB run to run)
if [[ ${GATE_MALLOC_ARENAS:-1} != 0 ]]; then export MALLOC_ARENA_MAX=${GATE_MALLOC_ARENAS:-1}; fi
if [[ ${GATE_MALLOC_MMAP:-65536} != 0 ]]; then export MALLOC_MMAP_THRESHOLD_=${GATE_MALLOC_MMAP:-65536}; fi
. "$GATE/../../site/film/tessera/capture/lib.sh"   # (dev, inbox, setup, HOMEDIR, WALLS)
. "$ROOT/sandbox/lib.sh"                            # (TESSERA_RT)
RT=$TESSERA_RT
WALL="$WALLS/wall-tiles.jpg"
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
: >"$OUT/events.tsv"
say() { printf '[%s] arena %s: %s\n' "$(date +%H:%M:%S)" "$ARENA" "$*" | tee -a "$OUT/gate.log" >&2; }
mark() { printf '%s\t%s\t%s\n' "$(date +%s.%N)" "$1" "${2:-}" >>"$OUT/events.tsv"; }
radios() { echo "$(date +%T) wifi=$(nmcli -t -f WIFI general 2>/dev/null) bt=$(bluetoothctl show 2>/dev/null | grep -m1 Powered | xargs)"; }
[[ -x $BIN ]] || { say "no binary at $BIN"; exit 2; }
[[ -x $ROOT/.sandbox/bin/vptr ]] || { say "no $ROOT/.sandbox/bin/vptr (run.sh prepares it)"; exit 2; }
radios >"$OUT/radios.txt"

SAMPLER=""
finish() {
    [[ -n $SAMPLER ]] && kill "$SAMPLER" 2>/dev/null && wait "$SAMPLER" 2>/dev/null
    SAMPLER=""
    dev down >/dev/null 2>&1
    radios >>"$OUT/radios.txt"
}
trap finish EXIT

# ── A fresh arena home: the gate's settings, nothing left from before ──
dev down >/dev/null 2>&1
case "$HOMEDIR" in */.sandbox/arena-"$ARENA"/home) rm -rf "${HOMEDIR%/home}" ;; *) say "odd arena home $HOMEDIR"; exit 2 ;; esac
mkdir -p "$HOMEDIR/.config/tessera" "$HOMEDIR/.local/state/tessera"
python3 -I "$GATE/config.py" "$HOMEDIR/.config/tessera/config.json" "$WALL"
install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
UI="$HOMEDIR/.local/share/tessera/bin/tessera-ui"
LOG="$HOMEDIR/.local/state/tessera/tessera-ui.log"

say "arena up"
dev up >/dev/null 2>&1 || { say "arena didn't start"; exit 2; }
# (the timing log, on from the measured start: wayland.rs wakes_on)
touch "$RT/tessera-wakes"
python3 -I "$GATE/sampler.py" "$RT" "$LOG" "$OUT" &
SAMPLER=$!
sleep 4
# lib.sh's setup: the seeded look (Glass, dark, liquid), the wallpaper, the
# test screen at 1x 1920x1080@60, the island cleared; its reload is the start
# that's measured
mark setup-start
rm -f "$HOMEDIR/.local/state/tessera/standalone-starts"
setup glass "$WALL" 1 >/dev/null 2>&1
python3 -I "$GATE/config.py" "$HOMEDIR/.config/tessera/config.json" "$WALL" --check || say "seeded settings changed by the shell"
mark setup-end
inbox 'ipc lock unlock; ipc panels close; ipc island clearHistory' >/dev/null 2>&1

# ── Wait for idle: no frame drawn for 3 s, 8 s after the start at least (40 s at most) ──
say "waiting for idle"
for _ in $(seq 80); do
    sleep 0.5
    python3 -I - "$OUT" <<'PY' && break
import sys, time
out = sys.argv[1]
ev = [l.split("\t") for l in open(out + "/events.tsv")]
setup_end = max(float(e[0]) for e in ev if e[1] == "setup-end")
last = setup_end
for l in open(out + "/log.tsv", errors="replace"):
    t, _, msg = l.partition("\t")
    if msg.startswith("wakes: ") and " drew {} " not in msg:
        last = max(last, float(t))
now = time.time()
sys.exit(0 if now - last >= 3 and now - setup_end >= 8 else 1)
PY
done
mark measure-start
say "measuring: idle $IDLE, $REPS reps, 120 Hz, scale 1.5, lock"
dev run bash "$GATE/scenario.sh" "$OUT/events.tsv" "$UI" "$REPS" "idle:$IDLE" reps hz120 scale15 lock >/dev/null 2>&1
mark measure-end
sleep 1.5
kill "$SAMPLER" 2>/dev/null; wait "$SAMPLER" 2>/dev/null; SAMPLER=""
cp -f "$HOMEDIR/.config/tessera/config.json" "$OUT/config.json" 2>/dev/null
python3 -I "$GATE/analyze.py" "$OUT" >"$OUT/run.json" || { say "analysis failed"; exit 2; }
say "done: $(python3 -I -c 'import json,sys; r=json.load(open(sys.argv[1])); print("idle cpu %.2f%%, stalls %d, rss %.1f MB" % (r["idle"]["cpu_pct_median"], r["stalls"]["count"], r["mem"]["idle_rss_mb"]))' "$OUT/run.json" 2>/dev/null)"
