#!/usr/bin/env bash
# Adds "Tessera" to the login screen (ly, SDDM, GDM… read wayland-sessions),
# next to your usual Hyprland. Picking it starts sandbox/tessera-session.sh
# from this folder. Needs sudo once.
#   tools/setup-session.sh            add (or update) the entry
#   tools/setup-session.sh --remove   take it off the login screen
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
FILE=/usr/share/wayland-sessions/tessera.desktop
source "$ROOT/shell/assets/scripts/old-names.sh"
OLD=$OLD_SESSION_ENTRY # (the entry under Tessera's old name)

if [[ "${1:-}" == "--remove" ]]; then
    sudo rm -f "$FILE" "$OLD"
    echo "Removed Tessera from the login screen."
    exit 0
fi

chmod +x "$ROOT/sandbox/tessera-session.sh"
tmp=$(mktemp)
cat >"$tmp" <<EOF
[Desktop Entry]
Name=Tessera
Comment=Tessera, the macOS-style Hyprland desktop, from $ROOT
Exec=$ROOT/sandbox/tessera-session.sh
Type=Application
DesktopNames=Hyprland
EOF
sudo install -Dm644 "$tmp" "$FILE"
sudo rm -f "$OLD"
rm -f "$tmp"
echo "Added: pick \"Tessera\" on the login screen. (Your usual Hyprland stays as it is.)"
