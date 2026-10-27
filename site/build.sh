#!/usr/bin/env bash
# Builds the site's downloadable settings package from your current iRice
# settings (the ones ./dev.sh tty uses): settings and look only, no personal
# data, keys, widget layout or wallpapers (the macOS ones are Apple's).
#   site/build.sh [config.json]    then: npx wrangler deploy --config site/wrangler.jsonc
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="${1:-$HERE/../.sandbox/home/.config/irice/config.json}"
OUT="$HERE/public/config"
work=$(mktemp -d)
mkdir -p "$work/irice-config" "$OUT"
python3 - "$SRC" "$work/irice-config/config.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d.get("desktop", {}).pop("widgets", None)
d.get("desktop", {}).pop("iconPositions", None)
w = d.get("wallpaper", {})
for k in ("path", "lightPath", "darkPath", "dynamicFolder", "directory", "live"):
    if k in w:
        w[k] = ""
w["dynamic"] = "off"
# Nothing that identifies you or holds a key
for sec, key in (("sharing", "name"), ("weather", "location"), ("emoji", "klipyKey"), ("notes", "folder"), ("desktop", "photosFolder")):
    if key in d.get(sec, {}):
        d[sec][key] = ""
json.dump(d, open(sys.argv[2], "w"), indent=2)
text = json.dumps(d)
assert "/home/" not in text, "a home path slipped into the shared settings"
PY
cp "$HERE/package/import.sh" "$HERE/package/README.txt" "$work/irice-config/"
chmod +x "$work/irice-config/import.sh"
tar czf "$OUT/irice-config.tar.gz" -C "$work" irice-config
rm -rf "$work"
echo "Built $OUT/irice-config.tar.gz"
