#!/usr/bin/env bash
# Select an area of the screen and search it with Google Lens (like end4).
# The picture goes straight to Lens (lens-upload.py, as circle to search's
# "Search the web" does), then its results open in the browser. Nothing is
# uploaded if the selection is cancelled.
set -uo pipefail
for c in grim slurp jq python3; do
    command -v "$c" >/dev/null || { notify-send -a "Screen search" "Can't search the screen" "Install $c first." ; exit 1; }
done
area=$(slurp) || exit 0
umask 077
img=$(mktemp --suffix=.jpg)
trap 'rm -f "$img"' EXIT
grim -t jpeg -q 90 -g "$area" "$img" || { notify-send -a "Screen search" "Couldn't capture the area" "Try again."; exit 1; }
out=$(python3 -I "$(dirname "$0")/lens-upload.py" "$img")
url=$(jq -r 'select(.ok) | .url' <<<"$out")
if [[ -n "$url" ]]; then
    exec xdg-open "$url"
fi
# Lens changed how it takes pictures: paste it there instead
if [[ $(jq -r '.fallback' <<<"$out") == true ]]; then
    wl-copy --type image/jpeg <"$img"
    notify-send -a "Screen search" "Paste the picture into Lens" "It's on the clipboard: press Ctrl+V on the page that opens."
    exec xdg-open "https://lens.google.com/"
fi
notify-send -a "Screen search" "Couldn't search the picture" "$(jq -r '.error // "Try again."' <<<"$out")"
exit 1
