#!/usr/bin/env bash
# shots-rust.sh: the site's screenshots from the Rust shell (tessera-ui), in a
# hidden sandbox arena (site/tools/arena.sh), never your screen: demo data
# only (Maya's earbuds and phone, a made-up battery), 2x (3840 x 2160). Then
# site/tools/crop-shots.py OUT crops them into site/public/media/shots.
#
#   [TESSERA_ARENA=60] site/tools/shots-rust.sh <tessera-ui binary> [OUT_DIR]
#
# After a run, check the host's radios (the last line) and look at every
# shot for a real name before committing a crop.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
BIN=$1 OUT=${2:-/tmp/tessera-shots-rust}
A() { "$HERE/arena.sh" "$BIN" "$OUT" "$@"; }
LEAVES="$HERE/../film/hero/assets/leaves.jpg"

# Glass over the tiles: Quick Settings, the island, the battery's live entry,
# Settings' new pages, the overview, the lock screen's island, Quick Share
A radios up glass wall-tiles.jpg 2 \
    P "move 3830 2150" \
    ipc "panels open controlCenter IRICE-TEST" wait 2.5 shot qs-g ipc "panels close" wait 1 \
    ipc "island demo parts" wait 2.5 shot island-g sh "ipc island dismissNewest" wait 2 \
    ipc "island demo output" wait 0.9 shot hud-g wait 3 \
    cfg power.saver '"low"' \
    bat "60 discharging" wait 3 bat "21 discharging" wait 3 bat "19 discharging" wait 4 \
    bat "12 discharging" wait 3 bat "9 discharging" wait 4 bat "6 discharging" wait 3 bat "4 discharging" wait 4 \
    shot battery-g sh "ipc island dismissNewest" wait 2 \
    ipc "panels open notificationCenter IRICE-TEST" wait 2.5 shot nc-g \
    P "move 3215 330" wait 0.4 P "click" wait 1.5 shot nc-open-g \
    ipc "panels close" wait 1 \
    ipc "panels settings health" wait 3 shot set-health \
    ipc "panels settings power" wait 3 shot set-power \
    ipc "panels settings security" wait 3 shot set-security \
    ipc "panels settings about" wait 3 shot set-about \
    P "move 3830 2150" \
    ipc "panels open overview IRICE-TEST" wait 3 shot overview-g ipc "panels close" wait 1 \
    sh "ipc panels closeSettings; key -k Escape; ipc panels close" wait 1 \
    ipc "lock lock" wait 3 \
    ipc "lock demo face" wait 1.6 ipc "lock demo face-reading" wait 1.2 shot lock-face-g \
    ipc "lock demo finger" wait 1.6 shot lock-finger-g \
    ipc "lock demo match" wait 0.5 shot lock-match-g \
    ipc "lock demo off" ipc "lock unlock" wait 2 \
    ipc "sharingdemo ask" wait 1.5 ipc "sharingdemo progress 46" wait 1.5 shot share-g \
    down

# Material over the tiles: Quick Settings (the split's other half), its lock
A up material wall-tiles.jpg 2 \
    P "move 3830 2150" \
    ipc "panels open controlCenter IRICE-TEST" wait 2.5 shot qs-m ipc "panels close" wait 1 \
    ipc "lock lock" wait 3 ipc "lock demo face" wait 1.6 ipc "lock demo face-reading" wait 1.2 shot lock-face-m \
    ipc "lock demo off" ipc "lock unlock" wait 1 \
    down

# The closing still: the leaves and the earbuds in the island
A up glass "$LEAVES" 2 \
    P "move 3830 2150" ipc "island demo parts" wait 2.5 shot hero \
    down radios
echo "shots in $OUT"
