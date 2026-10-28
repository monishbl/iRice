#!/usr/bin/env bash
# Installs an iRice update behind a full-screen "Installing update" screen,
# then starts the new shell once (Settings → Software Update → Install update).
#
#   update-run.sh <work dir> <repo> <config dir>
#
# The work dir holds copies of this script and of the update screen (ui/),
# plus choices.json: the update rewrites the repository, so nothing running
# here may be read from it while it does. Started detached by
# services/IriceUpdate.qml; it outlives the shell it replaces.
#
# Why the shell is stopped first: Quickshell reloads whenever its files
# change. Updating under a running shell reloaded it halfway (killing the
# update with it, and running half-new code without its rebuilt parts).
# The shell lock (irice-shell.lock, shared with keep-shell.sh, repair-shell.sh
# and the tty session's watchdog) is held meanwhile, so nothing restarts it early.
# A repo-wide lock (.git/irice-update.lock, also taken by irice-update.py
# apply) makes it one update at a time on a repository, from any session.
set -uo pipefail
T="$1" REPO="$2" CONFIG="$3"
[[ -n "$T" && -d "$T" && -n "$REPO" && -d "$REPO/.git" ]] || { echo "usage: update-run.sh <work dir> <repo> <config dir>" >&2; exit 2; }
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
SIG="${HYPRLAND_INSTANCE_SIGNATURE:-}"
SHELL_DIR="$REPO/shell"
exec >>"$T/log" 2>&1
echo "== $(date '+%F %T') update"

status() { # state percent text [detail]
    printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}" >"$T/status.new" && mv -f "$T/status.new" "$T/status"
}

env_of() { { tr '\0' '\n' <"/proc/$1/environ"; } 2>/dev/null; }
var_of() { env_of "$1" | sed -n "s/^$2=//p" | head -1; }
is_irice() {
    local c
    c="$(tr '\0' ' ' <"/proc/$1/cmdline" 2>/dev/null)"
    # An update screen (this one's or another's) is never the shell
    [[ "$c" == *"/irice-update."* || "$c" == *" -p $T/ui"* ]] && return 1
    [[ "$c" =~ (^|[[:space:]])(-c|--config)[[:space:]]+irice([[:space:]]|$) ]] && return 0
    [[ "$c" == *" -p $SHELL_DIR"* || "$c" == *" --path $SHELL_DIR"* ]] && return 0
    env_of "$1" | grep -qx "IRICE_TTY=1"
}
# This session's iRice shells
shells() {
    local p
    for p in $(pgrep -x qs; pgrep -x quickshell); do
        is_irice "$p" && [[ "$(var_of "$p" HYPRLAND_INSTANCE_SIGNATURE)" == "$SIG" ]] && echo "$p"
    done | sort -n
}

# One update at a time on this repository (another session may be updating
# it): nothing is touched unless this is the only one
exec 8>"$REPO/.git/irice-update.lock"
if ! flock -n 8; then
    status run 0 "Preparing"
    QS0="$(command -v qs || echo qs)"
    env -u IRICE_TTY setsid -f "$QS0" -p "$T/ui" >"$T/ui.log" 2>&1 </dev/null
    sleep 1.2
    status error 100 "Another update is running" "iRice is already being updated (in another session or window). Wait for it to finish, then check again."
    echo "another update holds the repository lock"
    exit 1
fi
export IRICE_UPDATE_HELD=1 # (irice-update.py apply: this process holds the repo lock)

mine=($(shells))
template="${mine[0]:-}"
QS=qs
if [[ -n "$template" ]]; then
    QS="$(readlink -f "/proc/$template/exe")"
    mapfile -d '' tenv <"/proc/$template/environ"
    mapfile -d '' tcmd <"/proc/$template/cmdline"
else
    tenv=()
    tcmd=()
fi
tty_ctl=""
if [[ -n "$template" ]] && env_of "$template" | grep -qx "IRICE_TTY=1"; then
    tty_ctl="$REPO/sandbox/tty-shell.sh"
fi

# The update screen, from its copy here
status run 0 "Preparing"
# (without IRICE_TTY: in the tty session it would pass for the shell)
env -u IRICE_TTY setsid -f "$QS" -p "$T/ui" >"$T/ui.log" 2>&1 </dev/null
sleep 1.2 # (it fades in over the desktop before the shell goes)

# Hold the shell lock, then stop the shell
lockdir="$RUN/hypr/$SIG"
mkdir -p "$lockdir"
exec 9>"$lockdir/irice-shell.lock"
if ! flock -w 15 9; then
    # Something (a shell restarting, a repair) holds it: stopping the shell
    # now could leave two running, or one reloading mid-update. Nothing has
    # been touched yet.
    status error 100 "iRice couldn't be stopped for the update" "Something was restarting the shell. Try again in a moment; nothing was changed."
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
python3 -I "$REPO/tools/irice-update.py" apply --config "$CONFIG" --choices "$T/choices.json" --rebuild "$(cat "$T/rebuild" 2>/dev/null)" \
    >"$T/result.json" 2> >(while IFS= read -r line; do
        echo "$line" >>"$T/log"
        [[ $line =~ ^PROGRESS\ ([0-9]+)\ (.*)$ ]] && status run "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
    done)
code=$?
sleep 0.2 # (the progress reader's last lines)
err="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("error",""))' "$T/result.json" 2>/dev/null)"
backup="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("backup",""))' "$T/result.json" 2>/dev/null)"
[[ $code -ne 0 && -z "$err" ]] && err="The updater stopped unexpectedly (see $T/log)."
cp "$T/result.json" "$RUN/irice-update-result.json" 2>/dev/null # for the new shell's Software Update page

# The new Hyprland config (keybinds, gestures, look), then exactly one shell
[[ -z "$err" ]] && hyprctl reload config-only >/dev/null 2>&1
status restart 92 "Restarting"
hyprctl irice-glass clear >/dev/null 2>&1
if [[ -n "$tty_ctl" ]]; then
    exec 9>&- # the test session's starter takes the lock itself
    env -i "${tenv[@]}" "$tty_ctl" start
elif [[ ${#tcmd[@]} -gt 0 ]]; then
    fresh=()
    for e in "${tenv[@]}"; do [[ $e == QS_ICON_THEME=* ]] || fresh+=("$e"); done
    env -i "${fresh[@]}" setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" "${tcmd[@]:1}" >/dev/null 2>&1 9>&-
    sleep 2
    exec 9>&-
else
    setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -c irice >/dev/null 2>&1 9>&-
    sleep 2
    exec 9>&-
fi

# Done once the new shell answers
up=0
for _ in $(seq 60); do
    p="$(shells | head -1)"
    if [[ -n "$p" ]] && timeout 3 "$QS" ipc --pid "$p" show >/dev/null 2>&1; then
        up=1
        break
    fi
    sleep 0.5
done
# …and has drawn: the wallpaper is up (the bar and dock fade in after it).
# Closing earlier left a black screen until it did.
if [[ $up == 1 ]]; then
    for _ in $(seq 40); do
        [[ "$(timeout 3 "$QS" ipc --pid "$p" call shell ready 2>/dev/null)" == yes ]] && break
        sleep 0.5
    done
fi
sleep 1 # (the bar and dock fade in)
if [[ -n "$err" ]]; then
    status error 100 "The update didn't finish" "$err"$'\x1f'"${backup:-}"
elif [[ $up == 0 ]]; then
    status error 100 "iRice didn't start again" "Press Super+Ctrl+Escape to start it, or run tools/irice-update.py check in the iRice folder."$'\x1f'"$backup"
else
    status done 100 "Update installed"
fi
# The screen closes itself (done) or waits for Close (error); then clean up
for _ in $(seq 600); do
    pgrep -f -- "-p $T/ui" >/dev/null || break
    sleep 1
done
rm -rf "$T/ui" "$T/choices.json"
echo "finished: ${err:-ok}"
