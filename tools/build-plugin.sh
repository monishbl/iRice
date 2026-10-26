#!/usr/bin/env bash
# Builds the irice-glass Hyprland plugin against the installed Hyprland headers
# and puts it where iRice's Hyprland config loads it from. Run it again after a
# Hyprland update (Settings → Appearance offers this when the plugin is stale).
#   tools/build-plugin.sh [--load]   --load also loads it into the running Hyprland
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/irice"
command -v pkg-config >/dev/null && pkg-config --exists hyprland || {
    echo "Hyprland headers not found. On Arch they come with the hyprland package; you also need: base-devel pkgconf"
    exit 1
}
make -C "$ROOT/hypr/plugin/irice-glass" -s
mkdir -p "$DEST"
cp "$ROOT/hypr/plugin/irice-glass/irice-glass.so" "$DEST/irice-glass.so.new"
mv "$DEST/irice-glass.so.new" "$DEST/irice-glass.so"
echo "Built $DEST/irice-glass.so for Hyprland $(pkg-config --modversion hyprland)"
if [[ "${1:-}" == "--load" ]] && command -v hyprctl >/dev/null; then
    hyprctl plugin unload "$DEST/irice-glass.so" >/dev/null 2>&1 || true
    hyprctl plugin load "$DEST/irice-glass.so"
fi
