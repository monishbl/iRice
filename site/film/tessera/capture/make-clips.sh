#!/usr/bin/env bash
# make-clips.sh: raw takes → web clips the films play (H.264, yuv420p, tv range,
# faststart; the large ones scaled to the size the films show them), stills →
# JPEG plates, and clips.js (paths, regions). Re-run after capture.sh.
set -euo pipefail
cd "$(dirname "$0")"
OUT=../clips
mkdir -p "$OUT" ../plates
for f in raw/*.mp4; do
    n=$(basename "$f" .mp4)
    w=$(ffprobe -v error -select_streams v -show_entries stream=width -of csv=p=0 "$f")
    case $n in
        *livetext) vf="scale=1400:-2" ;;   # (shown at ≤ 1400 px)
        full-*) vf="scale=1920:-2" ;;
        *) vf="scale=$w:-2" ;;
    esac
    ffmpeg -v error -y -i "$f" -an -vf "$vf,format=yuv420p" -c:v libx264 -preset slow -crf 20 -color_range tv -colorspace bt709 -color_primaries bt709 -color_trc bt709 -movflags +faststart "$OUT/$n.mp4"
done
for s in stills/*-desktop.png; do
    n=$(basename "$s" .png)
    magick "$s" -quality 88 "../plates/$n.jpg"
done
ls -la "$OUT" ../plates | awk 'NF > 5 { printf "%7.1f MB  %s\n", $5 / 1048576, $NF }'
