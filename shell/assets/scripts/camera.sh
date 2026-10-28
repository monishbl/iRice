#!/usr/bin/env bash
# Which apps have a camera open (services/Privacy.qml, the green dot), run
# under watch.sh. Prints "[app1,app2]" whenever the set changes ("[]" when
# none), once at the start too.
#
# Finding the apps (fuser) reads every process's open files, about a tenth
# of a second of CPU, so it only runs when a camera could be in use:
#   - a USB camera that's runtime-suspended (power/runtime_status, its power
#     management on) has nobody using it: no scan. Reading that is free.
#   - otherwise (awake, or power management off so the state says nothing):
#     scan.
# Between looks it waits for inotifywait to report a camera being opened or
# closed (no CPU at all meanwhile); without inotify-tools, it looks every 5 s
# (10 s while one is awake).
# Cameras plugged in later are picked up within 30 s. No cameras: a look
# every 30 s.
#
# It ends when the shell that started it is gone.
parent=$PPID
prev=x
devs=()
pm=() # each camera's USB device, for its power state

find_cameras() {
    devs=()
    pm=()
    local v d
    for v in /dev/video*; do
        [ -e "$v" ] || continue
        devs+=("$v")
        d=$(readlink -f "/sys/class/video4linux/${v##*/}/device" 2>/dev/null)
        # up from the interface to the USB device that carries power/
        while [ -n "$d" ] && [ "$d" != / ] && [ ! -r "$d/power/runtime_status" ]; do d=${d%/*}; done
        [ -n "$d" ] && [ "$d" != / ] && pm+=("$d/power") || pm+=("")
    done
}
# 0 when a camera may be in use: one is awake, or one can't tell
maybe_busy() {
    local p status control
    for p in "${pm[@]}"; do
        [ -n "$p" ] || return 0
        read -r control <"$p/control" 2>/dev/null || return 0
        [ "$control" = auto ] || return 0
        read -r status <"$p/runtime_status" 2>/dev/null || return 0
        [ "$status" = suspended ] || return 0
    done
    return 1
}
scan() {
    fuser "${devs[@]}" 2>/dev/null | tr -s ' ' '\n' | grep -o '[0-9]\+' | sort -u |
        while read -r p; do cat "/proc/$p/comm" 2>/dev/null; done | sort -u | paste -sd,
}

found=-30
while kill -0 "$parent" 2>/dev/null; do
    if [ $((SECONDS - found)) -ge 30 ]; then
        find_cameras
        found=$SECONDS
    fi
    cur=""
    [ ${#devs[@]} -gt 0 ] && maybe_busy && cur=$(scan)
    if [ "$cur" != "$prev" ]; then
        echo "[$cur]" || exit 0
        prev=$cur
    fi
    if [ ${#devs[@]} -eq 0 ]; then
        sleep 30
    elif command -v inotifywait >/dev/null 2>&1; then
        # An open or close (or 30 s, to notice new cameras), then let a burst settle
        inotifywait -qq -t 30 -e open -e close_write -e close_nowrite "${devs[@]}" 2>/dev/null
        sleep 0.3
    elif ! maybe_busy; then
        sleep 5 # (no inotify-tools; all asleep: checking that again is free)
    else
        sleep 10
    fi
done
