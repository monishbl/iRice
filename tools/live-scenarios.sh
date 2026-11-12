#!/usr/bin/env bash
# live-scenarios.sh: what a person does, in a hidden sandbox arena with YOUR
# settings (your config.json copied in: look, icon theme, motion, widgets),
# once with the QML shell and once with tessera-ui --standalone. Pointer
# hovers and clicks (the virtual pointer), windows opened and closed, short
# recordings of animations; pictures and videos side by side in OUT.
#   tools/live-scenarios.sh [--arena N] [--config FILE] [--only qml|rust] [--out DIR]
#                           [--suite walk|latency] [--bin TESSERA_UI] [--look material|glass] [--perf]
# --suite latency: every panel's time to first motion and animation length,
# both shells (see run_latency); --bin: the tessera-ui to test (default this
# checkout's release build); --perf: the Rust pass logs stalls and reloads.
# Never your screen: everything happens on the arena's headless IRICE-TEST.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARENA=7 USERCFG="$HOME/iRice/.sandbox/home/.config/tessera/config.json" ONLY="" OUT="$HOME/tessera-verify/live-$(date +%Y%m%d-%H%M)"
SUITE=walk BIN="$ROOT/native/tessera-ui/target/release/tessera-ui" LOOK="" PERF=""
while (($#)); do case "$1" in --arena) ARENA=$2; shift;; --config) USERCFG=$2; shift;; --only) ONLY=$2; shift;; --out) OUT=$2; shift;;
    --suite) SUITE=$2; shift;; --bin) BIN=$2; shift;; --look) LOOK=$2; shift;; --perf) PERF=1;; esac; shift; done
export IRICE_ARENA=$ARENA
. "$ROOT/site/film/tessera/capture/lib.sh"
CFG="$HOMEDIR/.config/tessera/config.json"
mkdir -p "$OUT"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$OUT/run.log"; }
[[ -n ${KEEP_UP:-} ]] || trap 'dev down >/dev/null 2>&1' EXIT   # (KEEP_UP=1: left up, to look closer; ./dev.sh down after)
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
if look:   # (--look: the other look, the rest of your settings as they are)
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
    # (this arena's log only: /tmp/irice-rt* would read every arena's)
    tail -50 "$HOMEDIR/.local/state/tessera/tessera-ui.log" > "$OUT/$pass-ui.log" 2>&1
}

# ── The latency suite (--suite latency) ─────────────────────────────────────
# Every panel the QML has, opened the way a person opens it (its keyboard
# shortcut as Hyprland runs it: the QML's global shortcut, standalone's
# `tessera-ui ipc call shortcut <name>`; the status menus by a click on the
# menu bar), recorded; then closed, recorded. The time to first motion is
# measured from a marker in the same recording: the pointer jumps 40 px just
# before the command (the compositor draws the pointer at once), so
# "first motion - marker" is command → first moving frame, whatever the
# recorder's own start-up. tools/anim-timing.py gives the rest (duration,
# frames, longest gap). Results: $OUT/latency-<pass>.tsv; the stall/reload
# log (TESSERA_PERF) of the Rust pass in $OUT/<pass>-perf.log.
# name | how: "sc <shortcut>" (Hyprland's bind), "ipc <args>" (shell-ipc), "click X Y"
LAT_PANELS=(
    "controlCenter|sc controlCenter" "notificationCenter|sc notificationCenter" "launchpad|sc launchpad"
    "overview|sc overview" "missionControl|sc missionControl" "spotlight|sc spotlight" "sidebar|sc sidebar"
    "clipboard|sc clipboard" "emoji|sc emoji" "wallpaper|sc wallpaper" "session|sc session"
    "cheatsheet|sc cheatsheet" "quickNote|sc quickNote" "screenTools|sc screenTools"
    "soundMenu|ipc panels open soundMenu IRICE-TEST" "batteryMenu|ipc panels open batteryMenu IRICE-TEST"
    "bluetoothMenu|ipc panels open bluetoothMenu IRICE-TEST" "systemMenu|ipc panels open systemMenu IRICE-TEST"
    # (clicks on the menu bar at 1x: Control Center's item, the clock → Notification Center; the
    # tester checks the coordinates against a shot first: ./dev.sh shot)
    "ccClick|click 1895 14" "clockClick|click 1680 14"
)
LAT_REPS=${LAT_REPS:-3}
# the command that runs a shortcut, as the compositor does it, for this pass
# (LAT_SC=global: the Rust pass too dispatches the global shortcut, as
# keybinds.lua binds it once tessera-ui registers the names itself; the
# default, exec, is the old `tessera-ui ipc call shortcut` per press)
sc_cmd() { # pass name
    if [[ $1 == qml || ${LAT_SC:-exec} == global ]]; then echo "hyprctl dispatch 'hl.dsp.global(\"tessera:$2\")' >/dev/null"
    else echo "hyprctl dispatch 'hl.dsp.exec_cmd(\"$HOMEDIR/.local/share/tessera/bin/tessera-ui ipc call shortcut $2\")' >/dev/null"; fi
}
how_cmd() { # pass how…
    local pass=$1; shift
    case $1 in
        sc) sc_cmd "$pass" "$2" ;;
        ipc) shift; echo "ipc $*" ;;
        click) echo "P move $2 $3; P click" ;;   # (the pointer drawn at X,Y is ignored: --ignore)
    esac
}
# lat_rec name lead tail body: a recording with the marker (pointer 1880,1040 → 1840,1040) right before the body
lat_rec() {
    local n=$1 lead=$2 tl=$3 body=$4
    inbox "P move 1880 1040; rm -f /tmp/lat-$ARENA-$n.mp4
wf-recorder -o IRICE-TEST -r 60 -f /tmp/lat-$ARENA-$n.mp4 >/dev/null 2>&1 & r=\$!
sleep $lead; P move 1840 1040; $body
sleep $tl; kill -INT \$r; wait \$r 2>/dev/null; cp /tmp/lat-$ARENA-$n.mp4 '$OUT/$n.mp4'; rm -f /tmp/lat-$ARENA-$n.mp4" </dev/null >/dev/null 2>&1
}
# reload the arena's shell with TESSERA_PERF on (the stall and reload log)
perf_reload() {
    dev run env TESSERA_PERF=1 TESSERA_ROOT="$ROOT" "$ROOT/sandbox/reload-shell.sh" </dev/null >/dev/null 2>&1
}
run_latency() { # qml|rust
    local pass=$1
    local tsv="$OUT/latency-$pass.tsv"
    log "[$pass] latency suite: setting up"
    use_config "$([[ $pass == rust ]] && echo 1 || echo 0)"
    install -D -m 755 "$BIN" "$HOMEDIR/.local/share/tessera/bin/tessera-ui"
    rm -f "$HOMEDIR/.local/state/tessera/standalone-starts"
    if [[ $pass == rust && -n ${PERF:-} ]]; then perf_reload; else dev reload </dev/null >/dev/null 2>&1; fi
    sleep 16
    inbox 'ipc lock unlock; ipc panels close' </dev/null >/dev/null 2>&1; sleep 2
    : >"$OUT/$pass-perf.log"
    local logf="$HOMEDIR/.local/state/tessera/tessera-ui.log" mark
    printf 'panel\trep\topen_ms\topen_dur_ms\topen_frames\topen_gap_ms\tclose_ms\tclose_dur_ms\tclose_gap_ms\n' >"$tsv"
    for e in "${LAT_PANELS[@]}"; do
        local name=${e%%|*} how=${e#*|}
        [[ -n ${LAT_ONLY:-} && ! $name =~ ^($LAT_ONLY)$ ]] && continue
        for rep in $(seq 1 "$LAT_REPS"); do
            mark=$(wc -l <"$logf" 2>/dev/null || echo 0)
            lat_rec "$pass-open-$name-$rep" 1.0 1.6 "$(how_cmd "$pass" $how)"
            lat_rec "$pass-close-$name-$rep" 1.0 1.4 "ipc panels close; hyprctl dispatch 'hl.dsp.send_shortcut({ mods = \"\", key = \"Escape\" })' >/dev/null 2>&1"
            inbox 'ipc panels close' </dev/null >/dev/null 2>&1; sleep 0.8
            [[ -f $logf ]] && { echo "## $name $rep"; tail -n +"$((mark + 1))" "$logf" | grep -E '^debug: (stall|reload)'; } >>"$OUT/$pass-perf.log"
            local ign=(); [[ $how == click* ]] && read -r _ cx cy <<<"$how" && ign=(--ignore "$cx,$cy")
            python3 "$ROOT/tools/anim-timing.py" --marker "${ign[@]}" "$OUT/$pass-open-$name-$rep.mp4" "$OUT/$pass-close-$name-$rep.mp4" \
                | sed "s/^/$name\t$rep\t/" >>"$tsv"
            [[ -n ${KEEP:-} ]] || rm -f "$OUT/$pass-open-$name-$rep.mp4" "$OUT/$pass-close-$name-$rep.mp4"
        done
        log "[$pass] $name: $(grep "^$name	" "$tsv" | cut -f3 | tr '\n' ' ')ms"
    done
    # keyboard shortcuts' own path, standalone: the client's time per call (no panel)
    if [[ $pass == rust ]]; then
        dev run bash -c "for i in 1 2 3 4 5 6 7 8 9 10; do s=\$(date +%s%N); '$HOMEDIR/.local/share/tessera/bin/tessera-ui' ipc call shortcut nothingHere >/dev/null 2>&1; e=\$(date +%s%N); echo \$(( (e - s) / 1000 )); done" \
            </dev/null 2>/dev/null | sort -n | awk '{a[NR]=$1} END {print "ipc client us: median " a[int((NR+1)/2)] ", min " a[1] ", max " a[NR]}' | tee -a "$OUT/run.log"
    fi
}
log "arena $ARENA, your settings from $USERCFG"
setup material "$WALLS/wall-tiles.jpg" 1
if [[ $SUITE == latency ]]; then
    [[ $ONLY != rust ]] && run_latency qml
    [[ $ONLY != qml ]] && run_latency rust
    python3 "$ROOT/tools/latency-table.py" "$OUT" | tee "$OUT/latency.md"
else
    [[ $ONLY != rust ]] && run_pass qml
    [[ $ONLY != qml ]] && run_pass rust
fi
log "done: $OUT"
