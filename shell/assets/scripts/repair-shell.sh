#!/usr/bin/env bash
# Repair the iRice shell without touching your apps or windows.
#
# Fixes duplicated bars and docks, a frozen or half-drawn shell, and leftovers
# from an earlier session that ended badly:
#   1. stops iRice shells (and tty test-session watchdogs) whose compositor
#      is gone: they latch onto the next session's display and draw twice
#   2. replaces this session's shell(s) with exactly one fresh shell, started
#      with the same command and environment as before
#   3. clears glass shapes left in the compositor plugin (the new shell
#      sends its own)
# Settings, layout, clipboard and the rest live in files, so nothing is lost.
# Run from the system menu (Repair Shell), Super+Ctrl+Escape, or a terminal.
# Only iRice is touched: other Quickshell configs (like end4's) are left alone.
set -uo pipefail

RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
SIG="${HYPRLAND_INSTANCE_SIGNATURE:-}"
SHELL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG="$RUN/irice-repair.log"
exec >>"$LOG" 2>&1
echo "== $(date '+%F %T') repair (session ${SIG:-unknown})"

env_of() { { tr '\0' '\n' <"/proc/$1/environ"; } 2>/dev/null; }
var_of() { env_of "$1" | sed -n "s/^$2=//p" | head -1; }
cmd_of() { tr '\0' ' ' <"/proc/$1/cmdline" 2>/dev/null; }

# Is the compositor with this signature (under this runtime dir) still there?
alive() {
    local sig="$1" run="${2:-$RUN}"
    [[ -n "$sig" && -S "$run/hypr/$sig/.socket.sock" ]] &&
        XDG_RUNTIME_DIR="$run" HYPRLAND_INSTANCE_SIGNATURE="$sig" timeout 3 hyprctl version >/dev/null 2>&1
}

# Is this Quickshell process an iRice shell?
is_irice() {
    local c
    c="$(cmd_of "$1")"
    [[ "$c" =~ (^|[[:space:]])(-c|--config)[[:space:]]+irice([[:space:]]|$) ]] && return 0
    [[ "$c" == *" -p $SHELL_DIR "* || "$c" == *" --path $SHELL_DIR "* ]] && return 0
    [[ "$c" =~ -p[[:space:]]+[^[:space:]]*/[Ii][Rr]ice/shell([[:space:]]|$) ]] && return 0
    env_of "$1" | grep -qx "IRICE_TTY=1"
}

shells() {
    local p
    for p in $(pgrep -x qs; pgrep -x quickshell); do
        is_irice "$p" && echo "$p"
    done | sort -n
}

# 1. Leftovers from sessions that have ended
removed=0
for p in $(shells); do
    s="$(var_of "$p" HYPRLAND_INSTANCE_SIGNATURE)"
    r="$(var_of "$p" XDG_RUNTIME_DIR)"
    # No readable session (e.g. a host process seen from a container): not ours to judge
    [[ -z "$s" || "$s" == "$SIG" ]] && continue
    if ! alive "$s" "${r:-$RUN}"; then
        echo "stale shell $p (session $s)"
        kill -9 "$p" 2>/dev/null && removed=$((removed + 1))
    fi
done
for p in $(pgrep -f "tty-shell.sh watchdog"); do
    s="$(var_of "$p" HYPRLAND_INSTANCE_SIGNATURE)"
    if [[ -n "$s" && "$s" != "$SIG" ]] && ! alive "$s" "$(var_of "$p" XDG_RUNTIME_DIR)"; then
        echo "stale watchdog $p"
        kill "$p" 2>/dev/null
    fi
done

# 2. This session: exactly one fresh shell
mine=()
for p in $(shells); do
    [[ "$(var_of "$p" HYPRLAND_INSTANCE_SIGNATURE)" == "$SIG" ]] && mine+=("$p")
done
echo "this session's shells: ${mine[*]:-none}"
extra=$(( ${#mine[@]} > 1 ? ${#mine[@]} - 1 : 0 ))

# Start the new one the way the old one was started (same command, same
# environment: sandbox paths, the tty marker...), else the usual way
template="${mine[0]:-}"
if [[ -n "$template" ]]; then
    mapfile -d '' tenv <"/proc/$template/environ"
    mapfile -d '' tcmd <"/proc/$template/cmdline"
else
    tenv=()
    tcmd=()
fi
tty_ctl=""
if [[ -n "$template" ]] && env_of "$template" | grep -qx "IRICE_TTY=1"; then
    tty_ctl="$(var_of "$template" IRICE_ROOT)/sandbox/tty-shell.sh"
    [[ -x "$tty_ctl" ]] || tty_ctl=""
fi

# Hold the shell lock while swapping (keep-shell.sh mustn't start one in the
# gap); the test session's own starter takes it itself
if [[ -z "$tty_ctl" && -d "$RUN/hypr/$SIG" ]]; then
    exec 7>"$RUN/hypr/$SIG/irice-shell.lock"
    flock -w 10 7
fi

if [[ ${#mine[@]} -gt 0 ]]; then
    kill "${mine[@]}" 2>/dev/null
    for _ in $(seq 20); do
        sleep 0.1
        left=()
        for p in "${mine[@]}"; do kill -0 "$p" 2>/dev/null && left+=("$p"); done
        [[ ${#left[@]} -eq 0 ]] && break
    done
    [[ ${#left[@]} -gt 0 ]] && kill -9 "${left[@]}" 2>/dev/null # frozen
fi

# 3. Glass shapes from the old shells (the new one sends its own)
alive "$SIG" && hyprctl irice-glass clear >/dev/null 2>&1

if [[ -n "$tty_ctl" ]]; then
    env -i "${tenv[@]}" "$tty_ctl" start # the test session's own starter (locked, no duplicates)
elif [[ ${#tcmd[@]} -gt 0 ]]; then
    # Same arguments through the launcher, which picks the icon theme afresh
    # (so a theme changed in Settings applies)
    fresh=()
    for e in "${tenv[@]}"; do [[ $e == QS_ICON_THEME=* ]] || fresh+=("$e"); done
    env -i "${fresh[@]}" setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" "${tcmd[@]:1}" >/dev/null 2>&1 7>&-
elif [[ -n "${IRICE_ROOT:-}" && -f "$IRICE_ROOT/shell/shell.qml" ]]; then
    setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -p "$IRICE_ROOT/shell" >/dev/null 2>&1 7>&-
else
    setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -c irice >/dev/null 2>&1 7>&-
fi
sleep 2 # let it register, then let keep-shell.sh look again
exec 7>&-
echo "started a fresh shell; removed $removed stale and $extra duplicate"

# Tell the user once the new shell can show it
sleep 4
msg="Your windows and apps weren't touched."
(( removed + extra > 0 )) && msg="Removed $((removed + extra)) extra $( (( removed + extra == 1 )) && echo shell || echo shells). $msg"
notify-send -a iRice -i view-refresh "Shell repaired" "$msg" 2>/dev/null || true
