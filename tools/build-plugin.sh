#!/usr/bin/env bash
# Builds the tessera-glass Hyprland plugin against the installed Hyprland headers
# and puts it where Tessera's Hyprland config loads it from. Run it again after a
# Hyprland update (Settings → Appearance offers this when the plugin is stale).
#   tools/build-plugin.sh [--load]   --load also loads it into the running Hyprland
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# (iRice's folders, from before it became Tessera, move over first: never both)
python3 -I "$ROOT/shell/assets/scripts/migrate-names.py" || true
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/tessera"
command -v pkg-config >/dev/null && pkg-config --exists hyprland || {
    echo "Hyprland headers not found. On Arch they come with the hyprland package; you also need: base-devel pkgconf"
    exit 1
}
make -C "$ROOT/hypr/plugin/tessera-glass" -s
mkdir -p "$DEST"
cp "$ROOT/hypr/plugin/tessera-glass/tessera-glass.so" "$DEST/tessera-glass.so.new"
mv "$DEST/tessera-glass.so.new" "$DEST/tessera-glass.so"
# (built before iRice became Tessera: this one replaces it; a running
# Hyprland keeps the copy it loaded until you log out)
rm -f "$DEST/irice-glass.so" "$DEST/irice-glass.built-for"
echo "Built $DEST/tessera-glass.so for Hyprland $(pkg-config --modversion hyprland)"
if [[ "${1:-}" == "--load" ]] && command -v hyprctl >/dev/null; then
    hyprctl plugin unload "$DEST/tessera-glass.so" >/dev/null 2>&1 || true
    # (and one loaded from where it was built before iRice became Tessera:
    # both would draw the glass)
    for old in "${XDG_DATA_HOME:-$HOME/.local/share}/irice/irice-glass.so" "$DEST/irice-glass.so" "$ROOT/hypr/plugin/irice-glass/irice-glass.so"; do
        hyprctl plugin unload "$old" >/dev/null 2>&1 || true
    done
    hyprctl plugin load "$DEST/tessera-glass.so"
fi
