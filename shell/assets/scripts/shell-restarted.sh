#!/usr/bin/env bash
# The shell had to be started again after stopping (keep-shell.sh, and the
# TTY session's watchdog, tty-shell.sh): written down, and when it's the
# third time in 15 minutes soon after an update, the recovery window opens
# (recover.sh: go back to before it), once a session.
#   shell-restarted.sh <session folder> <launch arguments, e.g. -c tessera>
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIR=$1
shift
[[ -d "$DIR" ]] || exit 0
now=$(date +%s)
recent=$( { cat "$DIR/tessera-shell.restarts" 2>/dev/null; echo "$now"; } | awk -v n="$now" '$1 > n - 900')
printf '%s\n' "$recent" >"$DIR/tessera-shell.restarts"
if [[ $(printf '%s\n' "$recent" | grep -c .) -ge 3 && ! -e "$DIR/tessera-recovery.shown" ]] && "$HERE/recover.sh" --check; then
    : >"$DIR/tessera-recovery.shown"
    setsid -f "$HERE/recover.sh" "$@" >/dev/null 2>&1 6>&- 8>&- 9>&-
fi
exit 0
