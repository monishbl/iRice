#!/usr/bin/env bash
# Adds "iRice" to the login screen (ly, SDDM, GDM… read wayland-sessions),
# next to your usual Hyprland. Picking it starts sandbox/irice-session.sh
# from this folder. Needs sudo once.
#   tools/setup-session.sh            add (or update) the entry
#   tools/setup-session.sh --remove   take it off the login screen
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FILE=/usr/share/wayland-sessions/irice.desktop

if [[ "${1:-}" == "--remove" ]]; then
    sudo rm -f "$FILE"
    echo "Removed iRice from the login screen."
    exit 0
fi

chmod +x "$ROOT/sandbox/irice-session.sh"
tmp=$(mktemp)
cat >"$tmp" <<EOF
[Desktop Entry]
Name=iRice
Comment=iRice, the macOS-style Hyprland desktop, from $ROOT
Exec=$ROOT/sandbox/irice-session.sh
Type=Application
DesktopNames=Hyprland
EOF
sudo install -Dm644 "$tmp" "$FILE"
rm -f "$tmp"
echo "Added: pick \"iRice\" on the login screen. (Your usual Hyprland stays as it is.)"
