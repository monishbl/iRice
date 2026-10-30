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
#     (this wrapper and the group's leader, with their start times, and a
#     random token the watcher gets as IRICE_WATCH_TOKEN), so a new shell can
#     end any group whose wrapper is gone (reap-watchers.sh). Once the leader
#     has exited, only members carrying that token count as ours. A daemon a
#     watcher happens to start (a sound server, say) leaves the group, so it
#     isn't one of ours to end even though it inherited the token.
# Usage: watch.sh <command> [args...]   (the Process needs stdinEnabled: true)
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
REC="$RUN/irice-watchers"
started() { awk '{ sub(/^.*\) /, ""); print $20 }' "/proc/$1/stat" 2>/dev/null; }

token=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
cmd=(env --default-signal=PIPE IRICE_WATCH_TOKEN="$token" "$@")
command -v setpriv >/dev/null 2>&1 && cmd=(setpriv --pdeathsig TERM -- "${cmd[@]}")
set -m # (background jobs get their own process group: the group is the watcher)
"${cmd[@]}" </dev/null &
w=$!
set +m
lead=$(started "$w")
umask 077
mkdir -p "$REC" 2>/dev/null
printf '%s %s %s %s %s\n' "$$" "$(started $$)" "$w" "$lead" "$token" >"$REC/$w" 2>/dev/null

# Capture pidfds before signaling, so PID/PGID reuse cannot target other work.
end() {
    python3 -I "$(dirname -- "$0")/process-identity.py" group "$w" "$lead" "$token"
    rm -f "$REC/$w"
}
trap end EXIT
# Ended by a signal (the shell's group killed on a restart), bash skips the
# EXIT trap unless the signal is caught: catch them so the group still goes
trap 'exit 143' TERM HUP INT
while read -t 5 _ || [ $? -gt 128 ]; do
    kill -0 "$w" 2>/dev/null || exit 1
done
