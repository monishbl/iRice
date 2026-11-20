#!/usr/bin/env bash
# Builds the site's downloadable settings package from your current Tessera
# settings (the ones ./dev.sh tty uses): settings and look only, no personal
# data, keys, widget layout or wallpapers (the macOS ones are Apple's).
#   site/build.sh [config.json]    then: npx wrangler deploy --config site/wrangler.jsonc
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# Your settings when there are any here (and the cleaned copy is kept in
# site/package/config.json, which is what GitHub's deploy builds from)
SAVED="$HERE/package/config.json"
SRC="${1:-$HERE/../.sandbox/home/.config/tessera/config.json}"
[[ -f "$SRC" ]] || SRC="$SAVED"
OUT="$HERE/public/config"
work=$(mktemp -d)
mkdir -p "$work/tessera-config" "$OUT"
python3 - "$SRC" "$work/tessera-config/config.json" <<'PY'
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
# Nothing tied to your accounts or hardware either (dropped, not blanked, so
# an importer keeps their own): a calendar id for new events or a task list
# exists only in your account, and the face camera only on your computer
for sec, key in (("google", "newEventCalendar"), ("google", "taskListId"), ("google", "hiddenCalendars"),
                 ("cloud", "hiddenCalendars"), ("lock", "faceDevice"), ("lock", "faceCount")):
    if isinstance(d.get(sec), dict):
        d[sec].pop(key, None)
json.dump(d, open(sys.argv[2], "w"), indent=2)
text = json.dumps(d)
assert "/home/" not in text, "a home path slipped into the shared settings"
def account_ids(v):
    if isinstance(v, dict):
        for x in v.values():
            yield from account_ids(x)
    elif isinstance(v, list):
        for x in v:
            yield from account_ids(x)
    elif isinstance(v, str) and (v.startswith("cloud|") or v.startswith("google|")):
        yield v
assert not list(account_ids(d)), "an account's calendar or list id slipped into the shared settings"
PY
[[ "$SRC" == "$SAVED" ]] || cp "$work/tessera-config/config.json" "$SAVED"
cp "$HERE/package/import.sh" "$HERE/package/README.txt" "$work/tessera-config/"
chmod +x "$work/tessera-config/import.sh"
tar czf "$OUT/tessera-config.tar.gz" -C "$work" tessera-config
rm -rf "$work"
echo "Built $OUT/tessera-config.tar.gz"
