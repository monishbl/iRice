#!/usr/bin/env bash
# Loads this checkout's shell in the dev sandbox in both looks, opens every
# panel and every Settings page, and fails on any QML error: the check a
# refactor runs before it's merged (tests don't load the shell).
#   tools/smoke-looks.sh [glass|material]   (both when not given)
# Runs in the first free sandbox arena (SMOKE_ARENAS of them, 3 by default:
# the usual sandbox and IRICE_ARENA=2, 3, started when needed), so checkouts
# check side by side; each arena's config is put back afterwards. Screenshots
# of the headless test output go to $SMOKE_SHOTS when it's set.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN="${IRICE_MAIN:-/home/$(id -un)/Projects/Personal/iRice}"
LOOKS=("${@:-glass material}")
[[ $# -eq 0 ]] && LOOKS=(glass material)
PANELS=(controlCenter notificationCenter spotlight launchpad clipboard overview missionControl emoji cheatsheet
    quickNote screenTools session sidebar wallpaper about systemMenu soundMenu wifiMenu bluetoothMenu batteryMenu
    resourcesMenu weatherMenu privacyMenu)
ERRORS='ERROR|caused by|ReferenceError|TypeError|is not defined|is not a type|Cannot assign|Unable to assign|non-existent property|Cannot read property|Cannot call method|unavailable'

cd "$MAIN" || exit 2
# (one checkout at a time per arena: the first free one; arena 1 keeps the
# lock name older checkouts' copies of this script use)
arena=""
until [[ -n "$arena" ]]; do
    for n in $(seq 1 "${SMOKE_ARENAS:-3}"); do
        lock=/tmp/irice-smoke.lock
        [[ $n != 1 ]] && lock=/tmp/irice-smoke-$n.lock
        exec 9>"$lock"
        if flock -n 9; then arena=$n; break; fi
        exec 9>&-
    done
    [[ -z "$arena" ]] && sleep 5
done
export IRICE_ARENA=$arena
IRICE_RT=$( ROOT="$MAIN"; source "$MAIN/sandbox/lib.sh"; echo "$IRICE_RT" )
IRICE_HOME=$( ROOT="$MAIN"; source "$MAIN/sandbox/lib.sh"; echo "$IRICE_HOME" )
LOG="$IRICE_RT/quickshell.log"
echo "arena $arena ($IRICE_RT)"
./dev.sh status | grep -q running || { ./dev.sh up >/dev/null; sleep 30; }
CFG="$IRICE_HOME/.config/irice/config.json"
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
# (through this checkout's shell-ipc.sh: main's looks for a Glass shell in
# main's shell/ only, so from a worktree every call missed it)
shell_ipc() { ./dev.sh run env QS=qs "$ROOT/shell/assets/scripts/shell-ipc.sh" call "$@"; }
ipc() { shell_ipc "$@" >/dev/null 2>&1; }

# What's stuck when the shell stops answering or drawing: the compositor or
# the shell, and what the shell's threads are doing
stuck_report() {
    local pid
    pid="$(pgrep -a -x irice-shell | awk -v rt="$IRICE_RT/" 'index($2, rt) == 1 {print $1}' | head -1)"
    echo "     compositor answers: $(timeout 10 ./dev.sh hyprctl monitors >/dev/null 2>&1 && echo yes || echo no)"
    if [[ -n "$pid" ]]; then
        echo "     shell $pid, threads (count, state, cpu, waiting in, name):"
        ps -L -o stat=,pcpu=,wchan:28=,comm= -p "$pid" | sort | uniq -c | sort -rn | head -8 | sed 's/^/       /'
    else
        echo "     no shell running"
    fi
    sed 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -v -E 'INFO|DEBUG|textinput' | tail -6 | sed 's/^/     log: /'
}

failed=0
for look in "${LOOKS[@]}"; do
    set_look "$look"
    sleep 25 # (Material's start guard counts a reload within ~20 s as a failed start)
    # (this checkout's shaders, and its shell: IRICE_ROOT given inside the
    # container, where dev.sh's own environment doesn't reach)
    "$ROOT/tools/build-shaders.sh" >/dev/null 2>&1
    ./dev.sh run env IRICE_ROOT="$ROOT" IRICE_LAB=0 "$MAIN/sandbox/reload-shell.sh" >/dev/null 2>&1
    sleep 45
    if [[ "$(look_now)" != "$look" ]]; then
        echo "FAIL $look: the start guard switched the look (the $look shell didn't start)"
        failed=1
        continue
    fi
    # (and it really is this checkout's shell running, not main's)
    if [[ "$look" == material ]]; then
        loaded="$(readlink -f "$IRICE_HOME/.cache/irice/material-shell/shell.qml")"
    else
        # (the sandbox's own shell: its binary link is in the sandbox's runtime
        # folder; the container also sees the person's real one)
        loaded="$(./dev.sh run sh -c 'pgrep -a -x irice-shell' 2>/dev/null | grep "^[0-9]* $IRICE_RT/" | grep -o -- "-p [^ ]*" | head -1)"
    fi
    if [[ "$loaded" != *"$ROOT/"* ]]; then
        echo "FAIL $look: the sandbox runs another checkout ($loaded), not $ROOT"
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
    # (a shell that died on the way logs no QML error: it must still answer,
    # and its screenshots mustn't be blank)
    if [[ "$(timeout 20 shell_ipc show 2>&1 | grep -c '^target')" -lt 5 ]]; then
        echo "FAIL $look: the shell stopped answering during the run"
        stuck_report
        failed=1
        continue
    fi
    if [[ -n "${SMOKE_SHOTS:-}" ]]; then
        read -r blank frozen < <(python3 -I - "$SMOKE_SHOTS" "$look" <<'PY'
import sys, os, pathlib, hashlib
from PIL import Image, ImageStat
blank = frozen = 0
prev = None
for f in sorted(pathlib.Path(sys.argv[1]).glob(sys.argv[2] + "-*.png"), key=os.path.getmtime):
    # (nothing drawn: near black on average; the pointer alone can be white)
    blank += ImageStat.Stat(Image.open(f).convert("L")).mean[0] < 6
    # (a different panel, the very same pixels: the shell stopped drawing)
    h = hashlib.md5(Image.open(f).tobytes()).digest()
    frozen += h == prev
    prev = h
print(blank, frozen)
PY
)
        if [[ "$blank" -gt 0 ]]; then
            echo "FAIL $look: $blank screenshots are blank (nothing drawn)"
            failed=1
            continue
        fi
        if [[ "$frozen" -gt 0 ]]; then
            echo "FAIL $look: $frozen screenshots repeat the one before (the shell stopped drawing)"
            stuck_report
            failed=1
            continue
        fi
    fi
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
