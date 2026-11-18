#!/usr/bin/env bash
# arena.sh: the site's shots and the film's takes from the Rust shell
# (tessera-ui, standalone), in a hidden sandbox arena: never your screen,
# demo data only (Maya's placeholder account, the sandbox shims keep the
# real Wi-Fi and Bluetooth untouched). A list of steps, run in order:
#
#   TESSERA_ARENA=60 site/tools/arena.sh <tessera-ui binary> <out dir> <step…>
#
#   up <glass|material> <wallpaper> [1|2] [dark|light]   a fresh arena home, the
#                                     binary installed, seeded, the test screen
#                                     at 1x (1920x1080) or 2x (3840x2160)
#   shot NAME            the whole test screen → OUT/NAME.png
#   ipc "ARGS"           tessera-ui ipc call ARGS
#   P "ARGS"             the virtual pointer (physical pixels)
#   key "ARGS"           wtype ARGS
#   sh "COMMANDS"        commands inside the sandbox (ipc, P and key defined)
#   cfg KEY JSON         one setting (the shell follows the file)
#   wait S               sleep S seconds
#   rec NAME "x,y wxh" LEAD TAIL "BODY"   a recording → OUT/NAME.mp4
#   bat "PCT STATE"      the arena's made-up battery ("15 discharging")
#   glassfake KIND       the glass plugin's state as Settings shows it (updated …)
#   radios               the host's Wi-Fi and Bluetooth power, to compare
#   down                 the arena stopped
#
# Wallpapers: a path, or a name in site/film/tessera/capture/seed/photos.
set -uo pipefail
BIN=$(realpath "$1") OUT=$(realpath -m "$2"); shift 2
HERE="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$HERE"
export TESSERA_ARENA=${TESSERA_ARENA:-60} IRICE_ARENA=${TESSERA_ARENA:-60} TESSERA_NO_BOX=1
. site/film/tessera/capture/lib.sh
mkdir -p "$OUT"
RT=/tmp/irice-rt-$TESSERA_ARENA
# a battery of its own (TESSERA_FAKE_BATTERY: the sandbox only), "bat" sets it
export TESSERA_FAKE_BATTERY="$OUT/battery"
echo "80 charging" >"$TESSERA_FAKE_BATTERY"
CFG="$HOMEDIR/.config/tessera/config.json"
cfg() { python3 -I - "$CFG" "$1" "$2" <<'PY'
import json, sys
p, k, v = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
c = json.load(open(p))
d = c
ks = k.split(".")
for x in ks[:-1]:
    d = d.setdefault(x, {})
d[ks[-1]] = v
json.dump(c, open(p, "w"), indent=2)
PY
}
radios() { echo "wifi=$(nmcli -t -f WIFI general 2>/dev/null) bt=$(bluetoothctl show 2>/dev/null | grep -m1 Powered | xargs)"; }
while (($#)); do
    step=$1; shift
    case $step in
    up)
        look=$1 wall=$2 scale=2 mode=dark; shift 2
        [[ $# -gt 0 && $1 =~ ^[12]$ ]] && { scale=$1; shift; }
        [[ $# -gt 0 && $1 =~ ^(dark|light)$ ]] && { mode=$1; shift; }
        [[ -f $wall ]] || wall="$WALLS/$wall"
        dev down >/dev/null 2>&1
        case "$HOMEDIR" in */.sandbox/arena-"$TESSERA_ARENA"/home) rm -rf "${HOMEDIR%/home}" ;; *) echo "odd home $HOMEDIR"; exit 2 ;; esac
        mkdir -p "$HOMEDIR/.config/tessera" "$HOMEDIR/.local/state/tessera"
        python3 -I tools/gate/config.py "$CFG" "$wall" >/dev/null
        install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
        dev up >/dev/null 2>&1 || { echo "arena didn't start"; exit 2; }
        sleep 6
        MODE=$mode ACCENT=${ACCENT:-blue} setup "$look" "$wall" "$scale" >/dev/null 2>&1
        python3 -I tools/gate/config.py "$CFG" "$wall" >/dev/null
        cfg appearance.look "\"$look\""
        cfg appearance.mode "\"$mode\""
        cfg appearance.accent "\"${ACCENT:-blue}\""
        cfg appearance.menuIcon '"circle"'
        cfg island.events '["timers", "music", "devices", "sharing", "notifications"]'
        inbox 'ipc lock unlock; ipc panels close; ipc island clearHistory' >/dev/null 2>&1
        sleep 3
        echo "up: $look $(basename "$wall") ${scale}x $mode"
        ;;
    shot) dev shot "$OUT/$1.png" >/dev/null 2>&1 && echo "shot $1"; shift ;;
    ipc) inbox "ipc $1" >/dev/null 2>&1; shift ;;
    P) inbox "P $1" >/dev/null 2>&1; shift ;;
    key) inbox "key $1" >/dev/null 2>&1; shift ;;
    sh) inbox "$1" >/dev/null 2>&1; shift ;;
    cfg) cfg "$1" "$2"; shift 2 ;;
    wait) sleep "$1"; shift ;;
    rec)
        rec "$1" "$2" "$3" "$4" "$5" >/dev/null
        { [[ "$CAP/raw/$1.mp4" -ef "$OUT/$1.mp4" ]] || mv -f "$CAP/raw/$1.mp4" "$OUT/$1.mp4"; } && echo "rec $1: $(ffprobe -v error -select_streams v -show_entries stream=width,height,nb_frames -of csv=p=0 "$OUT/$1.mp4")"
        shift 5
        ;;
    log) cp -f "$HOMEDIR/.local/state/tessera/tessera-ui.log" "$OUT/tessera-ui.log" 2>/dev/null; echo "log" ;;
    bat) echo "$1" >"$TESSERA_FAKE_BATTERY"; shift ;;
    glassfake) echo "$1" >"$RT/tessera-fake-glass"; shift ;;
    radios) echo "$(radios)" ;;
    down) dev down >/dev/null 2>&1; echo down ;;
    *) echo "no step $step"; exit 2 ;;
    esac
done
