#!/usr/bin/env bash
# film-clips.sh: the site's feature loops, cut from the Tessera film
# (site/film/tessera/film.html) without its chapter titles (the tiles have
# their own), and its hero video and poster.
#
#   [KIT=~/Projects/product-film-kit] [SHUTTER=1] site/tools/film-clips.sh [name …]
#   (SHUTTER: motion-blur sub-frames; 1, none, is plenty at tile size;
#   progress in site/film/tessera/clips-progress.json)
#
# Each clip is rendered frame-exact by the product-film kit's render.mjs
# (its frames must be unpacked first: site/film/tessera/capture/make-frames.sh),
# then encoded for the web at 1280 x 720 with a poster from its middle. The
# hero is the whole film (site/film/tessera/tessera.mp4, rendered by the kit)
# at about 1 Mbit/s.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
KIT="${KIT:-$HOME/Projects/product-film-kit}"
OUT="$HERE/public/media/clips"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$OUT"

# name: film seconds from and to (inside a chapter, clear of its warps)
declare -A CLIPS=(
    [new-earbuds]="12.2 20.2"
    [new-ask]="37.5 41.5"
    [new-circle]="41.9 46.2"
    [new-text]="46.35 49.45"
    [new-captions]="50.6 56.5"
    [new-unlock]="57.65 62.3"
    [new-glass]="62.6 65.4"
    [new-colours]="69.15 71.25"
)
names=("$@")
[[ ${#names[@]} -gt 0 ]] || names=("${!CLIPS[@]}")
for n in "${names[@]}"; do
    read -r from to <<<"${CLIPS[$n]}"
    echo "$n ($from–$to s)"
    node "$KIT/tools/render.mjs" tessera/film.html --root "$HERE/film" --from "$from" --to "$to" --fps 30 --shutter "${SHUTTER:-1}" --query notitles=1 --out "$TMP/$n.mp4" --progress "$HERE/film/tessera/clips-progress.json" >/dev/null
    ffmpeg -v error -y -i "$TMP/$n.mp4" -an -vf "scale=1280:-2:flags=lanczos,format=yuv420p" -c:v libx264 -preset slow -crf 26 -movflags +faststart "$OUT/$n.mp4"
    mid=$(python3 -c "print(($to - $from) * 0.6)")
    ffmpeg -v error -y -ss "$mid" -i "$OUT/$n.mp4" -frames:v 1 -q:v 4 "$OUT/$n.jpg"
    echo "  $(du -h "$OUT/$n.mp4" | cut -f1)"
done

# The hero: the film, about 1 Mbit/s, and a poster on its last image (the Badge)
FILM="$HERE/film/tessera/tessera.mp4"
if [[ -f "$FILM" && $# -eq 0 ]]; then
    echo "hero"
    ffmpeg -v error -y -i "$FILM" -an -c:v libx264 -preset slow -crf 28 -maxrate 1400k -bufsize 2800k -pix_fmt yuv420p -movflags +faststart "$HERE/public/media/hero.mp4"
    ffmpeg -v error -y -ss 74.4 -i "$FILM" -frames:v 1 -q:v 3 "$HERE/public/media/hero.jpg"
    echo "  $(du -h "$HERE/public/media/hero.mp4" | cut -f1)"
fi
