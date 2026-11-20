#!/usr/bin/env bash
# Repair the Tessera shell without touching your apps or windows.
#
# Fixes duplicated bars and docks, a frozen or half-drawn shell, and leftovers
# from an earlier session that ended badly:
#   1. stops Tessera shells (and tty test-session watchdogs) whose compositor
#      is gone: they latch onto the next session's display and draw twice
#   2. replaces this session's shell(s) with exactly one fresh shell, started
#      with the same command and environment as before
#   3. clears glass shapes left in the compositor plugin (the new shell
#      sends its own)
# Settings, layout, clipboard and the rest live in files, so nothing is lost.
# Run from the system menu (Repair Shell), Super+Ctrl+Escape, or a terminal.
# Only Tessera is touched: other Quickshell configs (like end4's) are left alone.
set -uo pipefail

RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
SIG="${HYPRLAND_INSTANCE_SIGNATURE:-}"
SHELL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG="$RUN/tessera-repair.log"
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

# Is this Quickshell process a Tessera shell?
is_tessera() {
    local c
    c="$(cmd_of "$1")"
    [[ "$c" =~ (^|[[:space:]])(-c|--config)[[:space:]]+tessera([[:space:]]|$) ]] && return 0
    [[ "$c" == *" -p $SHELL_DIR "* || "$c" == *" --path $SHELL_DIR "* ]] && return 0
    [[ "$c" =~ -p[[:space:]]+[^[:space:]]*/([Tt]essera|[Ii][Rr]ice)/shell([[:space:]]|$) ]] && return 0
    # (started by launch-shell.sh: the Material look runs from its own root)
    env_of "$1" | grep -qx "TESSERA_SHELL=1" && return 0
    env_of "$1" | grep -qx "TESSERA_TTY=1"
}

# Standalone (rust.standalone, the default): tessera-ui is the shell. Only one
# started with --standalone: the one the QML shell starts as its helper ends with it
standalone_pids() {
    local p
    for p in $(pgrep -x tessera-ui); do
        [[ "$(cmd_of "$p")" == *" --standalone"* ]] && echo "$p"
    done
    return 0
}
is_standalone() { [[ "$(cat "/proc/$1/comm" 2>/dev/null)" == tessera-ui && "$(cmd_of "$1")" == *" --standalone"* ]]; }

shells() {
    local p
    for p in $(pgrep -x tessera-shell; pgrep -x qs; pgrep -x quickshell; standalone_pids); do
        is_tessera "$p" && echo "$p"
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
    # (tessera-ui's own command isn't the launcher's: started the usual way,
    # the launcher picks standalone again from the settings)
    if is_standalone "$template"; then
        tcmd=()
    fi
    # (never the old shell's TESSERA_STANDALONE either: the QML shell that
    # tessera-ui became when EGL didn't start has =0, and kept, every Try
    # again started the QML shell again; the launcher decides afresh, 20 Nov)
    fresh=()
    for e in "${tenv[@]}"; do [[ $e == TESSERA_STANDALONE=* ]] || fresh+=("$e"); done
    tenv=("${fresh[@]}")
else
    tenv=()
    tcmd=()
fi
# The login session's own variables, from the compositor (not from the old
# shell: one restarted over ssh carried that ssh login's XDG_SESSION_ID and
# SSH_* into every later shell, so its polkit agent served the ssh session
# and the desktop's own asks found no agent; 20 Nov)
# (hyprctl names its pid: Hyprland's own /proc environ predates its signature)
compositor_pid() {
    [[ -n "$SIG" ]] || return 0
    hyprctl instances -j 2>/dev/null | tr -d ' \n\t' | grep -o "\"instance\":\"$SIG\",[^}]*" | grep -o '"pid":[0-9]*' | head -1 | cut -d: -f2
}
cpid="$(compositor_pid)"
# Run from another login (ssh, a tty): hand the repair to the compositor, so
# the new shell starts inside the desktop's login with the compositor's own
# environment (what its config set: ELECTRON_OZONE_PLATFORM_HINT,
# QT_QPA_PLATFORM, cursor…; /proc keeps only what it started with). A shell
# started from ssh had none of it: the dock's Electron apps looked for X11
# and quit, and polkit asked for a password for a Wi-Fi scan (20 Nov).
if [[ -n "$cpid" && -z "${TESSERA_REPAIR_HANDOFF:-}" && "$(cat /proc/self/sessionid 2>/dev/null)" != "$(cat "/proc/$cpid/sessionid" 2>/dev/null)" ]]; then
    me="$(readlink -f "${BASH_SOURCE[0]}")"
    if hyprctl dispatch "hl.dsp.exec_cmd(\"env TESSERA_REPAIR_HANDOFF=1 TESSERA_REPAIR_QUIET=${TESSERA_REPAIR_QUIET:-0} '$me'\")" 2>/dev/null | grep -q '^ok'; then
        echo "handed to the compositor's session"
        exit 0
    fi
fi
if [[ ${#tenv[@]} -gt 0 ]]; then
    fresh=()
    for e in "${tenv[@]}"; do
        case "$e" in
            SSH_CLIENT=* | SSH_CONNECTION=* | SSH_TTY=*) ;;
            XDG_SESSION_ID=* | XDG_SESSION_TYPE=* | XDG_SEAT=* | XDG_VTNR=*) [[ -z "$cpid" ]] && fresh+=("$e") ;;
            *) fresh+=("$e") ;;
        esac
    done
    if [[ -n "$cpid" ]]; then
        while IFS= read -r e; do fresh+=("$e"); done < <(env_of "$cpid" | grep -E '^XDG_(SESSION_ID|SESSION_TYPE|SEAT|VTNR)=')
    fi
    tenv=("${fresh[@]}")
fi
# (and the usual way below starts with this script's own environment)
unset SSH_CLIENT SSH_CONNECTION SSH_TTY
if [[ -n "$cpid" ]]; then
    while IFS= read -r e; do export "${e?}"; done < <(env_of "$cpid" | grep -E '^XDG_(SESSION_ID|SESSION_TYPE|SEAT|VTNR)=')
fi
# Run from another login (ssh), the shell would live in that login's
# session, which isn't the active one: polkit then asked for a password
# for what the desktop may just do (a Wi-Fi scan). Started in its own
# scope of the user's manager it counts as the desktop's, as apps do.
in_session=()
if [[ -n "$cpid" && "$(cat /proc/self/sessionid 2>/dev/null)" != "$(cat "/proc/$cpid/sessionid" 2>/dev/null)" ]] && command -v systemd-run >/dev/null; then
    in_session=(systemd-run --user --scope --quiet --collect --)
fi
tty_ctl=""
if [[ -n "$template" ]] && env_of "$template" | grep -qx "TESSERA_TTY=1"; then
    troot="$(var_of "$template" TESSERA_ROOT)"
    tty_ctl="$troot/sandbox/tty-shell.sh"
    [[ -x "$tty_ctl" ]] || tty_ctl=""
fi

# Hold the shell lock while swapping (keep-shell.sh mustn't start one in the
# gap); the test session's own starter takes it itself
if [[ -z "$tty_ctl" && -d "$RUN/hypr/$SIG" ]]; then
    exec 7>"$RUN/hypr/$SIG/tessera-shell.lock"
    flock -w 10 7
fi

# (a repair or an update's restart isn't a failed start: the standalone
# count goes, and the shell gets 6 s for its orderly exit, 18 Nov)
rm -f "${XDG_STATE_HOME:-$HOME/.local/state}/tessera/standalone-starts"
if [[ ${#mine[@]} -gt 0 ]]; then
    kill "${mine[@]}" 2>/dev/null
    for _ in $(seq 60); do
        sleep 0.1
        left=()
        for p in "${mine[@]}"; do kill -0 "$p" 2>/dev/null && left+=("$p"); done
        [[ ${#left[@]} -eq 0 ]] && break
    done
    [[ ${#left[@]} -gt 0 ]] && kill -9 "${left[@]}" 2>/dev/null # frozen
fi

# 3. Glass shapes from the old shells (the new one sends its own)
alive "$SIG" && hyprctl tessera-glass clear >/dev/null 2>&1

if [[ -n "$tty_ctl" ]]; then
    env -i "${tenv[@]}" "$tty_ctl" start # the test session's own starter (locked, no duplicates)
elif [[ ${#tcmd[@]} -gt 0 ]]; then
    # Same arguments through the launcher, which picks the icon theme afresh
    # (so a theme changed in Settings applies)
    fresh=()
    for e in "${tenv[@]}"; do [[ $e == QS_ICON_THEME=* ]] || fresh+=("$e"); done
    "${in_session[@]}" env -i "${fresh[@]}" setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" "${tcmd[@]:1}" >/dev/null 2>&1 7>&- 6>&-
elif [[ -n "${TESSERA_ROOT:-}" && -f "$TESSERA_ROOT/shell/shell.qml" ]]; then
    "${in_session[@]}" setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -p "$TESSERA_ROOT/shell" >/dev/null 2>&1 7>&- 6>&-
else
    "${in_session[@]}" setsid -f "$SHELL_DIR/assets/scripts/launch-shell.sh" -c tessera >/dev/null 2>&1 7>&- 6>&-
fi
sleep 2 # let it register, then let keep-shell.sh look again
exec 7>&- 6>&-
echo "started a fresh shell; removed $removed stale and $extra duplicate"

# Tell the user once the new shell can show it (not when switching looks:
# TESSERA_REPAIR_QUIET=1, switch-look.sh)
[[ ${TESSERA_REPAIR_QUIET:-0} == 1 ]] && exit 0
sleep 4
msg="Your windows and apps weren't touched."
(( removed + extra > 0 )) && msg="Removed $((removed + extra)) extra $( (( removed + extra == 1 )) && echo shell || echo shells). $msg"
notify-send -a Tessera -i view-refresh "Shell repaired" "$msg" 2>/dev/null || true
