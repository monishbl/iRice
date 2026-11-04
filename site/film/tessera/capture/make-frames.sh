#!/usr/bin/env bash
# make-frames.sh: the film's takes (capture.sh calm) → frames/<clip>/0001.jpg …
# at 30 fps, as the site film plays them (a frame at a time, so a seek is
# exact), levels expanded from the recorder's limited range to full, to match
# the stills. Sizes match the original film's clips where a take replaces one
# (note at 1920x1080; fastpair 1280x720; card 1280x800). And the plates.
set -euo pipefail
cd "$(dirname "$0")"
EXPAND="lutrgb=r='clip((val-16)*255/219,0,255)':g='clip((val-16)*255/219,0,255)':b='clip((val-16)*255/219,0,255)'"
for n in ${ONLY:-note fastpair card circle mqs spotask lockseq glassramp vol}; do
    # (the take as recorded when it's here, else its committed copy in ../clips)
    src=raw/$n.mp4
    [[ -f $src ]] || src=../clips/$n.mp4
    [[ -f $src ]] || { echo "missing $n"; continue; }
    case $n in
        note | vol) scale="scale=1920:1080:flags=lanczos," ;;
        *) scale="" ;;
    esac
    rm -rf "../frames/$n" && mkdir -p "../frames/$n"
    ffmpeg -v error -i "$src" -vf "fps=30,${scale}format=rgb24,$EXPAND" -q:v 3 "../frames/$n/%04d.jpg"
    echo "$n: $(ls "../frames/$n" | wc -l) frames, $(magick identify -format '%wx%h' "../frames/$n/0001.jpg")"
done
mkdir -p ../plates
for s in calm-g-desktop1 calm-g-desktop calm-m-desktop; do
    [[ -f stills/$s.png ]] && magick "stills/$s.png" -resize 1920x -quality 90 "../plates/$s.jpg"
done
ls ../plates
