#!/usr/bin/env bash
# settings-scenarios.sh: Settings in a hidden arena with the user's config
# (tools/live-scenarios.sh's way), step by step, for recording and timing
# page switches, menus and controls against the QML. Never the real screen.
#   ARENA=N OUT=DIR tools/settings-scenarios.sh <step> …
#   setup <qml|rust> <material|glass> [tessera-ui binary]   config, install, reload, open Settings (Appearance)
#   open [page]                     open Settings on a page (ipc panels settings)
#   shot <name>                     the whole screen into OUT/name.png
#   clicks <name> "x y" "x y" …     record REGION (default the whole screen) while clicking each
#                                   point in turn (move, glide 200 ms, rest 400 ms, click, 1 s);
#                                   OUT/name.mp4 + .clicks + .marker for tools/page-switch.py
#   keys <name> <n>                 record while pressing Down n times (1.3 s apart)
#   run '<shell>'                   anything in the arena (P = pointer, key = wtype, ipc)
#   log | down
# Coordinates are TESSERA-TEST's (1920x1080 at scale 1).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export TESSERA_ARENA=${ARENA:-${TESSERA_ARENA:-8}}
. "$ROOT/site/film/tessera/capture/lib.sh"
OUT=${OUT:-$HOME/tessera-verify/settings-a$TESSERA_ARENA}; mkdir -p "$OUT"
CFG="$HOMEDIR/.config/tessera/config.json"
USERCFG=${USERCFG:-$HOME/tessera/.sandbox/home/.config/tessera/config.json}
LOG=/tmp/tessera-ui-a$TESSERA_ARENA.log
step=${1:-help}; shift || true
case $step in
setup)
    pass=$1 look=$2 bin=${3:-$ROOT/native/tessera-ui/target/release/tessera-ui}
    dev status | grep -q running || { dev up >/dev/null 2>&1; sleep 12; }
    python3 -I "$ROOT/tools/settings-scenarios-cfg.py" "$USERCFG" "$CFG" "$pass" "$look"
    install -D -m 755 "$bin" "$HOMEDIR/.local/share/tessera/bin/tessera-ui.real"
    printf '#!/bin/sh\nexec env TESSERA_DEBUG=1 "$(dirname "$0")/tessera-ui.real" "$@" 2>>%s\n' "$LOG" > "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    chmod +x "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    dev hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null 2>&1
    dev reload </dev/null >/dev/null 2>&1; sleep 16
    dev hyprctl eval 'hl.monitor({ output = "TESSERA-TEST", mode = "1920x1080@60", position = "20000x20000", scale = 1 })' >/dev/null 2>&1
    dev hyprctl dispatch 'hl.dsp.focus({ monitor = "TESSERA-TEST" })' >/dev/null 2>&1
    inbox 'ipc lock unlock; ipc panels close; ipc island dismissNewest' </dev/null >/dev/null 2>&1; sleep 2
    inbox 'ipc panels settings appearance' </dev/null >/dev/null 2>&1; sleep 3
    # (where the window is: the clicks below assume it centred, 440,205 for Material's 1040x700)
    dev hyprctl clients 2>/dev/null | grep -B2 -A8 -i "settings" | grep -E "at:|size:|class" | head -6
    ;;
open) inbox "ipc panels settings ${1:-appearance}" </dev/null >/dev/null 2>&1 ;;
shot) dev shot "$OUT/$1.png" </dev/null >/dev/null 2>&1; echo "$OUT/$1.png" ;;
clicks)
    # (one vptr call per click: a click from a new virtual pointer without a
    # move first doesn't reach the window)
    name=$1; shift
    region=${REGION:-0,0 1920x1080}
    x=${region%%,*}; y=${region#*,}; y=${y%% *}; wh=${region#* }
    prev="${START:-560 300}"
    # the marker: the pointer jumps into the page at a known time (ties the video's clock to the wall clock)
    body="date +%s.%N > /tmp/s-marker.txt; P move ${MARK:-1100 700}; sleep 1; P move $prev; sleep 0.5;"
    for pt in "$@"; do
        body+="date +%s.%N >> /tmp/s-clicks.txt; P move $prev glide $pt 200 sleep 400 click; sleep 1.0;"
        prev=$pt
    done
    inbox "rm -f /tmp/s-rec.mp4 /tmp/s-clicks.txt; wf-recorder -g \"$((20000 + x)),$((20000 + y)) $wh\" -f /tmp/s-rec.mp4 -r 60 -c libx264 -p crf=18 -p preset=ultrafast >/dev/null 2>&1 & r=\$!; sleep 1; $body kill -INT \$r; wait \$r" </dev/null >/dev/null 2>&1
    dev run cat /tmp/s-rec.mp4 > "$OUT/$name.mp4"
    dev run cat /tmp/s-clicks.txt > "$OUT/$name.clicks"
    dev run cat /tmp/s-marker.txt > "$OUT/$name.marker"
    echo "$OUT/$name.mp4"
    ;;
keys)
    name=$1 n=$2
    region=${REGION:-0,0 1920x1080}
    x=${region%%,*}; y=${region#*,}; y=${y%% *}; wh=${region#* }
    body=""
    for _ in $(seq "$n"); do body+="key -k Down; sleep 1.3;"; done
    inbox "rm -f /tmp/s-rec.mp4; wf-recorder -g \"$((20000 + x)),$((20000 + y)) $wh\" -f /tmp/s-rec.mp4 -r 60 -c libx264 -p crf=18 -p preset=ultrafast >/dev/null 2>&1 & r=\$!; sleep 1; $body kill -INT \$r; wait \$r" </dev/null >/dev/null 2>&1
    dev run cat /tmp/s-rec.mp4 > "$OUT/$name.mp4"
    echo "$OUT/$name.mp4"
    ;;
run) inbox "$1" ;;
log) dev run bash -c "tail -n 80 $LOG" ;;
down) dev down ;;
*) sed -n 2,16p "$0" ;;
esac
