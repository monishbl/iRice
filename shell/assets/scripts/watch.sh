#!/usr/bin/env bash
# Runs a long-lived watcher (pactl subscribe, udevadm monitor, nmcli monitor,
# a polling loop...) so that it always ends with the shell:
#   - SIGPIPE back to its default: Quickshell ignores SIGPIPE and children
#     inherit that, so a watcher would never notice its reader is gone and
#     would run (and hold its connection) forever
#   - the watcher runs in a process group of its own, and the whole group is
#     ended when this wrapper ends: a watcher that's a pipeline or starts
#     helpers (pw-dump | while …) used to leave those running, orphaned and
#     busy, every time the shell restarted
#   - the watcher's leader gets SIGTERM if this wrapper dies, even by SIGKILL
#   - this wrapper ends when its stdin closes (the shell quit)
#   - each group is written down in $XDG_RUNTIME_DIR/irice-watchers/<group>
#     (this wrapper and the group's leader, with their start times), so a
#     new shell can end any group whose wrapper is gone (reap-watchers.sh).
#     Nothing is marked in the watcher's environment: a daemon a watcher
#     happens to start (a sound server, say) isn't one of ours to end.
# Usage: watch.sh <command> [args...]   (the Process needs stdinEnabled: true)
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
REC="$RUN/irice-watchers"
started() { awk '{ sub(/^.*\) /, ""); print $20 }' "/proc/$1/stat" 2>/dev/null; }

cmd=(env --default-signal=PIPE "$@")
command -v setpriv >/dev/null 2>&1 && cmd=(setpriv --pdeathsig TERM -- "${cmd[@]}")
set -m # (background jobs get their own process group: the group is the watcher)
"${cmd[@]}" </dev/null &
w=$!
set +m
lead=$(started "$w")
mkdir -p "$REC" 2>/dev/null
printf '%s %s %s %s\n' "$$" "$(started $$)" "$w" "$lead" >"$REC/$w" 2>/dev/null

# The group still exists and is still ours: some process in it that started
# no earlier than its leader (a group id is only reused once it's empty)
ours() {
    [[ -n "$lead" ]] || return 1
    awk -v g="$w" -v l="$lead" '{ s = $0; sub(/^.*\) /, "", s); split(s, f, " "); if (f[3] == g && f[20] >= l) { found = 1; exit } }
        END { exit !found }' /proc/[0-9]*/stat 2>/dev/null
}
end() {
    if ours; then
        kill -TERM -- -"$w" 2>/dev/null
        sleep 0.5
        ours && kill -KILL -- -"$w" 2>/dev/null
    fi
    rm -f "$REC/$w"
}
trap end EXIT
# Ended by a signal (the shell's group killed on a restart), bash skips the
# EXIT trap unless the signal is caught: catch them so the group still goes
trap 'exit 143' TERM HUP INT
while read -t 5 _ || [ $? -gt 128 ]; do
    kill -0 "$w" 2>/dev/null || exit 1
done
