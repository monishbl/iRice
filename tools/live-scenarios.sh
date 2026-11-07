#!/usr/bin/env bash
# live-scenarios.sh: what a person does, in a hidden sandbox arena with YOUR
# settings (your config.json copied in: look, icon theme, motion, widgets),
# once with the QML shell and once with tessera-ui --standalone. Pointer
# hovers and clicks (the virtual pointer), windows opened and closed, short
# recordings of animations; pictures and videos side by side in OUT.
#   tools/live-scenarios.sh [--arena N] [--config FILE] [--only qml|rust] [--out DIR] [--bin TESSERA_UI] [--look glass|material]
# Never your screen: everything happens on the arena's headless IRICE-TEST.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARENA=7 USERCFG="$HOME/iRice/.sandbox/home/.config/tessera/config.json" ONLY="" OUT="$HOME/tessera-verify/live-$(date +%Y%m%d-%H%M)"
BIN="" LOOK=""
while (($#)); do case "$1" in --arena) ARENA=$2; shift;; --config) USERCFG=$2; shift;; --only) ONLY=$2; shift;; --out) OUT=$2; shift;; --bin) BIN=$2; shift;; --look) LOOK=$2; shift;; esac; shift; done
# (the tessera-ui to test: another worktree's build, say; default this checkout's)
BIN="${BIN:-$ROOT/native/tessera-ui/target/release/tessera-ui}"
export IRICE_ARENA=$ARENA
. "$ROOT/site/film/tessera/capture/lib.sh"
CFG="$HOMEDIR/.config/tessera/config.json"
mkdir -p "$OUT"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$OUT/run.log"; }
trap 'dev down >/dev/null 2>&1' EXIT
P() { dev pointer -o IRICE-TEST "$@" </dev/null >/dev/null 2>&1; }
shot() { dev shot "$OUT/$1.png" </dev/null >/dev/null 2>&1; }
rec() { # name seconds command…: a recording of the whole screen while the command runs
    local n=$1 s=$2; shift 2
    inbox "wf-recorder -o IRICE-TEST -r 60 -f /tmp/rec-$n.mp4 >/dev/null 2>&1 & r=\$!; sleep 0.6; $*; sleep $s; kill -INT \$r; wait \$r 2>/dev/null; cp /tmp/rec-$n.mp4 '$OUT/$n.mp4'" </dev/null >/dev/null 2>&1
}
use_config() { # standalone 0|1
    python3 -I - "$USERCFG" "$CFG" "$1" "$LOOK" <<'PY'
import json, sys
src, dst, sa, look = sys.argv[1:]
c = json.load(open(src))
if look:
    c.setdefault("appearance", {})["look"] = look
c.setdefault("rust", {})["standalone"] = sa == "1"
c["rust"]["surfaces"] = []
for k in ("ai", "google", "cloud", "phone"):   # (accounts and keys stay out of the arena)
    c.pop(k, None)
json.dump(c, open(dst, "w"), indent=4)
PY
}
run_pass() { # qml|rust
    local pass=$1
    log "[$pass] setting up"
    use_config "$([[ $pass == rust ]] && echo 1 || echo 0)"
    install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    dev reload </dev/null >/dev/null 2>&1; sleep 14
    inbox 'ipc lock unlock; ipc panels close' </dev/null >/dev/null 2>&1; sleep 2
    P move 1500 600; sleep 1
    shot "$pass-01-idle"
    # the island: hovered open (its status line), then a hover over CPU
    P glide 960 14 400; sleep 1.5
    shot "$pass-02-island-hover"
    P glide 1500 600 300; sleep 1
    # the dock, magnified under the pointer
    P glide 960 1050 400; sleep 1
    shot "$pass-03-dock-hover"
    P glide 1500 600 300; sleep 1
    # panels, each recorded opening
    rec "$pass-04-controlcenter" 1.2 "ipc panels open controlCenter IRICE-TEST"
    shot "$pass-04-controlcenter"
    inbox 'ipc panels close' </dev/null >/dev/null 2>&1; sleep 1
    rec "$pass-05-launchpad" 1.2 "ipc panels open launchpad IRICE-TEST"
    shot "$pass-05-launchpad"
    inbox 'ipc panels close' </dev/null >/dev/null 2>&1; sleep 1
    # windows over the desktop widgets: two open, one closed (recorded)
    inbox 'foot >/dev/null 2>&1 & sleep 1.2; foot >/dev/null 2>&1 &' </dev/null >/dev/null 2>&1; sleep 2.5
    shot "$pass-06-windows"
    rec "$pass-07-close-window" 1.5 "hyprctl dispatch 'hl.dsp.window.close({ window = \"class:foot\" })'"
    sleep 1; shot "$pass-07-after-close"
    inbox "hyprctl dispatch 'hl.dsp.window.close({ window = \"class:foot\" })'" </dev/null >/dev/null 2>&1; sleep 1.5
    shot "$pass-08-desktop-again"
    # notifications with app icons
    inbox "notify-send -a Firefox -i firefox 'Download finished' 'report.pdf'; notify-send -a Files -i org.gnome.Nautilus 'Copied' '3 files'" </dev/null >/dev/null 2>&1; sleep 1.5
    shot "$pass-09-notifications"
    inbox 'ipc panels open notificationCenter IRICE-TEST' </dev/null >/dev/null 2>&1; sleep 1.5
    shot "$pass-10-notification-center"
    inbox 'ipc panels close' </dev/null >/dev/null 2>&1
    dev run bash -c 'cat /tmp/irice-rt*/tessera-ui*.log 2>/dev/null | tail -50' > "$OUT/$pass-ui.log" 2>&1
}
log "arena $ARENA, your settings from $USERCFG"
setup "${LOOK:-material}" "$WALLS/wall-tiles.jpg" 1
[[ $ONLY != rust ]] && run_pass qml
[[ $ONLY != qml ]] && run_pass rust
log "done: $OUT"
