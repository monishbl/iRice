#!/usr/bin/env bash
# Applies these iRice settings (look and behaviour, plus the wallpapers they
# use). Your current settings are backed up first; nothing else is touched.
#
#   ./import.sh                 for an installed iRice (~/.config/irice)
#   ./import.sh --repo <dir>    for iRice run from a repo with ./dev.sh tty
#                               (settings in <dir>/.sandbox/home/.config/irice)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

CONF="${XDG_CONFIG_HOME:-$HOME/.config}/irice"
if [[ "${1:-}" == "--repo" ]]; then
    [[ -d "${2:-}" ]] || { echo "Usage: ./import.sh --repo <path to the iRice repo>"; exit 1; }
    CONF="$(cd "$2" && pwd)/.sandbox/home/.config/irice"
elif [[ -n "${1:-}" ]]; then
    sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi
command -v python3 >/dev/null || { echo "This needs python3."; exit 1; }

WALL="$HOME/Pictures/Wallpapers/iRice"
mkdir -p "$CONF"
[[ -d "$HERE/wallpapers" ]] && mkdir -p "$WALL"

# Back up what's there
if [[ -f "$CONF/config.json" ]]; then
    backup="$CONF/config.json.before-import-$(date +%Y%m%d-%H%M%S)"
    cp "$CONF/config.json" "$backup"
    echo "Your previous settings: $backup"
fi

# Wallpapers (when the package has any)
if [[ -d "$HERE/wallpapers" ]]; then
    cp -r "$HERE/wallpapers/." "$WALL/"
    echo "Wallpapers: $WALL"
fi

# Settings, pointing at the wallpapers' new home. Keys these settings don't
# mention (your desktop widgets, for one) keep their current values.
python3 - "$HERE/config.json" "$CONF/config.json" "$WALL" <<'EOF'
import json, os, sys
src, dst, wall = sys.argv[1:4]
new = json.load(open(src))
def fill(x):
    if isinstance(x, dict):
        return {k: fill(v) for k, v in x.items()}
    if isinstance(x, list):
        return [fill(v) for v in x]
    if isinstance(x, str) and x.startswith("@WALLPAPERS@/"):
        return os.path.join(wall, x[len("@WALLPAPERS@/"):])
    return x
new = fill(new)
old = json.load(open(dst)) if os.path.exists(dst) else {}
def merge(a, b):
    for k, v in b.items():
        a[k] = merge(a.get(k, {}), v) if isinstance(v, dict) and isinstance(a.get(k), dict) else v
    return a
tmp = dst + ".tmp"
json.dump(merge(old, new), open(tmp, "w"), indent=2)
os.replace(tmp, dst) # (whole file at once: a running shell never reads half of it)
EOF
echo "Settings: $CONF/config.json"
echo
echo "Done. A running iRice picks them up by itself; if anything looks off, restart the shell (irice repair, or log out and in)."
echo "To undo: copy the backup above back to config.json."
