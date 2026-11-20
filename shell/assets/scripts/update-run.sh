#!/usr/bin/env bash
# Restarts the shell into a Tessera update: Software Update's "Restart shell
# now", the screen locking, or (legacy) the whole update behind the screen.
#
#   update-run.sh <work dir> <repo> <config dir> [finish [locked]]
#
# finish   (Rust shell, since background updates): the update was already
#          made ready with the shell running (tools/tessera-update.py stage);
#          this only stops the shell, installs what was staged (renames and
#          copies: tessera-update.py finish, a few seconds) and starts the new
#          shell, behind the update screen for those seconds.
# locked   the same when the screen has just locked: no update screen (the
#          lock covers everything); the new shell takes the lock back
#          (allow_session_lock_restore, its locked mark).
# (none)   the QML shell's older way: the whole update (download, builds)
#          behind the full-screen screen.
#
# The work dir holds copies of this script and of the update screen (ui/),
# plus choices.json: the update rewrites the repository, so nothing running
# here may be read from it while it does. Started detached by
# services/TesseraUpdate.qml; it outlives the shell it replaces.
#
# Why the shell is stopped first: Quickshell reloads whenever its files
# change. Updating under a running shell reloaded it halfway (killing the
# update with it, and running half-new code without its rebuilt parts).
# The shell lock (tessera-shell.lock, shared with keep-shell.sh, repair-shell.sh
# and the tty session's watchdog) is held meanwhile, so nothing restarts it early.
# A repo-wide lock (.git/tessera-update.lock, also taken by tessera-update.py)
# makes it one update at a time on a repository, from any session; a stale
# one (its holder gone, or only inherited) is taken, never a dead end.
set -uo pipefail
T="$1" REPO="$2" CONFIG="$3" MODE="${4:-}" LOCKED="${5:-}"
[[ -n "$T" && -d "$T" && -n "$REPO" && -d "$REPO/.git" ]] || { echo "usage: update-run.sh <work dir> <repo> <config dir> [finish [locked]]" >&2; exit 2; }
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
SIG="${HYPRLAND_INSTANCE_SIGNATURE:-}"
SHELL_DIR="$REPO/shell"
exec >>"$T/log" 2>&1
echo "== $(date '+%F %T') update${MODE:+ ($MODE${LOCKED:+, $LOCKED})}"
# (the restart into a staged update: is it still good for this copy? If not,
# the shell isn't touched, and the answer goes into the background job's
# status, which the running shell reads every second while it waits for its
# restart: not the result file, read only when a shell starts)
if [[ $MODE == finish ]]; then
    ok=$(python3 -I "$REPO/tools/tessera-update.py" staged | python3 -c 'import json, sys; s = json.load(sys.stdin); print("yes" if s.get("ready") and s.get("valid") else "no")' 2>/dev/null)
    if [[ $ok != yes ]]; then
        python3 -I "$REPO/tools/tessera-update.py" discard >/dev/null 2>&1
        python3 -I - "${XDG_STATE_HOME:-$HOME/.local/state}/tessera/update-job" <<'PY'
import json, os, sys, time
d = sys.argv[1]
os.makedirs(d, exist_ok=True)
now = time.time()
st = {"state": "error", "error": "The prepared update no longer fits this copy of Tessera. Check for updates again.",
      "text": "The update didn't finish", "started": now, "finished": now}
with open(os.path.join(d, "status.json.tmp"), "w") as f:
    json.dump(st, f)
os.replace(os.path.join(d, "status.json.tmp"), os.path.join(d, "status.json"))
PY
        echo "nothing valid staged: the shell stays"
        exit 1
    fi
fi

status() { # state percent text [detail]
    printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}" >"$T/status.new" && mv -f "$T/status.new" "$T/status"
}

env_of() { { tr '\0' '\n' <"/proc/$1/environ"; } 2>/dev/null; }
var_of() { env_of "$1" | sed -n "s/^$2=//p" | head -1; }
is_tessera() {
    local c
    c="$(tr '\0' ' ' <"/proc/$1/cmdline" 2>/dev/null)"
    # An update screen (this one's or another's) is never the shell
    [[ "$c" == *"/tessera-update."* || "$c" == *" -p $T/ui"* ]] && return 1
    [[ "$c" =~ (^|[[:space:]])(-c|--config)[[:space:]]+tessera([[:space:]]|$) ]] && return 0
    [[ "$c" == *" -p $SHELL_DIR"* || "$c" == *" --path $SHELL_DIR"* ]] && return 0
    # (started by launch-shell.sh: the Material look runs from its own root)
    env_of "$1" | grep -qx "TESSERA_SHELL=1" && return 0
    env_of "$1" | grep -qx "TESSERA_TTY=1"
}
# This session's Tessera shells
shells() {
    local p
    # (tessera-ui --standalone too: the shell itself since rust.standalone became the default)
    for p in $(pgrep -x tessera-shell; pgrep -x qs; pgrep -x quickshell; for u in $(pgrep -x tessera-ui); do tr '\0' ' ' <"/proc/$u/cmdline" 2>/dev/null | grep -q -e ' --standalone' && echo "$u"; done); do
        is_tessera "$p" && [[ "$(var_of "$p" HYPRLAND_INSTANCE_SIGNATURE)" == "$SIG" ]] && echo "$p"
    done | sort -n
}

# The update screen: Rust's (tessera-ui --update-screen) when a tessera-ui
# is installed that has it (launch-shell.sh's lookup; an older one fails
# --probe), else Quickshell's (qs -p) from $1. Either way "-p $T/ui" is in
# its command line, which the checks for the screen here look for (is_tessera,
# the wait at the end), and "/tessera-update.", which every shell stopper skips.
screen() {
    local b
    for b in "${TESSERA_UI:-}" "${XDG_DATA_HOME:-$HOME/.local/share}/tessera/bin/tessera-ui" "$HOME/.local/share/tessera/bin/tessera-ui"; do
        [[ -n "$b" && -x "$b" ]] || continue
        if "$b" --update-screen --probe >/dev/null 2>&1 </dev/null; then
            SCREEN=("$b" --update-screen -p "$T/ui")
            return
        fi
        break
    done
    SCREEN=("$1" -p "$T/ui")
}

# One update at a time on this repository (another session may be updating
# it): nothing is touched unless this is the only one. The lock's file says
# who holds it (tessera-update.py's repo_lock reads it the same way); a
# stale lock (its holder gone, or kept only by a shell that inherited the
# descriptor) is taken apart by tessera-update.py lock-holder and taken.
LOCKF="$REPO/.git/tessera-update.lock"
take_repo_lock() {
    local _ start
    for _ in 1 2 3 4; do
        exec 8<>"$LOCKF" || return 1 # (<>: opening doesn't empty the holder's record)
        if flock -n 8; then
            # (still the file at the path: one replaced meanwhile isn't the lock)
            if [[ "$(stat -L -c %d:%i "/proc/$$/fd/8" 2>/dev/null)" == "$(stat -c %d:%i "$LOCKF" 2>/dev/null)" ]]; then
                start="$(sed 's/.*) //' "/proc/$$/stat" 2>/dev/null | cut -d' ' -f20)"
                printf '{"pid": %d, "start": %s, "what": "%s", "started": %d, "session": "%s", "log": "%s"}\n' \
                    "$$" "${start:-0}" "$([[ $MODE == finish ]] && echo restart || echo apply)" "$(date +%s)" "$SIG" "$T/log" >"$LOCKF"
                return 0
            fi
        fi
        exec 8>&-
        HOLDER="$(python3 -I "$REPO/tools/tessera-update.py" lock-holder ${MARK:+--mark} 2>/dev/null)"
        [[ $HOLDER == *'"stale": true'* || $HOLDER == *'"free": true'* ]] || return 1
    done
    return 1
}
MARK=$([[ $MODE == finish ]] && echo 1)
if ! take_repo_lock; then
    msg="$(printf '%s' "$HOLDER" | python3 -c 'import json, sys; print(json.load(sys.stdin).get("message", ""))' 2>/dev/null)"
    msg="${msg:-Software Update is already running. Wait for it to finish, then try again.}"
    echo "another update holds the repository lock: $msg"
    if [[ $MODE == finish ]]; then
        # (the restart: no screen, the shell stays; Software Update says who's
        # running, from the job's status: lock-holder --mark)
        exit 1
    fi
    status run 0 "Preparing"
    QS0="$(command -v qs || echo qs)"
    screen "$QS0"
    env -u TESSERA_TTY setsid -f "${SCREEN[@]}" >"$T/ui.log" 2>&1 </dev/null 8>&- 9>&-
    sleep 1.2
    status error 100 "Software Update is already running" "$msg"
    exit 1
fi

mine=($(shells))
template="${mine[0]:-}"
QS=qs
if [[ -n "$template" ]]; then
    QS="$(readlink -f "/proc/$template/exe")"
    mapfile -d '' tenv <"/proc/$template/environ"
    # (an older update-run.sh exported TESSERA_UPDATE_HELD: the shell it started
    # passed it on to every update after; never into the next shell)
    kept=()
    for e in "${tenv[@]}"; do [[ $e == TESSERA_UPDATE_HELD=* ]] || kept+=("$e"); done
    tenv=("${kept[@]}")
    mapfile -d '' tcmd <"/proc/$template/cmdline"
    # (the shell is tessera-ui --standalone: Quickshell, for a QML screen, is
    # the one on the PATH; the shell starts again the usual way, and the
    # launcher picks standalone again from the settings)
    if [[ "$(cat "/proc/$template/comm" 2>/dev/null)" == tessera-ui ]]; then
        QS="$(command -v qs || echo qs)"
        tcmd=()
        fresh=()
        for e in "${tenv[@]}"; do [[ $e == TESSERA_STANDALONE=* ]] || fresh+=("$e"); done
        tenv=("${fresh[@]}")
    fi
else
    tenv=()
    tcmd=()
fi
tty_ctl=""
if [[ -n "$template" ]] && env_of "$template" | grep -qx "TESSERA_TTY=1"; then
    tty_ctl="$REPO/sandbox/tty-shell.sh"
fi

# The update screen, from its copy here (not while locked: the lock covers
# everything, and a screen would only show once it's unlocked)
if [[ $MODE == finish ]]; then
    status restart 40 "Restarting Tessera"
else
    status run 0 "Preparing"
fi
if [[ -z $LOCKED ]]; then
    # (without TESSERA_TTY: in the tty session it would pass for the shell)
    screen "$QS"
    env -u TESSERA_TTY setsid -f "${SCREEN[@]}" >"$T/ui.log" 2>&1 </dev/null 8>&- 9>&-
    sleep 1.2 # (it fades in over the desktop before the shell goes)
fi

# Hold the shell lock, then stop the shell
lockdir="$RUN/hypr/$SIG"
mkdir -p "$lockdir"
exec 9>"$lockdir/tessera-shell.lock"
if ! flock -w 15 9; then
    # Something (a shell restarting, a repair) holds it: stopping the shell
    # now could leave two running, or one reloading mid-update. Nothing has
    # been touched yet.
    status error 100 "Tessera couldn't be stopped for the update" "Something was restarting the shell. Try again in a moment; nothing was changed."
    echo "shell lock busy: stopped before changing anything"
    exit 1
fi
# The shells to stop, found again now that nothing can start one meanwhile
mine=($(shells))
if [[ ${#mine[@]} -gt 0 ]]; then
    kill "${mine[@]}" 2>/dev/null
    for _ in $(seq 30); do
        sleep 0.1
        left=()
        for p in "${mine[@]}"; do kill -0 "$p" 2>/dev/null && left+=("$p"); done
        [[ ${#left[@]} -eq 0 ]] && break
    done
    [[ ${#left[@]} -gt 0 ]] && kill -9 "${left[@]}" 2>/dev/null
fi

# The update, its steps shown as they come (PROGRESS lines on stderr)
# (rebuild: parts to rebuild anyway, e.g. "plugin" after a Hyprland update)
# (finish: what was staged, swapped in; seconds)
if [[ $MODE == finish ]]; then
    UPDATE=(finish --config "$CONFIG")
else
    UPDATE=(apply --config "$CONFIG" --choices "$T/choices.json" --rebuild "$(cat "$T/rebuild" 2>/dev/null)")
fi
# (TESSERA_UPDATE_HELD=<this script's pid>: its child, the updater, knows the
# lock is held for it; nothing else this starts inherits that, the new shell
# least of all)
TESSERA_UPDATE_HELD=$$ python3 -I "$REPO/tools/tessera-update.py" "${UPDATE[@]}" \
    >"$T/result.json" 2> >(while IFS= read -r line; do
        echo "$line" >>"$T/log"
        [[ $line =~ ^PROGRESS\ ([0-9]+)\ (.*)$ ]] && status "$([[ $MODE == finish ]] && echo restart || echo run)" "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
    done)
code=$?
sleep 0.2 # (the progress reader's last lines)
err="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("error",""))' "$T/result.json" 2>/dev/null)"
backup="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("backup",""))' "$T/result.json" 2>/dev/null)"
[[ $code -ne 0 && -z "$err" ]] && err="The updater stopped unexpectedly (see $T/log)."
cp "$T/result.json" "$RUN/tessera-update-result.json" 2>/dev/null # for the new shell's Software Update page

# The new Hyprland config (keybinds, gestures, look), then exactly one shell
[[ -z "$err" ]] && hyprctl reload config-only >/dev/null 2>&1
# A config-only reload does not rerun the session-start D-Bus export.
if [[ -z "$err" ]] && command -v dbus-update-activation-environment >/dev/null; then
    QT_QPA_PLATFORMTHEME="${QT_QPA_PLATFORMTHEME:-kde}" dbus-update-activation-environment QT_QPA_PLATFORMTHEME >/dev/null 2>&1 || true
fi
status restart 92 "Restarting"
hyprctl tessera-glass clear >/dev/null 2>&1
if [[ -n "$tty_ctl" ]]; then
    exec 9>&- 6>&- 8>&- # the test session's starter takes the lock itself
    env -i "${tenv[@]}" "$tty_ctl" start
elif [[ ${#tcmd[@]} -gt 0 ]]; then
    fresh=()
    for e in "${tenv[@]}"; do [[ $e == QS_ICON_THEME=* ]] || fresh+=("$e"); done
    env -i "${fresh[@]}" setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" "${tcmd[@]:1}" >/dev/null 2>&1 6>&- 8>&- 9>&-
    sleep 2
    exec 9>&- 6>&-
else
    setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -c tessera >/dev/null 2>&1 6>&- 8>&- 9>&-
    sleep 2
    exec 9>&- 6>&-
fi

# Asking a shell: tessera-ui over its own socket, the QML one over Quickshell's IPC
shell_ask() { # pid target function
    if [[ "$(cat "/proc/$1/comm" 2>/dev/null)" == tessera-ui ]]; then
        timeout 3 "/proc/$1/exe" ipc call "$2" "$3" 2>/dev/null
    else
        timeout 3 "$QS" ipc --pid "$1" call "$2" "$3" 2>/dev/null
    fi
}
shell_answers() {
    if [[ "$(cat "/proc/$1/comm" 2>/dev/null)" == tessera-ui ]]; then
        timeout 3 "/proc/$1/exe" ipc show >/dev/null 2>&1
    else
        timeout 3 "$QS" ipc --pid "$1" show >/dev/null 2>&1
    fi
}

# Done once the new shell answers
up=0
for _ in $(seq 60); do
    p="$(shells | head -1)"
    if [[ -n "$p" ]] && shell_answers "$p"; then
        up=1
        break
    fi
    sleep 0.5
done
# …and has drawn: the wallpaper is up (the bar and dock fade in after it).
# Closing earlier left a black screen until it did.
if [[ $up == 1 ]]; then
    for _ in $(seq 40); do
        [[ "$(shell_ask "$p" shell ready)" == yes ]] && break
        sleep 0.5
    done
fi
sleep 1 # (the bar and dock fade in)
if [[ -n "$err" ]]; then
    status error 100 "The update didn't finish" "$err"$'\x1f'"${backup:-}"
elif [[ $up == 0 ]]; then
    status error 100 "Tessera didn't start again" "Press Super+Ctrl+Escape to start it, or run tools/tessera-update.py check in the Tessera folder."$'\x1f'"$backup"
else
    status done 100 "Update installed"
fi
# The screen closes itself (done) or waits for Close (error); then clean up
# (locked: there's no screen; the new shell's Software Update says how it went)
[[ -n $LOCKED ]] && { rm -rf "$T/ui" "$T/choices.json"; echo "finished: ${err:-ok}"; exit 0; }
for _ in $(seq 600); do
    pgrep -f -- "-p $T/ui" >/dev/null || break
    sleep 1
done
rm -rf "$T/ui" "$T/choices.json"
echo "finished: ${err:-ok}"
