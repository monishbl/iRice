#!/usr/bin/env bash
# Generates the bundled default wallpapers (original artwork, no external assets).
# --only <name> renders just one of them.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/shell/assets/wallpapers"
W=2560 H=1440
mkdir -p "$OUT"

# name base blob1 blob2 blob3 line
_make() {
    local name="$1" base="$2" c1="$3" c2="$4" c3="$5" line="$6"
    magick -size ${W}x${H} "xc:$base" \
        \( -size ${W}x${H} xc:none -fill "$c1" -draw "ellipse 600,1100 900,520 0,360" \) -composite \
        \( -size ${W}x${H} xc:none -fill "$c2" -draw "ellipse 1900,380 820,560 0,360" \) -composite \
        \( -size ${W}x${H} xc:none -fill "$c3" -draw "ellipse 1500,1250 700,380 0,360" \) -composite \
        -blur 0x180 \
        \( -size ${W}x${H} xc:none -stroke "$line" -strokewidth 6 -fill none -draw "bezier -100,1300 700,1250 1500,500 2700,-100" -blur 0x3 \) -composite \
        \( -size ${W}x${H} xc:none -stroke "$line" -strokewidth 40 -fill none -draw "bezier -100,1300 700,1250 1500,500 2700,-100" -blur 0x60 \) -compose screen -composite \
        -attenuate 0.25 +noise Gaussian -quality 92 "$OUT/$name.jpg"
    echo "wrote $OUT/$name.jpg"
}
[[ "${1:-}" == "--only" ]] && ONLY="$2" || ONLY=""
make() {
    [[ -n "$ONLY" && "$1" != "$ONLY" ]] && return 0
    _make "$@"
}
make tahoe-dusk "#0B1026" "#1F4FD8" "#7B3FE4" "#0EA5B7" "#BFE3FF"
make tahoe-day  "#C9D8F2" "#5B8CFF" "#F5A3C7" "#7FD6E8" "#FFFFFF"
# Dynamic set (with dusk and day): sunrise and late night
make tahoe-dawn  "#2A1B3D" "#F2994A" "#EB5757" "#6C5CE7" "#FFE3C2"
make tahoe-night "#05060F" "#14286B" "#3B1F6E" "#0B4F5C" "#7FA7FF"
