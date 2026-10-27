#!/usr/bin/env bash
# Demo extras for ./demo.sh (started by tty.lua inside the session when
# IRICE_DEMO=1): once the shell answers, a track "plays" (Now Playing and the
# island's live activity) and a few notifications arrive. Everything it starts
# ends with this session's compositor.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SB="$ROOT/.sandbox/home"
export XDG_CONFIG_HOME="$SB/.config" XDG_STATE_HOME="$SB/.local/state" XDG_CACHE_HOME="$SB/.cache" XDG_DATA_HOME="$SB/.local/share"
QS=qs
[[ -x "$HOME/.local/share/irice/quickshell/bin/qs" ]] && QS="$HOME/.local/share/irice/quickshell/bin/qs"

alive() { timeout 3 hyprctl version >/dev/null 2>&1; }

# Wait for the shell (up to a minute)
for _ in $(seq 60); do
    "$QS" -p "$ROOT/shell" ipc show >/dev/null 2>&1 && break
    sleep 1
done
sleep 3 # let the menu bar and island settle

# A track for Now Playing, with a wallpaper as its artwork
art=$(find "$ROOT/shell/assets" "$SB/Pictures" -maxdepth 3 -type f \( -name '*.jpg' -o -name '*.png' \) 2>/dev/null | head -1)
python3 "$ROOT/sandbox/tools/fakeplayer.py" "${art:+file://$art}" >/dev/null 2>&1 &
player=$!

# A few notifications, spaced out like real ones
note() { notify-send -a "$1" -i "$2" "$3" "$4"; sleep 4; }
sleep 2
note "Calendar" "x-office-calendar" "Team demo in 15 minutes" "Conference room · iRice walkthrough"
note "Messages" "mail-message-new" "Sam" "Looking forward to seeing the new desktop!"
note "Weather" "weather-few-clouds" "Clearing up this afternoon" "Highs of 24° from 2 PM"

# Keep the player until the session ends
while alive; do sleep 5; done
kill "$player" 2>/dev/null
