#!/usr/bin/env bash
# updater-layout.sh: Settings → Software Update with long real values (a long
# commit message, a channel-mismatch line, "Built locally from the branch …",
# an update running, a build in progress and failed, 40 packages), in a hidden
# arena, shot from top to bottom, to check that no row's text runs under its
# buttons. Never the real screen; shots are for looking at, never committed.
#   ARENA=N tools/updater-layout.sh <tessera-ui binary> <out dir> <glass|material> [screen width]
# A screen narrower than the window (860 / 1040 px) narrows Settings with it
# (fit_window): 700 checks a narrow window. Scenarios: tools/updater-layout-feed.py.
# The Settings shown is a second tessera-ui (--surfaces settings) fed on
# stdin, so the arena's own shell can't overwrite the feed.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN=$(realpath "$1") OUT=$2 LOOK=$3 W=${4:-1920}
export TESSERA_NO_BOX=${TESSERA_NO_BOX:-1} TESSERA_ARENA=${ARENA:-20}
. "$ROOT/site/film/tessera/capture/lib.sh"
. "$ROOT/sandbox/lib.sh"
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
CFG="$HOMEDIR/.config/tessera/config.json"
radios() { echo "wifi=$(nmcli -t -f WIFI general 2>/dev/null) bt=$(bluetoothctl show 2>/dev/null | grep -m1 Powered | xargs)"; }

# ── A fresh arena with the gate's settings, the look asked for ──
dev down >/dev/null 2>&1
case "$HOMEDIR" in */.sandbox/arena-"$TESSERA_ARENA"/home) rm -rf "${HOMEDIR%/home}" ;; *) echo "odd arena home $HOMEDIR"; exit 2 ;; esac
mkdir -p "$HOMEDIR/.config/tessera" "$HOMEDIR/.local/state/tessera"
python3 -I "$ROOT/tools/gate/config.py" "$CFG" "$WALLS/wall-tiles.jpg"
install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
dev up >/dev/null 2>&1 || { echo "the arena didn't start"; exit 2; }
sleep 4
setup "$LOOK" "$WALLS/wall-tiles.jpg" 1 >/dev/null 2>&1
dev hyprctl eval "hl.monitor({ output = \"TESSERA-TEST\", mode = \"${W}x1080@60\", position = \"20000x20000\", scale = 1 })" >/dev/null 2>&1
dev hyprctl dispatch 'hl.dsp.focus({ monitor = "TESSERA-TEST" })' >/dev/null 2>&1
inbox 'ipc lock unlock; ipc panels close' >/dev/null 2>&1

# ── Settings of its own, fed the scenarios ──
F=$TESSERA_RT/updater-layout.fifo
rm -f "$F"; mkfifo "$F"
dev run bash -c "exec 3<>$F; TESSERA_STANDALONE=0 '$BIN' --surfaces settings <&3 >/dev/null 2>$TESSERA_RT/updater-layout.log & echo \$! > $TESSERA_RT/updater-layout.pid" >/dev/null 2>&1
sleep 3
exec 4>"$F"
echo '{"cmd":"open","surface":"settings","screen":"TESSERA-TEST","page":"update"}' >&4
X=$((W / 2 + 100))
for sc in plan running build failed; do
    python3 -I "$ROOT/tools/updater-layout-feed.py" "$sc" >&4
    sleep 3
    dev shot "$OUT/$LOOK-$W-$sc-0.png" >/dev/null 2>&1
    for k in 1 2 3 4 5 6; do
        inbox "P move $X 600 sleep 100 swipe 420 300 sleep 600" >/dev/null 2>&1; sleep 1.2
        dev shot "$OUT/$LOOK-$W-$sc-$k.png" >/dev/null 2>&1
    done
    inbox "P move $X 600 sleep 100 swipe -6000 300 sleep 400 swipe -6000 300" >/dev/null 2>&1; sleep 1
done
kill "$(cat "$TESSERA_RT/updater-layout.pid")" 2>/dev/null
exec 4>&-
dev down >/dev/null 2>&1
echo "shots in $OUT; $(radios)"
