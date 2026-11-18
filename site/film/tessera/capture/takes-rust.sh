#!/usr/bin/env bash
# takes-rust.sh: the film's takes again from the Rust shell (tessera-ui), for
# the parts of it that changed (18 Nov): the island's notification card and
# its volume row, Material's Quick Settings, and the lock screen with the
# Halo island (the face, then the finger, a match). Same framing and timing
# as capture.sh's islands / fix9 / mat / fix8, in a hidden arena
# (site/tools/arena.sh), into capture/raw/; then make-frames.sh.
#   [TESSERA_ARENA=60] capture/takes-rust.sh <tessera-ui binary>
set -uo pipefail
CAPD="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$CAPD/../../../.." && pwd)"
BIN=$1 RAW="$CAPD/raw"
A() { "$ROOT/site/tools/arena.sh" "$BIN" "$RAW" "$@"; }
LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
OCEAN="$ROOT/site/film/hero/assets/p-ocean.jpg"

# The island up close (2x, the takes' region: 1280 x 720 px)
A radios up glass "$LEAVES" 2 \
    P "move 3830 2150" \
    rec note "640,0 640x360" 0.5 3.5 'ipc island demo notification' \
    wait 6 \
    rec vol "640,0 640x360" 0.1 1.2 'ipc island demo output; sleep 0.5; for i in 1 2 3 4; do wpctl set-volume @DEFAULT_AUDIO_SINK@ 6%+; sleep 0.18; done; sleep 1' \
    down

# The lock screen (1x): Bedside Mode, a slide back, someone there (the lock
# screen), the camera looking, the reader asking, the match, unlocked
A up glass "$LEAVES" 1 \
    P "move 1915 1075" \
    cfg lock.standby true wait 1 ipc "lock lock" wait 2.5 ipc "lock demo standby" wait 2.5 key "-k Right" wait 2.5 \
    rec lockseq "0,0 1920x1080" 0.3 2.0 'sleep 1.4; key -k Left; sleep 1.6; ipc lock demo wake; sleep 1.5; ipc lock demo face; sleep 1.2; ipc lock demo finger; sleep 1.3; ipc lock demo match; sleep 0.9; ipc lock unlock; sleep 3' \
    ipc "lock demo off" \
    down

# Material's Quick Settings springing open (1x, over the ocean)
A up material "$OCEAN" 1 \
    P "move 1915 1075" \
    rec mqs "0,0 1920x1080" 0.4 1.2 'ipc panels open controlCenter IRICE-TEST; sleep 2' \
    ipc "panels close" \
    down radios
