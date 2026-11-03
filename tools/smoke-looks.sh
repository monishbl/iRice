#!/usr/bin/env bash
# Loads this checkout's shell in the dev sandbox in both looks, opens every
# panel and every Settings page, and fails on any QML error: the check a
# refactor runs before it's merged (tests don't load the shell).
#   tools/smoke-looks.sh [glass|material]   (both when not given)
# The sandbox is one, in the main checkout: checkouts take turns (a lock),
# and the sandbox's config is put back afterwards. Screenshots of the
# headless test output go to $SMOKE_SHOTS when it's set.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN="${IRICE_MAIN:-/home/$(id -un)/Projects/Personal/iRice}"
LOOKS=("${@:-glass material}")
[[ $# -eq 0 ]] && LOOKS=(glass material)
LOG=/tmp/irice-rt/quickshell.log
CFG="$MAIN/.sandbox/home/.config/irice/config.json"
PANELS=(controlCenter notificationCenter spotlight launchpad clipboard overview missionControl emoji cheatsheet
    quickNote screenTools session sidebar wallpaper about systemMenu soundMenu wifiMenu bluetoothMenu batteryMenu
    resourcesMenu weatherMenu privacyMenu)
ERRORS='ERROR|caused by|ReferenceError|TypeError|is not defined|is not a type|Cannot assign|Unable to assign|non-existent property|Cannot read property|Cannot call method|unavailable'

exec 9>/tmp/irice-smoke.lock
flock 9 # (one checkout at a time in the one sandbox)
cd "$MAIN" || exit 2
./dev.sh status | grep -q running || { ./dev.sh up >/dev/null; sleep 30; }
backup="$(mktemp)"
cp "$CFG" "$backup"
trap 'cp "$backup" "$CFG"; rm -f "$backup"' EXIT

set_look() {
    python3 -I - "$CFG" "$1" <<'PY'
import json, sys
p, look = sys.argv[1], sys.argv[2]
c = json.load(open(p))
c.setdefault("appearance", {})["look"] = look
json.dump(c, open(p, "w"), indent=4)
PY
}
look_now() { python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("appearance", {}).get("look"))' "$CFG"; }
ipc() { ./dev.sh ipc "$@" >/dev/null 2>&1; }

failed=0
for look in "${LOOKS[@]}"; do
    set_look "$look"
    sleep 25 # (Material's start guard counts a reload within ~20 s as a failed start)
    IRICE_ROOT="$ROOT" ./dev.sh reload >/dev/null 2>&1
    sleep 45
    if [[ "$(look_now)" != "$look" ]]; then
        echo "FAIL $look: the start guard switched the look (the $look shell didn't start)"
        failed=1
        continue
    fi
    ./dev.sh hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null 2>&1
    for p in "${PANELS[@]}"; do
        ipc panels open "$p" IRICE-TEST
        sleep 1.2
        [[ -n "${SMOKE_SHOTS:-}" ]] && mkdir -p "$SMOKE_SHOTS" && ./dev.sh shot "$SMOKE_SHOTS/$look-$p.png" >/dev/null 2>&1
        ipc panels close
        sleep 0.4
    done
    for page in $(grep -o 'id: "[a-z-]*"' "$ROOT/shell/common/SettingsMap.js" | cut -d'"' -f2); do
        ipc panels settings "$page"
        sleep 1.2
        [[ -n "${SMOKE_SHOTS:-}" ]] && ./dev.sh shot "$SMOKE_SHOTS/$look-settings-$page.png" >/dev/null 2>&1
    done
    ipc panels close
    found=$(sed 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -E "$ERRORS" | grep -v -E 'greeter.json|matugen' | sed -E 's/0x[0-9a-f]+/0x…/g' | sort | uniq -c | sort -rn)
    if [[ -n "$found" ]]; then
        echo "FAIL $look:"
        echo "$found" | head -40
        failed=1
    else
        echo "ok   $look: shell started, ${#PANELS[@]} panels and every Settings page opened without QML errors"
    fi
done
exit $failed
