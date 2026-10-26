#!/usr/bin/env bash
# Select an area of the screen and search it with Google Lens (like end4).
# Lens needs the picture at a public address: it's uploaded to uguu.se, a
# temporary file host (files are deleted after a few hours), then Lens opens
# in the browser. Nothing is uploaded if the selection is cancelled.
set -uo pipefail
for c in grim slurp curl jq; do
    command -v "$c" >/dev/null || { notify-send -a "Google Lens" "Can't search the screen" "Install $c first." ; exit 1; }
done
area=$(slurp) || exit 0
img=$(mktemp --suffix=.png)
trap 'rm -f "$img"' EXIT
grim -g "$area" "$img" || { notify-send -a "Google Lens" "Couldn't capture the area" "Try again."; exit 1; }
url=$(curl -sf --max-time 30 -F "files[]=@$img" https://uguu.se/upload | jq -r '.files[0].url // empty')
if [[ -z "$url" ]]; then
    notify-send -a "Google Lens" "Couldn't upload the picture" "Check your internet connection and try again."
    exit 1
fi
exec xdg-open "https://lens.google.com/uploadbyurl?url=$(jq -rn --arg u "$url" '$u|@uri')"
