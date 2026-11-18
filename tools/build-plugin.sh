#!/usr/bin/env bash
# Builds the tessera-glass Hyprland plugin against the installed Hyprland headers
# and puts it where Tessera's Hyprland config loads it from. Run it again after a
# Hyprland update (Settings → Appearance offers this when the plugin is stale).
#   tools/build-plugin.sh [--load]   --load also loads it into the running Hyprland
#
# The whole run goes through tools/build-progress.py when it's beside this:
# the phase, the time and the log ($XDG_STATE_HOME/tessera/build.log) for the
# installer, Software Update and Settings → Appearance (which runs this, with
# --load, and shows how far it is). A failure says why on a line starting
# "error: " (the progress file's "error": Settings shows it).
# Beside the plugin it writes tessera-glass.built-for: the Hyprland version it
# was built for (Settings compares it with the running Hyprland).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -z "${TESSERA_PLUGIN_INNER:-}" ]] && command -v python3 >/dev/null && [[ -f "$ROOT/tools/build-progress.py" ]]; then
    exec python3 -I "$ROOT/tools/build-progress.py" make --label tessera-glass --title "the glass plugin" -- env TESSERA_PLUGIN_INNER=1 bash "${BASH_SOURCE[0]}" "$@"
fi
# (folders under Tessera's old name move over first, never both: migrate-names.py)
python3 -I "$ROOT/shell/assets/scripts/migrate-names.py" --no-checkout 2>/dev/null || true
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/tessera"
LOAD=0
[[ "${1:-}" == "--load" ]] && LOAD=1
# X.Y.Z out of whatever a version says ("v0.57.2-b", "0.57.2.r12.gabc")
xyz() { grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -1; }
if ! command -v pkg-config >/dev/null || ! pkg-config --exists hyprland; then
    echo "error: Hyprland's headers aren't installed: on Arch they come with the hyprland package; you also need base-devel and pkgconf"
    exit 1
fi
headers=$(pkg-config --modversion hyprland | xyz)
running=""
if (( LOAD )) && command -v hyprctl >/dev/null && [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]]; then
    running=$(hyprctl version -j 2>/dev/null | sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' | xyz || true)
    [[ -n "$running" ]] || running=$(hyprctl version 2>/dev/null | head -1 | xyz || true)
fi
echo "Building the glass plugin for Hyprland $headers${running:+ (running: $running)}"
make -C "$ROOT/hypr/plugin/tessera-glass" -s
mkdir -p "$DEST"
cp "$ROOT/hypr/plugin/tessera-glass/tessera-glass.so" "$DEST/tessera-glass.so.new"
mv "$DEST/tessera-glass.so.new" "$DEST/tessera-glass.so"
printf '%s\n' "$headers" >"$DEST/tessera-glass.built-for"
echo "Built $DEST/tessera-glass.so for Hyprland $headers"
(( LOAD )) || exit 0
if ! command -v hyprctl >/dev/null || [[ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]]; then
    echo "Not in a Hyprland session: it loads at your next login"
    exit 0
fi
# A plugin built from other headers than the running Hyprland's doesn't load
# (Hyprland checks): the package was updated since this session started, or
# the headers are another build's
if [[ -n "$running" && "$running" != "$headers" ]]; then
    echo "error: Hyprland's headers ($headers) don't match the running Hyprland ($running)"
    exit 1
fi
# Where this session's Hyprland config loads it from (hypr/tessera/glass.lua):
# a console session started from a checkout (sandbox/tty-shell.sh sets
# TESSERA_ROOT and TESSERA_TTY) loads the checkout's own build; an installed
# session the copy in $DEST
so="$DEST/tessera-glass.so"
from="${TESSERA_ROOT:-}"
if [[ "${TESSERA_TTY:-}" == 1 && -n "$from" && "$(cd "$from" 2>/dev/null && pwd -P)" == "$(cd "$ROOT" && pwd -P)" ]]; then
    so="$ROOT/hypr/plugin/tessera-glass/tessera-glass.so"
fi
# (every copy this session may have loaded, or two would draw the glass)
for old in "$DEST/tessera-glass.so" "$ROOT/hypr/plugin/tessera-glass/tessera-glass.so"; do
    hyprctl plugin unload "$old" >/dev/null 2>&1 || true
done
out=$(hyprctl plugin load "$so" 2>&1 || true)
if [[ "$(printf '%s' "$out" | tr -d '[:space:]')" != ok ]]; then
    echo "hyprctl: $out"
    if printf '%s' "$out" | grep -qiE 'version|mismatch|hash'; then
        echo "error: Hyprland's headers ($headers) don't match the running Hyprland"
    else
        echo "error: Hyprland didn't load it: $(printf '%s' "$out" | head -1)"
    fi
    exit 1
fi
echo "Loaded $so"
