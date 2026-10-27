#!/usr/bin/env bash
# Directional display sounds (Settings → Displays → Directional sound): for
# each sound style, the display connect/disconnect sounds panned toward the
# side the display is on. Connecting arrives from that side and settles near
# the centre; disconnecting leaves toward it. Equal-power panning, so it's as
# loud wherever it is. Needs ffmpeg (with libvorbis); run after make-sounds.py.
#   display-connect-left.ogg  display-connect-right.ogg
#   display-disconnect-left.ogg  display-disconnect-right.ogg
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOUNDS="$ROOT/shell/assets/sounds"

# pos: -1 left .. 1 right, moving from $1 to $2 over the first 0.35 s
pan() {
    local from=$1 to=$2
    local p="($from+($to-($from))*min(t/0.35\\,1))"
    local m="(val(0)+val(1))/2*1.2"
    echo "aeval=$m*cos(($p+1)*PI/4)|$m*sin(($p+1)*PI/4):c=stereo"
}

for dir in "$SOUNDS"/*/; do
    for kind in connect disconnect; do
        src="$dir/display-$kind.ogg"
        [[ -f "$src" ]] || continue
        for side in left right; do
            s=1
            [[ $side == left ]] && s=-1
            if [[ $kind == connect ]]; then
                from=$(echo "0.85*$s" | bc -l) to=$(echo "0.25*$s" | bc -l)
            else
                from=$(echo "0.25*$s" | bc -l) to=$(echo "0.85*$s" | bc -l)
            fi
            ffmpeg -loglevel error -y -i "$src" -af "$(pan "$from" "$to")" -c:a libvorbis -q:a 3 "$dir/display-$kind-$side.ogg"
        done
    done
    echo "made ${dir#"$ROOT"/}display-*-{left,right}.ogg"
done
