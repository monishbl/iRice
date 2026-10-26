#!/usr/bin/env bash
# Runs INSIDE the `irice` distrobox. Starts a nested Hyprland in a window on the
# host compositor, fully isolated from the host session:
#   - own XDG_RUNTIME_DIR  -> own Hyprland IPC sockets, own Quickshell IPC
#   - own D-Bus session    -> our notification server / tray never touch the host's
#   - own HOME             -> set by distrobox (.sandbox/home), host ~/.config untouched
# Never use pkill/killall here: distrobox shares the host PID namespace.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RT=/tmp/irice-rt
HOST_RT=/run/user/$(id -u)
HOST_WL="${IRICE_HOST_WAYLAND:-${WAYLAND_DISPLAY:-wayland-1}}"

if [[ "$HOME" != "$ROOT/.sandbox/home" ]]; then
    echo "inner.sh: refusing to run, HOME is $HOME (expected the sandbox home)" >&2
    exit 1
fi
if [[ -f "$RT/hyprland.pid" ]] && kill -0 "$(cat "$RT/hyprland.pid")" 2>/dev/null; then
    echo "inner.sh: sandbox already running (pid $(cat "$RT/hyprland.pid"))" >&2
    exit 1
fi

rm -rf "$RT"
mkdir -p -m 700 "$RT"
ln -s "$HOST_RT/$HOST_WL" "$RT/wayland-host"
# Host audio, so volume/media widgets have something real to show
if [[ "${IRICE_HOST_AUDIO:-1}" == 1 ]]; then
    [[ -S "$HOST_RT/pipewire-0" ]] && ln -s "$HOST_RT/pipewire-0" "$RT/pipewire-0"
    [[ -d "$HOST_RT/pulse" ]] && ln -s "$HOST_RT/pulse" "$RT/pulse"
fi

# Never let the sandbox spend real money with host API keys
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN GEMINI_API_KEY OPENAI_API_KEY OPENROUTER_API_KEY
unset HYPRLAND_INSTANCE_SIGNATURE HYPRLAND_CMD DBUS_SESSION_BUS_ADDRESS \
      QT_QPA_PLATFORMTHEME XDG_MENU_PREFIX DISPLAY
export XDG_RUNTIME_DIR="$RT"
export WAYLAND_DISPLAY=wayland-host
export XDG_CONFIG_HOME="$HOME/.config" XDG_CACHE_HOME="$HOME/.cache" \
       XDG_DATA_HOME="$HOME/.local/share" XDG_STATE_HOME="$HOME/.local/state"
export IRICE_ROOT="$ROOT" SHELL=/bin/bash IRICE_SANDBOX=1 LANG=C.UTF-8 LC_ALL=C.UTF-8
# Real system bus (NetworkManager, UPower, BlueZ), so status items show real data
export DBUS_SYSTEM_BUS_ADDRESS=unix:path=/run/host/run/dbus/system_bus_socket

LOG="$RT/hyprland.log"
echo "iRice sandbox: runtime $RT, log $LOG"
dbus-run-session -- bash -c 'echo $$ > "$0/hyprland.pid"; echo "$DBUS_SESSION_BUS_ADDRESS" > "$0/session-bus"; exec start-hyprland -- --config "$1"' \
    "$RT" "$ROOT/sandbox/hypr/hyprland.lua" >"$LOG" 2>&1
