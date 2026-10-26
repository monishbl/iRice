#!/usr/bin/env bash
# Shell control for the ./dev.sh tty session (runs inside it, started by tty.lua).
#   tty-shell.sh start     start the shell (no-op if this session already has one)
#   tty-shell.sh restart   kill the shell (hard, if it's frozen) and start it again
#   tty-shell.sh watchdog  restart the shell whenever it stops answering IPC
# The shell runs on the host's Quickshell with config/state/cache in the
# sandbox home, so your real ~/.config is untouched.
#
# A new session reuses the Wayland socket of an old one, so everything here is
# tied to this compositor (HYPRLAND_INSTANCE_SIGNATURE): the watchdog exits
# when its compositor is gone, and shells of dead sessions are cleaned up,
# otherwise they'd pile onto the next session as duplicate bars and docks.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SB="$ROOT/.sandbox/home"
export IRICE_SANDBOX=1 IRICE_ROOT="$ROOT" IRICE_TTY=1
export XDG_CONFIG_HOME="$SB/.config" XDG_STATE_HOME="$SB/.local/state" XDG_CACHE_HOME="$SB/.cache" XDG_DATA_HOME="$SB/.local/share"
SHELL_DIR="$ROOT/shell"
SIG="${HYPRLAND_INSTANCE_SIGNATURE:?not inside a Hyprland session}"
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
LOCKDIR="$RUN/hypr/$SIG"

# Is this session's compositor still there?
alive() {
    [[ -S "$LOCKDIR/.socket.sock" ]] && timeout 3 hyprctl version >/dev/null 2>&1
}

sig_of() { { tr '\0' '\n' <"/proc/$1/environ"; } 2>/dev/null | sed -n 's/^HYPRLAND_INSTANCE_SIGNATURE=//p'; }
is_tty_shell() { { tr '\0' '\n' <"/proc/$1/environ"; } 2>/dev/null | grep -qx "IRICE_TTY=1"; }

# This session's shells, oldest first
shell_pids() {
    local p
    for p in $(pgrep -x qs; pgrep -x quickshell); do
        is_tty_shell "$p" && [[ "$(sig_of "$p")" == "$SIG" ]] && echo "$p"
    done | sort -n
    return 0
}

# Test-session shells whose compositor has gone (left over from earlier sessions)
reap() {
    local p s
    for p in $(pgrep -x qs; pgrep -x quickshell); do
        is_tty_shell "$p" || continue
        s="$(sig_of "$p")"
        [[ "$s" == "$SIG" ]] && continue
        [[ -S "$RUN/hypr/$s/.socket.sock" ]] && HYPRLAND_INSTANCE_SIGNATURE="$s" timeout 3 hyprctl version >/dev/null 2>&1 && continue
        kill -9 "$p" 2>/dev/null
    done
}

start() {
    alive || return 0
    (
        flock -n 9 || exit 0 # another start is running
        [[ -n "$(shell_pids)" ]] && exit 0
        # The locks' descriptors must not leak into the shell (and the apps it
        # opens), or they'd hold the locks and block every later start
        setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -p "$SHELL_DIR" >/dev/null 2>&1 8>&- 9>&-
        sleep 2 # let it register before the lock is released
    ) 9>"$LOCKDIR/irice-shell.lock"
}

stop() {
    local pids
    pids="$(shell_pids)"
    [[ -z "$pids" ]] && return 0
    kill $pids 2>/dev/null
    for _ in $(seq 20); do
        sleep 0.1
        pids="$(for p in $pids; do kill -0 "$p" 2>/dev/null && echo "$p"; done)"
        [[ -z "$pids" ]] && return 0
    done
    kill -9 $pids 2>/dev/null || true # frozen: doesn't handle SIGTERM
}

case "${1:-start}" in
    start) start ;;
    restart) stop; start ;;
    watchdog)
        exec 8>"$LOCKDIR/irice-watchdog.lock"
        flock -n 8 || exit 0 # this session already has one
        reap
        sleep 20 # let it start
        misses=0
        while sleep 10; do
            if ! alive; then
                stop # the session ended: don't outlive it
                exit 0
            fi
            pids=($(shell_pids))
            if (( ${#pids[@]} == 0 )); then
                start # crashed
                misses=0
                sleep 10
                continue
            fi
            if (( ${#pids[@]} > 1 )); then
                kill "${pids[@]:1}" 2>/dev/null # keep the oldest
            fi
            if timeout 4 qs ipc --pid "${pids[0]}" show >/dev/null 2>&1; then
                misses=0
            elif (( ++misses >= 2 )); then
                notify-send -a iRice "Restarting the shell" "It stopped responding." 2>/dev/null
                stop; start
                misses=0
                sleep 10
            fi
        done
        ;;
esac
