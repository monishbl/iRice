#!/usr/bin/env bash
# Quit an app for real (the dock's and the menu bar's Quit, like macOS),
# after its windows were asked to close (services/Apps.qml quit()).
#   quit-app.sh <pid> [pid…]    the processes behind the app's windows
# Apps that hide to the tray on close (Electron: Claude, Webex, Discord;
# Ente Auth) keep running after that. So: wait up to 3 s for them to end;
# if they're still running and none of their windows is still showing (one
# still showing means the app is asking something, "Save changes?", so
# it's left to you), SIGTERM the app's process tree, then SIGKILL whatever
# is left 5 s later. Never the compositor, the shell or the session.
set -u

# The app's processes: the window's process and everything under it, taken
# now (children are re-parented once the parent ends)
tree() {
    local p
    for p in "$@"; do
        echo "$p"
        tree $(pgrep -P "$p" 2>/dev/null)
    done
}
mapfile -t all < <(tree "$@" | sort -un)

# Leave alone anything the session can't lose
safe=()
for p in "${all[@]}"; do
    [[ "$p" =~ ^[0-9]+$ ]] && (( p > 1 )) || continue
    [[ "$p" == "$$" || "$p" == "$PPID" ]] && continue
    comm=$(cat "/proc/$p/comm" 2>/dev/null) || continue
    case "$comm" in
        Hyprland|hyprland|start-hyprland|quickshell|qs|systemd|dbus-daemon|dbus-broker|Xwayland|pipewire*|wireplumber|login|sddm*|ly*|gdm*|greetd) continue ;;
    esac
    safe+=("$p")
done
(( ${#safe[@]} > 0 )) || exit 0

alive() {
    local p
    for p in "${safe[@]}"; do kill -0 "$p" 2>/dev/null && echo "$p"; done
}
shown() { # windows that still belong to these processes
    local pids
    pids=$(IFS=,; echo "${safe[*]}")
    hyprctl -j clients 2>/dev/null | jq -e --arg p "$pids" '($p | split(",") | map(tonumber)) as $ps | any(.[]; .pid as $x | $ps | index($x))' >/dev/null 2>&1
}

# 1. Closing the windows may be enough
for _ in $(seq 12); do
    [[ -z "$(alive)" ]] && exit 0
    sleep 0.25
done
# A window is still up: a question for you, or a window that won't close
shown && exit 0

# 2. Ask the processes to end
mapfile -t left < <(alive)
(( ${#left[@]} > 0 )) && kill -TERM "${left[@]}" 2>/dev/null
for _ in $(seq 20); do
    [[ -z "$(alive)" ]] && exit 0
    sleep 0.25
done

# 3. Whatever didn't
mapfile -t left < <(alive)
(( ${#left[@]} > 0 )) && kill -KILL "${left[@]}" 2>/dev/null
exit 0
