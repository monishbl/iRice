#!/usr/bin/env bash
# Where the shell's memory goes in a session like a real one (docs/gate.md,
# "Memory"): glibc's malloc left at its defaults (as launch-shell.sh starts
# it), the gate's seeded home (Glass, the wallpaper), the gate's script
# (every panel, Settings, Spotlight, the dock, windows, 120 Hz, scale 1.5,
# the lock), then left alone. Snapshots by kind of mapping (memmap.py):
# idle after start, right after the script, 1 minute and LATER s after it.
#   tools/gate/memprobe.sh <arena> <tessera-ui binary> <out dir>
# Env: PROBE_REPS (4), PROBE_LATER (600 s), PROBE_WALL (the gate's
# wall-tiles.jpg, 2400x1800; a real wallpaper is often 4-6K), PROBE_ENV ("": extra env for the
# shell, e.g. "MALLOC_ARENA_MAX=2"), PROBE_THP_OFF (1: the shell started
# with transparent huge pages off, prctl; for comparing only).
# Writes <out>/{snap.jsonl,rss.tsv,events.tsv,tessera-ui.log}. Never kills by
# a pattern: the arena by dev.sh, the terminals by scenario.sh's exact pids.
set -uo pipefail
ARENA=$1 BIN=$(realpath "$2") OUT=$3
GATE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export TESSERA_NO_BOX=${TESSERA_NO_BOX:-1} TESSERA_ARENA=$ARENA
REPS=${PROBE_REPS:-4} LATER=${PROBE_LATER:-600}
# (glibc's defaults: nothing of the gate's malloc caps)
unset MALLOC_ARENA_MAX MALLOC_MMAP_THRESHOLD_ MALLOC_TRIM_THRESHOLD_ GLIBC_TUNABLES
for kv in ${PROBE_ENV:-}; do export "${kv?}"; done
. "$GATE/../../site/film/tessera/capture/lib.sh"   # (dev, inbox, setup, HOMEDIR, WALLS)
. "$ROOT/sandbox/lib.sh"                            # (TESSERA_RT)
RT=$TESSERA_RT
WALL=${PROBE_WALL:-$WALLS/wall-tiles.jpg}
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
: >"$OUT/events.tsv"; : >"$OUT/snap.jsonl"; : >"$OUT/rss.tsv"
say() { printf '[%s] arena %s: %s\n' "$(date +%H:%M:%S)" "$ARENA" "$*" | tee -a "$OUT/probe.log" >&2; }
radios() { echo "$(date +%T) wifi=$(nmcli -t -f WIFI general 2>/dev/null) bt=$(bluetoothctl show 2>/dev/null | grep -m1 Powered | xargs)"; }
radios >"$OUT/radios.txt"
finish() { cp -f "$LOG" "$OUT/tessera-ui.log" 2>/dev/null; dev down >/dev/null 2>&1; radios >>"$OUT/radios.txt"; }
trap finish EXIT

dev down >/dev/null 2>&1
case "$HOMEDIR" in */.sandbox/arena-"$ARENA"/home) rm -rf "${HOMEDIR%/home}" ;; *) say "odd arena home $HOMEDIR"; exit 2 ;; esac
mkdir -p "$HOMEDIR/.config/tessera" "$HOMEDIR/.local/state/tessera"
python3 -I "$GATE/config.py" "$HOMEDIR/.config/tessera/config.json" "$WALL"
UI="$HOMEDIR/.local/share/tessera/bin/tessera-ui"
if [[ ${PROBE_THP_OFF:-0} == 1 ]]; then
    install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/real/tessera-ui"
    mkdir -p "$(dirname "$UI")"
    printf '#!/bin/sh\nexec python3 -I -c "import ctypes,os,sys; ctypes.CDLL(None).prctl(41,1,0,0,0); os.execv(sys.argv[1], sys.argv[1:])" %q "$@"\n' \
        "$HOMEDIR/.local/share/tessera/real/tessera-ui" >"$UI"
    chmod 755 "$UI"
else
    install -D -m 755 "$BIN" "$UI"
fi
LOG="$HOMEDIR/.local/state/tessera/tessera-ui.log"

# The shell of this arena: comm tessera-ui, --standalone, this arena's runtime dir
shell_pid() {
    local p
    for p in $(pgrep -x -u "$(id -u)" tessera-ui); do
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -q -- --standalone || continue
        grep -qzx "XDG_RUNTIME_DIR=$RT" "/proc/$p/environ" 2>/dev/null && { echo "$p"; return 0; }
    done
    return 1
}
snap() { local p; p=$(shell_pid) && python3 -I "$GATE/memmap.py" "$p" "$1" >>"$OUT/snap.jsonl" && say "snapshot $1: $(tail -1 "$OUT/snap.jsonl" | python3 -I -c 'import json,sys; r=json.load(sys.stdin); s=r["status"]; k=r["kinds"]; print("RSS %.0f anon %.0f (heap %.0f arenas %.0f other anon %.0f, THP %.0f) PSS %.0f MB" % (s["VmRSS"], s["RssAnon"], k.get("heap",{}).get("rss",0), k.get("arenas",{}).get("rss",0), k.get("anon",{}).get("rss",0), r["total"]["thp"], r["total"]["pss"]))')"; }
rss_line() { local p; p=$(shell_pid) && printf '%s\t%s\t%s\n' "$(date +%s)" "$1" "$(grep -E '^(VmRSS|RssAnon)' "/proc/$p/status" | awk '{printf "%s ", $2}')" >>"$OUT/rss.tsv"; }

say "arena up"
dev up >/dev/null 2>&1 || { say "arena didn't start"; exit 2; }
touch "$RT/tessera-wakes"
sleep 4
rm -f "$HOMEDIR/.local/state/tessera/standalone-starts"
setup glass "$WALL" 1 >/dev/null 2>&1
inbox 'ipc lock unlock; ipc panels close; ipc island clearHistory' >/dev/null 2>&1
sleep 30
snap idle-start
say "script: $REPS reps, 120 Hz, scale 1.5, lock"
dev run bash "$GATE/scenario.sh" "$OUT/events.tsv" "$UI" "$REPS" reps hz120 scale15 lock >/dev/null 2>&1
snap after-script
for i in $(seq 6); do sleep 10; rss_line idle; done
snap after-1m
for i in $(seq $(( (LATER - 60) / 10 ))); do sleep 10; rss_line idle; done
snap "after-$((LATER / 60))m"
say "done"
