#!/usr/bin/env bash
# Quit an app for real (the dock's and the menu bar's Quit, like macOS),
# after its windows were asked to close (services/Apps.qml quit()).
#   quit-app.sh <pid> [pid…]    the processes behind the app's windows
# Apps that hide to the tray on close (Electron: Claude, Webex, Discord;
# Ente Auth) keep running after that. So: wait up to 3 s for them to end;
# if they're still running and none of their windows is still showing (one
# still showing means the app is asking something, "Save changes?", so
# it's left to you), SIGTERM the app's process tree, then SIGKILL whatever
# is left 5 s later. Never the compositor, the shell or the session, nor
# anything running under them (a nested test session in a terminal, say).
# If it can't tell whether a window is showing, it ends nothing.
set -u

# Processes the session can't lose: neither they nor anything under them
protected() { # pid
    local comm cmd
    comm=$(cat "/proc/$1/comm" 2>/dev/null) || return 0
    case "$comm" in
        Hyprland|hyprland|start-hyprland|quickshell|qs|systemd|dbus-daemon|dbus-broker|Xwayland|pipewire*|wireplumber|login|sddm*|ly*|gdm*|greetd|distrobox*|podman|conmon) return 0 ;;
    esac
    cmd=$(tr '\0' ' ' <"/proc/$1/cmdline" 2>/dev/null)
    [[ "$cmd" == *Hyprland* || "$cmd" == *start-hyprland* || "$cmd" == *"qs -p"* || "$cmd" == *watch.sh* ||
        "$cmd" == *tty-shell.sh* || "$cmd" == *dev.sh* || "$cmd" == *distrobox* || "$cmd" == *launch-shell.sh* ]]
}
# When a process started (clock ticks since boot): a pid seen again later is
# the same process only if this matches (pids get reused)
started() { awk '{ sub(/^.*\) /, ""); print $20 }' "/proc/$1/stat" 2>/dev/null; }

# The app's processes: the window's process and everything under it, taken
# now (children are re-parented once the parent ends), stopping at anything
# protected
tree() {
    local p
    for p in "$@"; do
        [[ "$p" =~ ^[0-9]+$ ]] && (( p > 1 )) || continue
        [[ "$p" == "$$" || "$p" == "$PPID" ]] && continue
        protected "$p" && continue
        echo "$p"
        tree $(pgrep -P "$p" 2>/dev/null)
    done
}
declare -A born=()
safe=()
for p in $(tree "$@" | sort -un); do
    s=$(started "$p") || continue
    [[ -n "$s" ]] || continue
    born[$p]=$s
    safe+=("$p")
done
(( ${#safe[@]} > 0 )) || exit 0

# Still running, and still the same process
alive() {
    local p
    for p in "${safe[@]}"; do [[ "$(started "$p")" == "${born[$p]}" ]] && echo "$p"; done
}
# A window that still belongs to these processes? When that can't be told
# (hyprctl or jq missing or failing), say yes: ending an app that's asking
# "Save changes?" loses work
shown() {
    local pids out rc
    command -v jq >/dev/null 2>&1 || return 0
    out=$(timeout 3 hyprctl -j clients 2>/dev/null) || return 0
    [[ -n "$out" ]] || return 0
    pids=$(IFS=,; echo "${safe[*]}")
    jq -e --arg p "$pids" '($p | split(",") | map(tonumber)) as $ps | any(.[]; .pid as $x | $ps | index($x))' >/dev/null 2>&1 <<<"$out"
    rc=$?
    (( rc == 1 )) && return 1 # (jq -e: 1 is a clear "no")
    return 0
}
signal() { # SIG — only the ones that are still the same processes
    local left
    mapfile -t left < <(alive)
    (( ${#left[@]} > 0 )) && kill "-$1" "${left[@]}" 2>/dev/null
}

# 1. Closing the windows may be enough
for _ in $(seq 12); do
    [[ -z "$(alive)" ]] && exit 0
    sleep 0.25
done
# A window is still up: a question for you, or a window that won't close
shown && exit 0

# 2. Ask the processes to end
signal TERM
for _ in $(seq 20); do
    [[ -z "$(alive)" ]] && exit 0
    sleep 0.25
done

# 3. Whatever didn't (checked again just before)
shown && exit 0
signal KILL
exit 0
