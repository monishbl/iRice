#!/usr/bin/env bash
# Keeps the Tessera shell running in this Hyprland session. Hyprland runs it
# every 30 s (execs.lua); if this session has had no Tessera shell for two
# checks in a row (so not during a restart or a repair), it starts one.
# Starts, repairs and this check share one lock, so they never race into
# duplicate bars. A shell that stopped three times in 15 minutes soon after an
# update gets the recovery screen too (recover.sh: go back to before it).
#   keep-shell.sh <launch arguments, e.g. -c tessera>
set -uo pipefail
SIG="${HYPRLAND_INSTANCE_SIGNATURE:-}"
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
DIR="$RUN/hypr/$SIG"
[[ -n "$SIG" && -d "$DIR" ]] || exit 0
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

running() {
    local p
    for p in $(pgrep -x tessera-shell; pgrep -x irice-shell; pgrep -x qs; pgrep -x quickshell); do
        { tr '\0' '\n' <"/proc/$p/environ"; } 2>/dev/null | grep -qx "HYPRLAND_INSTANCE_SIGNATURE=$SIG" || continue
        # (-c tessera, or -c irice: started before iRice became Tessera)
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -qi -e tessera -e irice && return 0
    done
    # Standalone (rust.standalone): tessera-ui is the shell. Only one started
    # with --standalone counts: the one the QML shell starts as its helper ends with it.
    for p in $(pgrep -x tessera-ui); do
        { tr '\0' '\n' <"/proc/$p/environ"; } 2>/dev/null | grep -qx "HYPRLAND_INSTANCE_SIGNATURE=$SIG" || continue
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -q -e '--standalone' && return 0
    done
    return 1
}

exec 9>"$DIR/tessera-shell.lock"
flock -n 9 || exit 0 # a start or a repair is under way
# (also the lock's name from before iRice became Tessera: a repair started
# by the old scripts may still run in this session)
exec 6>"$DIR/irice-shell.lock"
flock -n 6 || exit 0
if running; then
    rm -f "$DIR/tessera-shell.missing"
    exit 0
fi
# First miss: wait for the next check before acting
if [[ ! -e "$DIR/tessera-shell.missing" ]]; then
    : >"$DIR/tessera-shell.missing"
    exit 0
fi
rm -f "$DIR/tessera-shell.missing"
"$HERE/shell-restarted.sh" "$DIR" "$@" 9>&- 6>&- # (the third time soon after an update: the recovery window)
setsid -f "$HERE/launch-shell.sh" "$@" >/dev/null 2>&1 9>&- 6>&-
sleep 2 # let it register before the lock is released
