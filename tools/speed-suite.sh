#!/usr/bin/env bash
# speed-suite.sh: how fast each panel opens and closes, the QML shell (the
# last all-QML checkout, --code) or tessera-ui standalone (--bin), in one
# hidden sandbox arena with your settings and the look asked for.
#
#   tools/speed-suite.sh --side qml|rust --arena N --look glass|material --out DIR
#                        [--code DIR] [--bin TESSERA_UI] [--reps 3] [--only 'a|b'] [--env K=V]…
#
# Per panel and repetition: open, then close, each watched by
# tools/speed-probe.py inside the sandbox: every compositor frame on the test
# screen gets a time (the glass plugin's frame counter, read every ~3 ms), and
# Hyprland's layer events say when a panel's surface mapped and unmapped.
# So: trigger → first frame, first → last frame of the motion, frames more
# than 20 ms apart and the longest gap. No recorder runs (a recorder's own
# encoding dropped frames and hid the shell's). The "calib" row triggers a
# pointer move: the harness's own start-up, to subtract.
# Results: OUT/<side>-<look>.tsv (tools/speed-table.py makes the tables);
# with --env TESSERA_DEBUG=1 the Rust stall lines per panel: OUT/<side>-<look>-perf.log.
# Never your screen: the arena's TESSERA-TEST only.
set -uo pipefail
TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SIDE="" ARENA="" LOOK=glass OUT="" CODE="" BIN="" REPS=3 ONLY="" ENVS=()
USERCFG="$HOME/tessera/.sandbox/home/.config/tessera/config.json"
while (($#)); do case "$1" in
    --side) SIDE=$2; shift;; --arena) ARENA=$2; shift;; --look) LOOK=$2; shift;; --out) OUT=$2; shift;;
    --code) CODE=$2; shift;; --bin) BIN=$2; shift;; --reps) REPS=$2; shift;; --only) ONLY=$2; shift;;
    --env) ENVS+=("$2"); shift;; *) echo "unknown option $1"; exit 2;; esac; shift; done
[[ -n $SIDE && -n $ARENA && -n $OUT ]] || { sed -n 2,8p "$0"; exit 2; }
[[ $ARENA == 1 ]] && { echo "never arena 1 (yours)"; exit 2; }
CODE=${CODE:-$(cd "$TOOLS/.." && pwd)}
export TESSERA_ARENA=$ARENA
# shellcheck source=/dev/null
. "$CODE/site/film/tessera/capture/lib.sh"   # (ROOT, HOMEDIR, dev, inbox, setup: that checkout's)
CFG="$HOMEDIR/.config/tessera/config.json"
mkdir -p "$OUT"
TAG="$SIDE-$LOOK"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$OUT/run-$TAG.log"; }
trap 'dev down >/dev/null 2>&1' EXIT

use_config() {
    python3 -I - "$USERCFG" "$CFG" "$LOOK" "$SIDE" <<'PY'
import json, sys
src, dst, look, side = sys.argv[1:]
c = json.load(open(src))
c.setdefault("appearance", {})["look"] = look
r = c.setdefault("rust", {})
r["standalone"] = side == "rust"
r["surfaces"] = []
for k in ("ai", "google", "cloud", "phone"):   # (accounts and keys stay out of the arena)
    c.pop(k, None)
json.dump(c, open(dst, "w"), indent=4)
PY
}

# name | open | close   (sc NAME: the global shortcut as Hyprland's bind runs it;
# anything else: a command in the sandbox's prelude: ipc, P, key)
PANELS=(
    "calib|P move 960 540|P move 1500 700"
    "controlCenter|sc controlCenter|key -k Escape"
    "notificationCenter|sc notificationCenter|key -k Escape"
    "launchpad|sc launchpad|key -k Escape"
    "spotlight|sc spotlight|key -k Escape"
    "overview|sc overview|key -k Escape"
    "missionControl|sc missionControl|key -k Escape"
    "soundMenu|ipc panels open soundMenu TESSERA-TEST|key -k Escape"
    "batteryMenu|ipc panels open batteryMenu TESSERA-TEST|key -k Escape"
    "systemMenu|ipc panels open systemMenu TESSERA-TEST|key -k Escape"
    "island|ipc island post bell Speed test|ipc island dismissNewest"
)
cmd_of() { [[ $1 == sc\ * ]] && echo "hyprctl dispatch 'hl.dsp.global(\"tessera:${1#sc }\")' >/dev/null" || echo "$1"; }
CLEAN='ipc panels close; ipc island dismissNewest; ipc island dismissNewest; P move 1500 700'

# probe_cmd COMMAND: tools/speed-probe.py in the sandbox around it
printf '%s' "$PRELUDE" >"$OUT/prelude-$TAG.sh"
probe_cmd() {
    printf '%s' "$1" >"$OUT/cmd-$TAG"
    inbox "python3 -I '$TOOLS/speed-probe.py' '$OUT/prelude-$TAG.sh' \"\$(cat '$OUT/cmd-$TAG')\" 1.5" </dev/null 2>/dev/null | tail -1
}

log "setup: arena $ARENA, $SIDE, $LOOK, code $CODE"
dev down </dev/null >/dev/null 2>&1
[[ $HOMEDIR == */.sandbox/arena-[2-9]/home && -d $HOMEDIR ]] && rm -rf "$HOMEDIR"
setup material "$WALLS/wall-tiles.jpg" 1 >/dev/null 2>&1
use_config
if [[ $SIDE == rust ]]; then
    if ((${#ENVS[@]})); then
        install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/real/tessera-ui"
        printf '#!/bin/sh\nexec env %s %q "$@"\n' "$(printf '%q ' "${ENVS[@]}")" "$HOMEDIR/.local/share/tessera/bin/real/tessera-ui" >"$HOMEDIR/.local/share/tessera/bin/tessera-ui"
        chmod 755 "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    else
        install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    fi
    rm -f "$HOMEDIR/.local/state/tessera/standalone-starts"
fi
dev reload </dev/null >/dev/null 2>&1
sleep 20
inbox "ipc lock unlock; $CLEAN" </dev/null >/dev/null 2>&1; sleep 2
# (the ipc client's own time, which ipc-triggered rows include: qs ipc call vs tessera-ui ipc)
ipcms=$(inbox 'for i in 1 2 3 4 5; do s=$(date +%s%N); ipc island timeLeft; e=$(date +%s%N); echo $(( (e - s) / 1000000 )); done' </dev/null 2>/dev/null | sort -n | sed -n 3p)
log "ipc client: median ${ipcms:-?} ms"
logf="$HOMEDIR/.local/state/tessera/tessera-ui.log"
TSV="$OUT/$TAG.tsv"
printf 'panel\trep\tphase\tfirst\tlast\tframes\tlate\tgap\tmap\tmap_ns\tunmap\tunmap_ns\n' >"$TSV"
: >"$OUT/$TAG-perf.log"
for e in "${PANELS[@]}"; do
    IFS='|' read -r name how closeby <<<"$e"
    [[ -n $ONLY && ! $name =~ ^($ONLY)$ ]] && continue
    open=$(cmd_of "$how") close=$(cmd_of "$closeby")
    for rep in $(seq 1 "$REPS"); do
        mark=$(wc -l <"$logf" 2>/dev/null || echo 0)
        o=$(probe_cmd "$open"); sleep 0.3
        c=$(probe_cmd "$close")
        inbox "$CLEAN" </dev/null >/dev/null 2>&1; sleep 1
        [[ -f $logf ]] && { echo "## $name $rep"; tail -n +"$((mark + 1))" "$logf" | grep -E '^debug: (stall|reload)'; } >>"$OUT/$TAG-perf.log"
        printf '%s\t%s\topen\t%s\n%s\t%s\tclose\t%s\n' "$name" "$rep" "$o" "$name" "$rep" "$c" >>"$TSV"
    done
    log "$name: $(grep "^$name	" "$TSV" | cut -f3-8 | tr '\t' ' ' | tr '\n' '|')"
done
log "done: $TSV"
