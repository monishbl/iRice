#!/usr/bin/env bash
# updater-layout.sh: Settings → Software Update in each of its states (up
# to date, updates ready, running with progress, ready to restart, an error,
# an update running elsewhere, Options, the log sheet), in a hidden arena, a
# shot of each, to check them against the design rules. Never the real
# screen; shots are for looking at, never committed.
#   [MODE=light|dark] ARENA=N tools/updater-layout.sh <tessera-ui binary> <out dir> <glass|material> [screen width]
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
M=${MODE:-dark}
for sc in uptodate ready running restart error busy options sheet; do
    page=update
    [[ $sc == options ]] && page=update/options
    python3 -I "$ROOT/tools/updater-layout-feed.py" "$sc" >&4
    echo "{\"cmd\":\"page\",\"surface\":\"settings\",\"page\":\"$page\"}" >&4
    sleep 3
    dev shot "$OUT/$LOOK-$M-$W-$sc.png" >/dev/null 2>&1
    # (Options is longer than the window: its end too)
    if [[ $sc == options ]]; then
        inbox "P move $X 600 sleep 100 swipe 900 300 sleep 600" >/dev/null 2>&1; sleep 1.2
        dev shot "$OUT/$LOOK-$M-$W-$sc-end.png" >/dev/null 2>&1
        inbox "P move $X 600 sleep 100 swipe -6000 300 sleep 400" >/dev/null 2>&1; sleep 1
    fi
done
kill "$(cat "$TESSERA_RT/updater-layout.pid")" 2>/dev/null
exec 4>&-
dev down >/dev/null 2>&1
echo "shots in $OUT; $(radios)"
