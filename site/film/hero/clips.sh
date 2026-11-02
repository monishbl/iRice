#!/usr/bin/env bash
# The film's footage: real iRice, recorded in the dev sandbox (a 3840x2400
# output at 2x, the leaves wallpaper, the blue accent) with wf-recorder while
# `./dev.sh ipc island demo …` and `sharingdemo …` played the events. Only a
# region was recorded each time (`wf-recorder -g`: a whole 4K output drops
# to about 6 fps), on top of assets/screen.jpg, a still of the whole screen:
#   charge, note, wifi, vol, music   the island: the top centre (device px
#                                    1280-2560 x 0-720), at 1920x1080
#   fastpair                         the same region, at 1280x720
#   card                             the centre (1280-2560 x 800-1600), 1280x800
# The film shows them a frame at a time (a seek has to be exact), so this
# unpacks each into frames/<clip>/0001.jpg … (not committed). Run it once
# before previewing or rendering. (The recordings are limited-range video;
# the frames are expanded to full range, to match the still.)
set -euo pipefail
cd "$(dirname "$0")"
for c in clips/*.mp4; do
	n=$(basename "$c" .mp4)
	rm -rf "frames/$n" && mkdir -p "frames/$n"
	ffmpeg -v error -i "$c" -vf "format=rgb24,lutrgb=r='clip((val-16)*255/219,0,255)':g='clip((val-16)*255/219,0,255)':b='clip((val-16)*255/219,0,255)'" -q:v 2 "frames/$n/%04d.jpg"
	echo "$n: $(ls "frames/$n" | wc -l) frames"
done
