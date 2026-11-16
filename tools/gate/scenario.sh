#!/usr/bin/env bash
# The gate's interaction script (tools/gate). Runs INSIDE one arena
# (`./dev.sh run bash tools/gate/scenario.sh …`: the arena's runtime dir,
# Wayland display and Hyprland), so every step is a local call, not a
# distrobox/ssh round trip, and the timing is the same for every run.
#   scenario.sh <events.tsv> <tessera-ui binary> <reps> <part…>
# part: idle:<seconds>:<count> | reps | hz120 | scale15 | lock
# Each step writes "<wall time>\t<event>\t<detail>" to events.tsv; the
# analysis (analyze.py) cuts the shell's log and samples by these.
# Windows: the terminal it opens is alacritty (kitty, foot if not), at a
# fixed size, closed by its address; never a pattern kill.
set -uo pipefail
EV=$1 UI=$2 REPS=$3
shift 3
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VPTR="$ROOT/.sandbox/bin/vptr"
OUTPUT=IRICE-TEST

mark() { printf '%s\t%s\t%s\n' "$(date +%s.%N)" "$1" "${2:-}" >>"$EV"; }
ipc() { "$UI" ipc call "$@" >/dev/null 2>&1; }
P() { "$VPTR" -o "$OUTPUT" "$@" >/dev/null 2>&1; }
hc() { hyprctl "$@" >/dev/null 2>&1; }
dsp() { hyprctl dispatch "$1" >/dev/null 2>&1; }
# (the output's mode, scale; the position keeps it out of WAYLAND-1's way)
mode() { hc eval "hl.monitor({ output = \"$OUTPUT\", mode = \"$1\", position = \"20000x20000\", scale = $2 })"; }

TERM_CMD=()
if command -v alacritty >/dev/null; then
    TERM_CMD=(alacritty -o window.dimensions.columns=90 -o window.dimensions.lines=28 -e sleep 600)
elif command -v kitty >/dev/null; then
    TERM_CMD=(kitty -o initial_window_width=90c -o initial_window_height=28c sleep 600)
elif command -v foot >/dev/null; then
    TERM_CMD=(foot -W 90x28 sleep 600)
fi
# A window's address by its pid (waits up to 4 s for it to map)
addr_of() {
    local i a
    for i in $(seq 40); do
        a=$(hyprctl clients -j 2>/dev/null | python3 -I -c 'import json,sys; p=int(sys.argv[1]); print(next((c["address"] for c in json.load(sys.stdin) if c.get("pid")==p), ""))' "$1")
        [[ -n $a ]] && { echo "$a"; return 0; }
        sleep 0.1
    done
    return 1
}
TPIDS=()
term_open() { # → the new window's address on stdout
    [[ ${#TERM_CMD[@]} -gt 0 ]] || return 1
    setsid -f "${TERM_CMD[@]}" >/dev/null 2>&1 </dev/null
    # (setsid -f: the terminal's pid is the newest of ours with this command)
    local p
    for _ in $(seq 30); do
        p=$(pgrep -nx -u "$(id -u)" "${TERM_CMD[0]}") && grep -qzx "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR" "/proc/$p/environ" 2>/dev/null && break
        p=""; sleep 0.1
    done
    [[ -n $p ]] || return 1
    TPIDS+=("$p")
    local a
    a=$(addr_of "$p") || { mark note "terminal $p didn't map"; return 1; }
    mark window "$a"
    echo "$a"
}
ws_of() { hyprctl clients -j 2>/dev/null | python3 -I -c 'import json,sys; print(next((c["workspace"]["name"] for c in json.load(sys.stdin) if c["address"]==sys.argv[1]), "gone"))' "$1"; }
term_close() { dsp "hl.dsp.window.close({ window = \"address:$1\" })"; }
# (any terminal of ours still there at the end: its exact pid)
cleanup() {
    local p
    for p in "${TPIDS[@]}"; do
        grep -qzx "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR" "/proc/$p/environ" 2>/dev/null && kill "$p" 2>/dev/null
    done
}
trap cleanup EXIT

panel() { # name hold-seconds
    mark panel "$1"
    ipc panels toggle "$1"; sleep "$2"
    ipc panels close; sleep 0.6
}
dock_hover() { # y: the dock's row in output pixels
    mark dock "$1"
    P move 960 500 glide 420 "$1" 250 glide 1500 "$1" 1400 glide 420 "$1" 1400 glide 960 500 250
    sleep 0.8
}
spotlight() {
    mark spotlight
    ipc panels toggle spotlight; sleep 0.9
    wtype -d 40 "terminal"; sleep 1.2
    wtype -k BackSpace -k BackSpace -k BackSpace; wtype -d 40 "set"; sleep 1.0
    ipc panels close; sleep 0.8
}
settings() {
    mark settings
    ipc panels openSettings appearance; sleep 1.6
    for p in dock wallpaper notifications displays; do ipc panels openSettings "$p"; sleep 1.0; done
    ipc panels toggleSettings; sleep 1.0
}
windows() {
    mark windows
    local a b
    a=$(term_open) || { mark note "no terminal"; return; }
    sleep 1.2
    b=$(term_open) || b=""
    sleep 1.2
    # minimize the second (the shell's own way: to special:minimized), restore it
    if [[ -n $b ]]; then
        mark minimize
        dsp "hl.dsp.window.move({ workspace = \"special:minimized\", window = \"address:$b\", follow = false })"; sleep 1.2
        mark restore "$(ws_of "$b")"
        dsp "hl.dsp.window.move({ workspace = \"1\", window = \"address:$b\", follow = false })"; sleep 1.2
        mark restored "$(ws_of "$b")"
    fi
    mark workspaces
    dsp 'hl.dsp.focus({ workspace = 2 })'; sleep 1.0
    dsp 'hl.dsp.focus({ workspace = 3 })'; sleep 1.0
    dsp 'hl.dsp.focus({ workspace = 1 })'; sleep 1.0
    term_close "$a"; [[ -n $b ]] && term_close "$b"
    sleep 1.2
}
one_rep() { # n
    mark rep-start "$1"
    for p in controlCenter notificationCenter launchpad wifiMenu missionControl; do panel "$p" 1.1; done
    spotlight
    settings
    dock_hover 1045
    windows
    sleep 2
    mark rep-end "$1"
}
short_pass() { # dock row
    for p in controlCenter launchpad missionControl; do panel "$p" 1.1; done
    dock_hover "$1"
    local a
    a=$(term_open) && { sleep 1.2; term_close "$a"; sleep 1.2; }
}

ipc lock unlock; ipc panels close
for part in "$@"; do
    case $part in
        idle:*)
            IFS=: read -r _ secs count <<<"$part"
            for i in $(seq "$count"); do mark idle-start "$i"; sleep "$secs"; mark idle-end "$i"; done ;;
        reps)
            mark script-start
            for r in $(seq "$REPS"); do one_rep "$r"; done ;;
        hz120)
            mark hz120-start
            mode 1920x1080@120 1; sleep 3
            short_pass 1045
            mode 1920x1080@60 1; sleep 3
            mark hz120-end ;;
        scale15)
            mark scale15-start
            mode 1920x1080@60 1.5; sleep 3
            short_pass 1030
            mode 1920x1080@60 1; sleep 3
            mark scale15-end ;;
        lock)
            mark lock-start
            ipc lock lock; sleep 2.5
            wtype -d 40 mock; wtype -k Return; sleep 3
            # (in case the password wasn't taken: unlocking an unlocked
            # shell does nothing, so the rest never runs behind a lock)
            ipc lock unlock; sleep 1
            mark lock-end
            mark script-end ;;
    esac
done
mark done
