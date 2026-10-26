#!/usr/bin/env bash
# Runs a long-lived watcher (pactl subscribe, udevadm monitor, nmcli monitor,
# a polling loop...) so that it always ends with the shell:
#   - SIGPIPE back to its default: Quickshell ignores SIGPIPE and children
#     inherit that, so a watcher would never notice its reader is gone and
#     would run (and hold its connection) forever
#   - the watcher gets SIGTERM if this wrapper dies, even by SIGKILL
#   - this wrapper ends when its stdin closes (the shell quit) and takes the
#     watcher with it
# Usage: watch.sh <command> [args...]   (the Process needs stdinEnabled: true)
cmd=(env --default-signal=PIPE "$@")
command -v setpriv >/dev/null 2>&1 && cmd=(setpriv --pdeathsig TERM -- "${cmd[@]}")
"${cmd[@]}" &
w=$!
trap 'kill "$w" 2>/dev/null' EXIT
while read -t 5 _ || [ $? -gt 128 ]; do
    kill -0 "$w" 2>/dev/null || exit 1
done
