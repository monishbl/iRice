#!/usr/bin/env bash
# iRice as a login-screen session (ly, SDDM, GDM…: "iRice" next to Hyprland).
# Installed by tools/setup-session.sh as /usr/share/wayland-sessions/irice.desktop.
#
# The same iRice as ./dev.sh tty (this folder's settings, your ~/.config/hypr
# and usual desktop untouched), started by the login screen instead of a
# console. Two ways, decided at start:
#   - the only graphical login of yours: a normal session. Hyprland tells the
#     user systemd and D-Bus about its display, so portals, the keyring and
#     background services work as in any desktop.
#   - another desktop of yours is logged in too (say end4 on another console):
#     isolated like ./dev.sh tty, on its own message bus and leaving the user
#     systemd alone, so neither desktop takes the other's display (apps would
#     hang on portals that open on the wrong screen, or none).
# A log of each start: ~/.local/state/irice-session.log
set -uo pipefail
ROOT="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
LOG="${XDG_STATE_HOME:-$HOME/.local/state}/irice-session.log"
mkdir -p "$(dirname "$LOG")"
exec >>"$LOG" 2>&1
echo "== $(date '+%F %T') iRice session (from $ROOT)"

# Another graphical session of this user that's still running?
others=0
for s in $(loginctl list-sessions --no-legend 2>/dev/null | awk -v u="$(id -un)" '$3 == u { print $1 }'); do
    [[ "$s" == "${XDG_SESSION_ID:-}" ]] && continue
    t=$(loginctl show-session "$s" -p Type --value 2>/dev/null)
    st=$(loginctl show-session "$s" -p State --value 2>/dev/null)
    [[ ( "$t" == wayland || "$t" == x11 ) && "$st" != closing ]] && others=1
done

if ! command -v start-hyprland >/dev/null || [[ ! -f "$ROOT/sandbox/hypr/tty.lua" ]]; then
    echo "Can't start: start-hyprland or $ROOT/sandbox/hypr/tty.lua is missing. Run $ROOT/tools/setup-tty.sh."
    exit 1
fi

export IRICE_ROOT="$ROOT"
if [[ $others == 1 ]]; then
    echo "Another desktop of yours is logged in: starting isolated"
    export HYPRLAND_NO_SD_VARS=1
    exec dbus-run-session -- start-hyprland -- --config "$ROOT/sandbox/hypr/tty.lua"
fi
echo "The only desktop logged in: starting as a normal session"
exec start-hyprland -- --config "$ROOT/sandbox/hypr/tty.lua"
