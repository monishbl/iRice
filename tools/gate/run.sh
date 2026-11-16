#!/usr/bin/env bash
# The smoothness + stability gate, on the build machine (docs/gate.md):
#   tools/gate/run.sh <baseline-ref> <candidate-ref>
# Builds each ref's tessera-ui (cargo --profile fast --locked) in its own
# gate copy (~/tb-gate-base, ~/tb-gate-cand: `git clone --shared` of
# ~/tessera-build, targets warmed from its target/fast with reflinks), keeps
# each binary by commit (~/tb-gate-bins/<sha>: a ref built once is not built
# again), runs the same scripted session on both in hidden arenas 13 and 14
# (gate.sh), and compares them (compare.py). Prints the report's path and
# the verdict; exit 0 = pass, 1 = fail, 2 = couldn't run, 3 = unreliable
# (more than GATE_MAX_LOAD tasks running besides the builds while measuring,
# after retries). Builds go through ~/bin/tbuild (a low-weight cgroup scope;
# plain nice'd cargo without it); while measuring, ~/.cache/tessera-governor.hold
# makes ~/bin/tgovernor pause every Tessera build (removed on exit); arenas
# at nice 10.
# Env: GATE_ROUNDS (2: the second with the arenas swapped, each side's
# rounds averaged), GATE_PARALLEL (1: both arenas at once; 0: one after the
# other, the order swapped too), GATE_REPS, GATE_IDLE, GATE_MALLOC_ARENAS
# (gate.sh), GATE_TB (~/tessera-build), GATE_OUT (report dir), GATE_MAX_LOAD
# (8), GATE_WAIT (60 s, inside the hold, for a quiet machine), GATE_RETRIES (1), GATE_HOLD_MAX
# (600 s: the longest the builds are held; each hold is followed by a pause
# as long before the next).
# The sandbox (dev.sh, hypr config, glass plugin, the script) is this
# checkout's for both sides: only the tessera-ui binary differs.
set -uo pipefail
[[ $# -eq 2 ]] || { sed -n '2,26p' "$0"; exit 2; }
BASE_REF=$1 CAND_REF=$2
GATE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$GATE/../.." && pwd)"
TB=${GATE_TB:-$HOME/tessera-build}
BINS=$HOME/tb-gate-bins
T0=$(date +%s)
say() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >&2; }

# ── This checkout's sandbox tools: the virtual pointer and the glass plugin ──
if [[ ! -x $ROOT/.sandbox/bin/vptr ]]; then
    mkdir -p "$ROOT/.sandbox/bin"
    if [[ -x $TB/.sandbox/bin/vptr ]]; then cp "$TB/.sandbox/bin/vptr" "$ROOT/.sandbox/bin/vptr"
    else (cd "$ROOT/sandbox/tools" && ./build.sh) >&2 || { say "can't build vptr"; exit 2; }; fi
fi
SO="$ROOT/hypr/plugin/tessera-glass/tessera-glass.so"
if [[ ! -f $SO ]]; then
    say "building the glass plugin"
    make -C "$ROOT/hypr/plugin/tessera-glass" >/dev/null 2>&1 || cp "$TB/hypr/plugin/tessera-glass/tessera-glass.so" "$SO" || { say "no glass plugin"; exit 2; }
fi

# ── Build a ref (once per commit) → the binary's path on stdout ──
build() { # side ref
    local side=$1 ref=$2 dir=$HOME/tb-gate-$1 sha
    if [[ ! -d $dir/.git ]]; then
        say "[$side] cloning $TB"
        git clone -q --shared "$TB" "$dir" || return 1
    fi
    git -C "$dir" fetch -q "$TB" '+refs/heads/*:refs/remotes/tb/*' 2>/dev/null
    sha=$(git -C "$dir" rev-parse -q --verify "$ref^{commit}" 2>/dev/null || git -C "$dir" rev-parse -q --verify "tb/$ref^{commit}" 2>/dev/null) \
        || { say "[$side] no commit '$ref' in $TB"; return 1; }
    if [[ -x $BINS/$sha/tessera-ui ]]; then
        say "[$side] $ref = ${sha:0:9}: built before"
        echo "$BINS/$sha/tessera-ui"; return 0
    fi
    git -C "$dir" checkout -q -f "$sha" || return 1
    local tgt=$dir/native/tessera-ui/target
    if [[ ! -d $tgt/fast && -d $TB/native/tessera-ui/target/fast ]]; then
        say "[$side] warming the target from $TB (reflinks)"
        mkdir -p "$tgt"
        cp -r --reflink=auto "$TB/native/tessera-ui/target/fast" "$tgt/" || return 1
        cp -f "$TB/native/tessera-ui/target/CACHEDIR.TAG" "$tgt/" 2>/dev/null
    fi
    say "[$side] building ${sha:0:9} (cargo --profile fast --locked)"
    local t=$(date +%s)
    # (server1 is someone's computer too: ~/bin/tbuild runs cargo in a
    # low-weight scope the build governor watches; without it, nice'd by hand)
    local cargo=(nice -n 19 ionice -c 3 cargo)
    [[ -x $HOME/bin/tbuild ]] && cargo=("$HOME/bin/tbuild")
    (cd "$dir/native/tessera-ui" && CARGO_BUILD_JOBS=${GATE_JOBS:-10} "${cargo[@]}" build --profile fast --locked --bin tessera-ui) >"$dir/gate-build.log" 2>&1 \
        || { say "[$side] build failed: $dir/gate-build.log"; tail -15 "$dir/gate-build.log" >&2; return 1; }
    say "[$side] built in $(( $(date +%s) - t )) s"
    mkdir -p "$BINS/$sha"
    cp -f "$tgt/fast/tessera-ui" "$BINS/$sha/tessera-ui.tmp" && mv -f "$BINS/$sha/tessera-ui.tmp" "$BINS/$sha/tessera-ui"
    echo "$BINS/$sha/tessera-ui"
}

BASE_BIN=$(build base "$BASE_REF") || exit 2
# (the same commit both sides: one build, the A/A run that measures the noise)
CAND_SHA=$(git -C "$HOME/tb-gate-base" rev-parse -q --verify "$CAND_REF^{commit}" 2>/dev/null || git -C "$HOME/tb-gate-base" rev-parse -q --verify "tb/$CAND_REF^{commit}" 2>/dev/null || true)
if [[ -n $CAND_SHA && -x $BINS/$CAND_SHA/tessera-ui ]]; then
    CAND_BIN=$BINS/$CAND_SHA/tessera-ui
else
    CAND_BIN=$(build cand "$CAND_REF") || exit 2
fi
BSHA=$(basename "$(dirname "$BASE_BIN")") CSHA=$(basename "$(dirname "$CAND_BIN")")

OUT=${GATE_OUT:-$HOME/tb-gate-reports/$(date +%Y%m%d-%H%M%S)-${BSHA:0:7}-${CSHA:0:7}}
mkdir -p "$OUT"
MAXLOAD=${GATE_MAX_LOAD:-8}
export GATE_MAX_LOAD=$MAXLOAD

# ── Builds paused while measuring: ~/bin/tgovernor freezes every Tessera build
# (its cgroup scope) while ~/.cache/tessera-governor.hold exists ──
HOLD=$HOME/.cache/tessera-governor.hold
GOV=$HOME/.cache/tessera-governor.json
# Everyone else's builds wait while it's there, so a hold is short and never
# back to back: at most GATE_HOLD_MAX (600 s; a watchdog removes the file
# then, whatever the run is doing), and the next hold, this run's or the next
# run's, only after a pause at least as long as the last hold
# (~/.cache/tessera-gate-hold.last: "<end> <seconds held>")
HOLD_MAX=${GATE_HOLD_MAX:-600}
LAST=$HOME/.cache/tessera-gate-hold.last
HOLD_T0="" WD=""
release() {
    [[ -n $WD ]] && kill "$WD" 2>/dev/null
    WD=""
    rm -f "$HOLD"
    if [[ -n $HOLD_T0 ]]; then
        echo "$(date +%s) $(( $(date +%s) - HOLD_T0 ))" >"$LAST"
        HOLD_T0=""
    fi
}
trap release EXIT
trap 'exit 130' INT TERM
hold() {
    local end held wait
    if read -r end held <"$LAST" 2>/dev/null && [[ $end =~ ^[0-9]+$ && $held =~ ^[0-9]+$ ]]; then
        wait=$(( end + held - $(date +%s) ))
        if ((wait > 0)); then
            say "builds' turn: the last hold was ${held} s, waiting ${wait} s before holding again"
            sleep "$wait"
        fi
    fi
    mkdir -p "${HOLD%/*}"; : >"$HOLD"
    HOLD_T0=$(date +%s)
    # (the watchdog: its own subshell, killed by its pid in release)
    ( sleep "$HOLD_MAX"; rm -f "$HOLD" ) & WD=$!
    # (the governor says hold, nothing of the builds still running: ~2 s)
    local i
    for i in $(seq 20); do
        python3 -I - "$GOV" <<'PY' && return 0
import json, sys, time
try:
    g = json.load(open(sys.argv[1]))
except Exception:
    sys.exit(1)
fresh = time.time() - g.get("at", 0) < 10
running = [b for b in g.get("builds", []) if not (b.get("paused") or b.get("frozen"))]
sys.exit(0 if fresh and g.get("hold") and not running else 1)
PY
        sleep 1
    done
    say "the build governor didn't confirm the hold ($GOV): builds may run during the measurement"
}
# How busy the machine is right now: runnable tasks (/proc/loadavg's 4th
# field, not the 1-minute average, which lags a paused build by minutes),
# averaged over 5 s; frozen builds don't count
runnable() {
    python3 -I -c '
import time
v = []
for _ in range(10):
    v.append(int(open("/proc/loadavg").read().split()[3].split("/")[0]) - 1)
    time.sleep(0.5)
print(round(sum(v) / len(v), 1))'
}
# Wait (GATE_WAIT s, 60: the builds are held meanwhile) for the rest of the machine to be quiet. Still
# busy: measured anyway, and the report says UNRELIABLE (exit 3).
quiet() {
    local waited=0 r
    while r=$(runnable); awk -v l="$r" -v m="$MAXLOAD" 'BEGIN { exit !(l > m - 2) }'; do
        ((waited == 0)) && say "$r tasks running: waiting for fewer than $((MAXLOAD - 2)) (up to ${GATE_WAIT:-60} s)"
        ((waited >= ${GATE_WAIT:-60})) && { say "still busy ($r running): measuring anyway"; return; }
        sleep 15; waited=$((waited + 15))
    done
}
# One round: both sides, base in arena $1 and the candidate in $2, into
# $OUT/<side><round>. Parallel (both at once: what the machine does to one
# it does to the other) unless GATE_PARALLEL=0
round() { # base-arena cand-arena n
    local rb rc pb pc
    say "round $3: baseline ${BSHA:0:9} in arena $1, candidate ${CSHA:0:9} in arena $2"
    # (the sandboxes below the desktop's own work, above the builds)
    if [[ ${GATE_PARALLEL:-1} == 1 ]]; then
        nice -n 10 "$GATE/gate.sh" "$1" "$BASE_BIN" "$OUT/base$3" & pb=$!
        nice -n 10 "$GATE/gate.sh" "$2" "$CAND_BIN" "$OUT/cand$3" & pc=$!
        wait $pb; rb=$?; wait $pc; rc=$?
    elif (($3 % 2)); then
        nice -n 10 "$GATE/gate.sh" "$1" "$BASE_BIN" "$OUT/base$3"; rb=$?
        nice -n 10 "$GATE/gate.sh" "$2" "$CAND_BIN" "$OUT/cand$3"; rc=$?
    else
        nice -n 10 "$GATE/gate.sh" "$2" "$CAND_BIN" "$OUT/cand$3"; rc=$?
        nice -n 10 "$GATE/gate.sh" "$1" "$BASE_BIN" "$OUT/base$3"; rb=$?
    fi
    ((rb == 0 && rc == 0)) || { say "a run failed (baseline $rb, candidate $rc): $OUT"; return 1; }
}
# GATE_ROUNDS (2): the second round swaps the arenas (and, run one after
# the other, the order): A/A runs showed arena 13 drawing slower than 14
# when both run at once, and a later run differing from an earlier one;
# each side's rounds are averaged (compare.py)
ROUNDS=${GATE_ROUNDS:-2}
for attempt in $(seq $((1 + ${GATE_RETRIES:-1}))); do
    say "measuring (attempt $attempt, $ROUNDS rounds) → $OUT"
    bl=() cl=()
    for n in $(seq "$ROUNDS"); do
        # (a hold per round, ~5.5 min; the builds get as long again between)
        hold
        quiet
        if ((n % 2)); then round 13 14 "$n"; else round 14 13 "$n"; fi || { release; exit 2; }
        [[ -f $HOLD ]] || say "round $n outlasted the ${HOLD_MAX} s hold: builds may have run at its end"
        release
        bl+=("$OUT/base$n/run.json") cl+=("$OUT/cand$n/run.json")
    done
    python3 -I "$GATE/compare.py" "$(IFS=,; echo "${bl[*]}")" "$(IFS=,; echo "${cl[*]}")" "$OUT" "$BASE_REF (${BSHA:0:9})" "$CAND_REF (${CSHA:0:9})"
    verdict=$?
    ((verdict == 3 && attempt <= ${GATE_RETRIES:-1})) || break
    # (kept for the record, then measured again)
    say "the machine was busy: measuring again"
    mv "$OUT" "$OUT-busy-$attempt"; mkdir -p "$OUT"
done
say "radios: $(tail -1 "$OUT/cand1/radios.txt")"
say "report: $OUT/report.md ($(( $(date +%s) - T0 )) s)"
exit $verdict
