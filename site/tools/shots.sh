#!/usr/bin/env bash
# shots.sh: the site's screenshots, taken in the hidden dev sandbox (never
# your screen): 2x (3840 x 2160), Quick Settings the same moment in both
# looks so the page can split them, then cropped by crop-shots.py.
#
#   IRICE_ARENA=7 [LOOKS='glass material'] [SEARCH=1] [EXTRAS=1] site/tools/shots.sh [OUT_DIR]
#                                     (default /tmp/tessera-shots; EXTRAS=0 skips
#                                     the closing still)
#   site/tools/crop-shots.py OUT_DIR
#
# Uses the film's capture helpers (site/film/tessera/capture/lib.sh: an arena
# brought up and seeded with public placeholder names, the Badge, a canned AI
# model).
# Names: <shot>-g / <shot>-m for the two looks.
set -uo pipefail
cd "$(dirname "$0")/../film/tessera/capture"
. ./lib.sh
OUT="${1:-/tmp/tessera-shots}"
mkdir -p "$OUT"
TILES="$CAP/seed/photos/wall-tiles.jpg"
LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
snap() { dev shot "$OUT/$1.png" >/dev/null && echo "  $1"; }

# Each look, over the same tiles: Quick Settings (the page splits these),
# the island's earbuds and a Quick Share transfer (the page uses Glass's)
for look in ${LOOKS:-glass material}; do
    l=${look:0:1}
    echo "$look"
    ACCENT=blue setup "$look" "$TILES" 2 "3840x2160@60"
    inbox 'P move 3830 2150'
    inbox 'ipc panels open controlCenter IRICE-TEST'; sleep 2.5; snap "qs-$l"
    inbox 'ipc panels close'; sleep 1
    inbox 'ipc island demo parts'; sleep 2.5; snap "island-$l"
    inbox 'ipc island dismissNewest'; sleep 1.5
    inbox 'ipc sharingdemo ask'; sleep 1.5
    inbox 'ipc sharingdemo progress 46'; sleep 1.5; snap "share-$l"
    inbox 'ipc sharingdemo done; ipc panels close'; sleep 2
done

# Search's answers over the white flower, and Software (SEARCH=0 skips them)
if [[ ${SEARCH:-1} == 1 ]]; then
    echo search
    ACCENT=blue setup glass "$ROOT/site/tools/wallpapers/photo-flower.jpg" 2 "3840x2160@60"
    inbox 'P move 3830 2150'
    for q in "spotlight-flower:18% of 2450" "spotlight-days:days until christmas" "spotlight-time:time in Tokyo"; do
        inbox 'ipc panels open spotlight IRICE-TEST'; sleep 1
        inbox "key \"${q#*:}\""; sleep 2; snap "${q%%:*}"
        inbox 'ipc panels close'; sleep 1
    done
    inbox 'ipc panels appStore'; sleep 6; snap g-appstore
    inbox 'key -k Escape; ipc panels close'; sleep 1
fi

[[ ${EXTRAS:-1} == 1 ]] || { dev down >/dev/null 2>&1; echo "shots in $OUT"; exit 0; }

# The closing still: the leaves and the island's earbuds (the island steps
# aside while Quick Settings is open, so not both)
ACCENT=blue setup glass "$LEAVES" 2 "3840x2160@60"
inbox 'P move 3830 2150; ipc island demo parts'; sleep 2.5; snap hero
dev down >/dev/null 2>&1
echo "shots in $OUT"
