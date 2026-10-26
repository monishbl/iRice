#!/usr/bin/env bash
# Downloads macOS wallpapers (Tahoe back to Mojave) from the community
# collection github.com/LAYTAT/macOS-Wallpapers into ~/Pictures/Wallpapers/macOS,
# as screen-sized JPEGs. "Name Light" / "Name Dark" pairs switch with light
# and dark mode; the Tahoe Beach folder follows the time of day. The pictures
# are Apple's and aren't part of iRice. Needs curl and ImageMagick.
#   tools/get-wallpapers.sh [--size 2560x1440]
set -uo pipefail
SIZE=1920x1080
[[ "${1:-}" == "--size" && -n "${2:-}" ]] && SIZE="$2"
DEST="$(xdg-user-dir PICTURES 2>/dev/null || echo "$HOME/Pictures")/Wallpapers/macOS"
command -v magick >/dev/null || { echo "Needs ImageMagick (magick)"; exit 1; }
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$DEST/Tahoe Beach"
BASE="https://raw.githubusercontent.com/LAYTAT/macOS-Wallpapers/main"
# source name | destination (relative to DEST)
while IFS='|' read -r src dst; do
    [[ -z "$src" ]] && continue
    out="$DEST/$dst.jpg"
    [[ -s "$out" ]] && { echo "have $dst"; continue; }
    url="$BASE/${src// /%20}"
    if curl -fsSL --retry 3 -o "$TMP/in" "$url"; then
        magick "$TMP/in" -resize "${SIZE}^" -quality 92 -sampling-factor 4:2:0 -strip "$out" && echo "ok   $dst ($(du -h "$out" | cut -f1))"
    else
        echo "FAIL $src"
        failed=1
    fi
    rm -f "$TMP/in"
done <<'LIST'
26-Tahoe-Beach-Dawn.png|Tahoe Beach/1 Dawn
26-Tahoe-Beach-Day.png|Tahoe Beach/2 Day
26-Tahoe-Beach-Dusk.png|Tahoe Beach/3 Dusk
26-Tahoe-Beach-Night.png|Tahoe Beach/4 Night
26-Tahoe-Light-6K.png|Tahoe Light
26-Tahoe-Dark-6K.png|Tahoe Dark
15-Sequoia-Light-6K.jpg|Sequoia Light
15-Sequoia-Dark-6K.jpg|Sequoia Dark
14-Sonoma-Light.jpg|Sonoma Light
14-Sonoma-Dark.jpg|Sonoma Dark
13-Ventura-Light.jpg|Ventura Light
13-Ventura-Dark.jpg|Ventura Dark
12-Light.jpg|Monterey Light
12-Dark.jpg|Monterey Dark
11-0-Day.jpg|Big Sur Light
11-0-Night.jpg|Big Sur Dark
10-15-Day.jpg|Catalina Light
10-15-Night.jpg|Catalina Dark
10-14-Day.jpg|Mojave Light
10-14-Night.jpg|Mojave Dark
LIST
[[ ${failed:-0} == 1 ]] && exit 1
echo "Wallpapers are in $DEST"
