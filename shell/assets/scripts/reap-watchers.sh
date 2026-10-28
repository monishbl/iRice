#!/usr/bin/env bash
# Run once when the shell starts: ends iRice watchers left behind by an
# earlier shell (it crashed, or its wrapper was killed outright).
#   reap-watchers.sh [--dry-run]    --dry-run lists them and ends nothing
#
# watch.sh writes each watcher group down in $XDG_RUNTIME_DIR/irice-watchers/
# <group>: its own pid and start time, and the group leader's. A group is
# left over when that wrapper is gone (or its pid now belongs to another
# process); it's ended only if the group still exists and is still the same
# one (a member started no earlier than its leader: group ids are reused
# once empty). Groups whose wrapper lives are in use and never touched.
# Nothing is found by environment marks: a daemon a watcher happened to
# start isn't one of ours to end.
#
# Copies from before those records (screenshare.sh and transfers.sh, which
# used to pile up) are found by being orphaned: their parent is init or the
# user's service manager, not a watch.sh. Other iRice scripts run detached
# on purpose (a system update, a recording, Quit) and are never touched.
dry=0
[ "${1:-}" = --dry-run ] && dry=1
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
REC="$RUN/irice-watchers"
me=$(id -u)

started() { awk '{ sub(/^.*\) /, ""); print $20 }' "/proc/$1/stat" 2>/dev/null; }
ppid_of() { awk '{ sub(/^.*\) /, ""); print $2 }' "/proc/$1/stat" 2>/dev/null; }
cmd_of() { { tr '\0' ' ' <"/proc/$1/cmdline"; } 2>/dev/null; }
mine() { [ "$(stat -c %u "/proc/$1" 2>/dev/null)" = "$me" ]; }

# Members of group $1 that started no earlier than $2 (its leader)
members() {
    local p
    for p in $(awk -v g="$1" -v l="$2" '{ s = $0; sub(/^.*\) /, "", s); split(s, f, " ")
            if (f[3] == g && f[20] >= l) { n = FILENAME; gsub(/[^0-9]/, "", n); print n } }' /proc/[0-9]*/stat 2>/dev/null); do
        mine "$p" && echo "$p"
    done
}

# 1. Recorded groups whose wrapper is gone
if [ -d "$REC" ]; then
    for f in "$REC"/*; do
        [ -f "$f" ] || continue
        read -r wpid wstart grp gstart <"$f" 2>/dev/null
        case "$wpid$wstart$grp$gstart" in *[!0-9]* | "") [ $dry = 1 ] || rm -f "$f"; continue ;; esac
        [ "$(started "$wpid")" = "$wstart" ] && continue # its wrapper lives: in use
        m=$(members "$grp" "$gstart")
        if [ -n "$m" ]; then
            if [ $dry = 1 ]; then
                echo "would end watcher group $grp ($(cmd_of "$grp" | cut -c1-70)): $(echo $m | wc -w) processes"
                continue
            fi
            kill -TERM -- -"$grp" 2>/dev/null
            sleep 0.3
            [ -n "$(members "$grp" "$gstart")" ] && kill -KILL -- -"$grp" 2>/dev/null
        fi
        [ $dry = 1 ] || rm -f "$f"
    done
fi

# 2. Copies from before the records: orphaned screenshare.sh / transfers.sh,
#    with everything under them
tree() {
    local p=$1 c
    echo "$p"
    for c in $(pgrep -P "$p"); do tree "$c"; done
}
for d in /proc/[0-9]*; do
    p=${d#/proc/}
    [ "$p" = "$$" ] && continue
    mine "$p" || continue
    cmd_of "$p" | grep -qE '/shell/assets/scripts/(screenshare|transfers)\.sh' || continue
    pp=$(ppid_of "$p")
    [ -n "$pp" ] || continue
    if [ "$pp" != 1 ]; then
        # under a watch.sh (in use) or a script's own subshell: leave it;
        # under the user's systemd (a subreaper): left over
        [ "$(cat "/proc/$pp/comm" 2>/dev/null)" = systemd ] || continue
    fi
    if [ $dry = 1 ]; then
        echo "would end leftover $p ($(cmd_of "$p" | cut -c1-70)) and $(($(tree "$p" | wc -l) - 1)) under it"
    else
        kill -TERM $(tree "$p") 2>/dev/null
    fi
done
exit 0
