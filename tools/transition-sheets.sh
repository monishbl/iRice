#!/usr/bin/env bash
# transition-sheets.sh: every panel's opening and closing recorded in a hidden
# sandbox arena, each recording cut into a contact sheet of its frames (every
# frame the recorder caught, its time after the trigger written under it), to
# look for half-drawn states: glass without its content, content without its
# glass, shapes left behind, jumps.
#   tools/transition-sheets.sh --side qml|rust --arena N --look glass|material --out DIR
#                              [--code DIR] [--bin TESSERA_UI] [--only 'a|b'] [--env K=V]…
# Never your screen: the arena's IRICE-TEST only.
set -uo pipefail
TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SIDE="" ARENA="" LOOK=glass OUT="" CODE="" BIN="" ONLY="" KEEPUP="" ENVS=()
USERCFG="$HOME/iRice/.sandbox/home/.config/tessera/config.json"
while (($#)); do case "$1" in
    --side) SIDE=$2; shift;; --arena) ARENA=$2; shift;; --look) LOOK=$2; shift;; --out) OUT=$2; shift;;
    --code) CODE=$2; shift;; --bin) BIN=$2; shift;; --only) ONLY=$2; shift;; --keep-up) KEEPUP=1;; --env) ENVS+=("$2"); shift;;
    *) echo "unknown option $1"; exit 2;; esac; shift; done
[[ -n $SIDE && -n $ARENA && -n $OUT && $ARENA != 1 ]] || { sed -n 2,8p "$0"; exit 2; }
CODE=${CODE:-$(cd "$TOOLS/.." && pwd)}
export IRICE_ARENA=$ARENA
# shellcheck source=/dev/null
. "$CODE/site/film/tessera/capture/lib.sh"
CFG="$HOMEDIR/.config/tessera/config.json"
mkdir -p "$OUT"
TAG="$SIDE-$LOOK"
[[ -n $KEEPUP ]] || trap 'dev down >/dev/null 2>&1' EXIT
PANELS=(
    "settings|sc settings|sc settings"
    "controlCenter|sc controlCenter|key -k Escape"
    "notificationCenter|sc notificationCenter|key -k Escape"
    "launchpad|sc launchpad|key -k Escape"
    "spotlight|sc spotlight|key -k Escape"
    "overview|sc overview|key -k Escape"
    "missionControl|sc missionControl|key -k Escape"
    "cheatsheet|sc cheatsheet|key -k Escape"
    "clipboard|sc clipboard|key -k Escape"
    "emoji|sc emoji|key -k Escape"
    "session|sc session|key -k Escape"
    "sidebar|sc sidebar|key -k Escape"
    "wallpaper|sc wallpaper|key -k Escape"
    "soundMenu|ipc panels open soundMenu IRICE-TEST|key -k Escape"
    "systemMenu|ipc panels open systemMenu IRICE-TEST|key -k Escape"
    "island|ipc island post bell Speed test|ipc island dismissNewest"
    "islandHover|P glide 960 14 200|P glide 1500 700 200"
)
cmd_of() { [[ $1 == sc\ * ]] && echo "hyprctl dispatch 'hl.dsp.global(\"tessera:${1#sc }\")' >/dev/null" || echo "$1"; }
CLEAN='ipc panels close; ipc island dismissNewest; ipc island dismissNewest; P move 1500 700'
dev down </dev/null >/dev/null 2>&1
[[ $HOMEDIR == */.sandbox/arena-[2-9]/home && -d $HOMEDIR ]] && rm -rf "$HOMEDIR"
setup material "$WALLS/wall-tiles.jpg" 1 >/dev/null 2>&1
python3 -I - "$USERCFG" "$CFG" "$LOOK" "$SIDE" <<'PY'
import json, sys
src, dst, look, side = sys.argv[1:]
c = json.load(open(src))
c.setdefault("appearance", {})["look"] = look
r = c.setdefault("rust", {}); r["standalone"] = side == "rust"; r["surfaces"] = []
for k in ("ai", "google", "cloud", "phone"):
    c.pop(k, None)
json.dump(c, open(dst, "w"), indent=4)
PY
if [[ $SIDE == rust ]]; then
    if ((${#ENVS[@]})); then
        # (a wrapper that sets them and becomes the binary, still named tessera-ui)
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
# rec name body: 3 s from the trigger, the recorder started 1.2 s before (an encoder behind a busy screen loses its last second)
rec() {
    local n=$1 body=$2
    inbox "rm -f /tmp/ts-$ARENA-$n.mp4
wf-recorder -o IRICE-TEST -r 60 -c libx264 -p preset=ultrafast -p crf=18 -f /tmp/ts-$ARENA-$n.mp4 >/dev/null 2>&1 & r=\$!
sleep 1.2; echo \$(date +%s.%N) >/tmp/ts-$ARENA-$n.t; $body
sleep 3.0; kill -INT \$r; wait \$r 2>/dev/null; cp /tmp/ts-$ARENA-$n.mp4 '$OUT/$n.mp4'; rm -f /tmp/ts-$ARENA-$n.mp4" </dev/null >/dev/null 2>&1
}
for e in "${PANELS[@]}"; do
    IFS='|' read -r name how closeby <<<"$e"
    [[ -n $ONLY && ! $name =~ ^($ONLY)$ ]] && continue
    # (a first open, made ahead or not; then a second, the usual one)
    for k in 1 2; do
        rec "$TAG-$name-open$k" "$(cmd_of "$how")"
        rec "$TAG-$name-close$k" "$(cmd_of "$closeby")"
        inbox "$CLEAN" </dev/null >/dev/null 2>&1; sleep 1
    done
    for k in 1 2; do
        for ph in open close; do
            v="$OUT/$TAG-$name-$ph$k.mp4"
            [[ -s $v ]] || continue
            # the first 48 frames the recorder caught (it records only when the screen changes: time 0 is the first change), 6 across, time stamped
            ffmpeg -v error -y -i "$v" -vf "scale=480:-1,drawtext=text='%{pts\\:flt}':x=6:y=6:fontsize=20:fontcolor=yellow:box=1:boxcolor=black@0.6,tile=6x8" \
                -fps_mode passthrough -frames:v 1 "$OUT/$TAG-$name-$ph$k.png" </dev/null
        done
    done
    echo "[$(date +%T)] $name" | tee -a "$OUT/run-$TAG.log"
done
