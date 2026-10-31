#!/usr/bin/env bash
# Keeps the iRice shell running in this Hyprland session. Hyprland runs it
# every 30 s (execs.lua); if this session has had no iRice shell for two
# checks in a row (so not during a restart or a repair), it starts one.
# Starts, repairs and this check share one lock, so they never race into
# duplicate bars. A shell that stopped three times in 15 minutes soon after an
# update gets the recovery screen too (recover.sh: go back to before it).
#   keep-shell.sh <launch arguments, e.g. -c irice>
set -uo pipefail
SIG="${HYPRLAND_INSTANCE_SIGNATURE:-}"
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
DIR="$RUN/hypr/$SIG"
[[ -n "$SIG" && -d "$DIR" ]] || exit 0
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

running() {
    local p
    for p in $(pgrep -x qs; pgrep -x quickshell); do
        { tr '\0' '\n' <"/proc/$p/environ"; } 2>/dev/null | grep -qx "HYPRLAND_INSTANCE_SIGNATURE=$SIG" || continue
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -qi irice && return 0
    done
    return 1
}

exec 9>"$DIR/irice-shell.lock"
flock -n 9 || exit 0 # a start or a repair is under way
if running; then
    rm -f "$DIR/irice-shell.missing"
    exit 0
fi
# First miss: wait for the next check before acting
if [[ ! -e "$DIR/irice-shell.missing" ]]; then
    : >"$DIR/irice-shell.missing"
    exit 0
fi
rm -f "$DIR/irice-shell.missing"
# The restarts of the last 15 minutes, written down
now=$(date +%s)
recent=$( { cat "$DIR/irice-shell.restarts" 2>/dev/null; echo "$now"; } | awk -v n="$now" '$1 > n - 900')
printf '%s\n' "$recent" >"$DIR/irice-shell.restarts"
if [[ $(printf '%s\n' "$recent" | grep -c .) -ge 3 && ! -e "$DIR/irice-recovery.shown" ]] && "$HERE/recover.sh" --check; then
    : >"$DIR/irice-recovery.shown" # (once a session)
    setsid -f "$HERE/recover.sh" "$@" >/dev/null 2>&1 9>&-
fi
setsid -f "$HERE/launch-shell.sh" "$@" >/dev/null 2>&1 9>&-
sleep 2 # let it register before the lock is released
