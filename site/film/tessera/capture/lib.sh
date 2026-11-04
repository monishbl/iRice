#!/usr/bin/env bash
# Shared helpers for the film's capture scripts. Everything happens in one
# hidden, silent sandbox arena (./dev.sh with IRICE_ARENA), never on the
# real screen. Source it: . capture/lib.sh
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
CAP="$ROOT/site/film/tessera/capture"
export IRICE_ARENA="${IRICE_ARENA:-7}"
HOMEDIR="$ROOT/.sandbox/arena-$IRICE_ARENA/home"
[[ $IRICE_ARENA == 1 ]] && HOMEDIR="$ROOT/.sandbox/home"
WALLS="$CAP/seed/photos"
dev() { "$ROOT/dev.sh" "$@"; }

# The prelude every in-sandbox script gets: ipc, the virtual pointer, keys
PRELUDE='
ipc() { QS=qs '"$ROOT"'/shell/assets/scripts/shell-ipc.sh call "$@" >/dev/null 2>&1; }
P() { '"$ROOT"'/.sandbox/bin/vptr -o IRICE-TEST "$@" >/dev/null 2>&1; }
key() { wtype "$@" 2>/dev/null; }
'
inbox() { dev run bash -c "$PRELUDE$1"; }

# setup <glass|material> [wallpaper] [scale 2|1]: the look, seeded, reloaded,
# the test screen at 2x (4K pixels, 1080p layout) or 1x, the island quiet
setup() {
    local look=$1 wall=${2:-$WALLS/wall-sunset-lake.jpg} scale=${3:-2} mode=${4:-}
    # (the arena up first: `up` seeds its home afresh, so the seed comes after)
    # (a fresh arena starts a shell of its own: wait for it, so the reload
    # below replaces it instead of racing it and leaving two)
    dev status | grep -q running || { dev up >/dev/null 2>&1; sleep 12; }
    python3 "$CAP/seed/seed.py" "$HOMEDIR" "$look" "$wall" >/dev/null
    # (the hidden window's output off first: the shell's icon colouring
    # needs it gone when it starts, or Material's dock icons draw blank)
    dev hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null
    dev reload >/dev/null 2>&1
    sleep 18
    [[ -n $mode ]] || { [[ $scale == 2 ]] && mode="3840x2160@60" || mode="1920x1080@60"; }
    dev hyprctl eval "hl.monitor({ output = \"IRICE-TEST\", mode = \"$mode\", position = \"20000x20000\", scale = $scale })" >/dev/null
    dev hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null
    dev hyprctl dispatch 'hl.dsp.focus({ monitor = "IRICE-TEST" })' >/dev/null
    sleep 4
    inbox 'ipc island dismissNewest; ipc island clearHistory; ipc panels close'
    sleep 2
}

# still <name>: the whole test screen, at its pixel size
still() { mkdir -p "$CAP/stills"; dev shot "$CAP/stills/$1.png" >/dev/null; }

# rec <name> <region "x,y wxh" in screen points> <lead s> <tail s> <body>:
# records just that region (near full frame rate, unlike the whole 4K
# screen) while the body runs in the sandbox, into capture/raw/<name>.mp4
rec() {
    local name=$1 region=$2 lead=$3 tail=$4 body=$5 x y wh
    x=${region%%,*}; y=${region#*,}; y=${y%% *}; wh=${region#* }
    mkdir -p "$CAP/raw"
    inbox "rm -f /tmp/film-$name.mp4
wf-recorder -g \"$((20000 + x)),$((20000 + y)) $wh\" -f /tmp/film-$name.mp4 -r 60 -c libx264 -p crf=14 -p preset=fast >/dev/null 2>&1 & r=\$!
sleep $lead
$body
sleep $tail
kill -INT \$r; wait \$r"
    dev run cat "/tmp/film-$name.mp4" >"$CAP/raw/$name.mp4"
    dev run rm -f "/tmp/film-$name.mp4"
    echo "$name: $(ffprobe -v error -select_streams v -show_entries stream=width,height,nb_frames -of csv=p=0 "$CAP/raw/$name.mp4")"
}
