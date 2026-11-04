#!/usr/bin/env bash
# Runs INSIDE the `irice` distrobox. Starts a nested Hyprland, fully isolated
# from the host session:
#   - own display          -> it draws into a private, virtual KWin, so no
#                             window shows on your desktop and your keyboard and
#                             pointer never reach it (IRICE_VISIBLE=1: a window
#                             on the host compositor instead, as it used to be)
#   - own sound            -> a private PipeWire whose only output is a silent
#                             "Speakers" (IRICE_HOST_AUDIO=1: your real audio)
#   - own XDG_RUNTIME_DIR  -> own Hyprland IPC sockets, own Quickshell IPC
#   - own D-Bus session    -> our notification server / tray never touch the host's
#   - own HOME             -> set by distrobox (.sandbox/home), host ~/.config untouched
# Never use pkill/killall here: distrobox shares the host PID namespace.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/sandbox/lib.sh"
RT="$IRICE_RT"
HOST_RT=/run/user/$(id -u)
HOST_WL="${IRICE_HOST_WAYLAND:-${WAYLAND_DISPLAY:-wayland-1}}"

if [[ "$HOME" != "$IRICE_HOME" ]]; then
    echo "inner.sh: refusing to run, HOME is $HOME (expected the sandbox home)" >&2
    exit 1
fi
if [[ -f "$RT/hyprland.pid" ]] && kill -0 "$(cat "$RT/hyprland.pid")" 2>/dev/null; then
    echo "inner.sh: sandbox already running (pid $(cat "$RT/hyprland.pid"))" >&2
    exit 1
fi

rm -rf "$RT"
mkdir -p -m 700 "$RT"
helpers=()
trap '[[ ${#helpers[@]} -gt 0 ]] && kill "${helpers[@]}" 2>/dev/null; true' EXIT
if [[ "${IRICE_VISIBLE:-0}" == 1 ]]; then
    ln -s "$HOST_RT/$HOST_WL" "$RT/wayland-host"
else
    # KWin's virtual backend: no screen and no input devices, one 1920x1080
    # output (Hyprland needs Wayland interfaces as new as KWin's; Weston,
    # cage and sway are too old); its socket in a folder of its own, so it's
    # never taken for the nested Hyprland's wayland-N (dev.sh's nested). The
    # nested Hyprland is its only window.
    mkdir -m 700 "$RT/display"
    sock=irice-display
    env -u WAYLAND_DISPLAY -u DISPLAY XDG_RUNTIME_DIR="$RT/display" KWIN_COMPOSE=O2ES \
        kwin_wayland --virtual --width 1920 --height 1080 --socket "$sock" >"$RT/display.log" 2>&1 &
    helpers+=($!)
    for _ in $(seq 80); do [[ -S "$RT/display/$sock" ]] && break; sleep 0.1; done
    [[ -S "$RT/display/$sock" ]] || { echo "inner.sh: the hidden display didn't start, see $RT/display.log" >&2; exit 1; }
    ln -s "$RT/display/$sock" "$RT/wayland-host"
fi
# Host audio (IRICE_HOST_AUDIO=1), so volume/media widgets play for real
if [[ "${IRICE_HOST_AUDIO:-0}" == 1 ]]; then
    [[ -S "$HOST_RT/pipewire-0" ]] && ln -s "$HOST_RT/pipewire-0" "$RT/pipewire-0"
    # A real directory with the socket linked in: PulseAudio clients refuse a
    # symlinked $XDG_RUNTIME_DIR/pulse ("Too many levels of symbolic links")
    [[ -S "$HOST_RT/pulse/native" ]] && mkdir -m 700 "$RT/pulse" && ln -s "$HOST_RT/pulse/native" "$RT/pulse/native"
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
# Silent sound (unless IRICE_HOST_AUDIO=1): PipeWire, WirePlumber with every
# device monitor off (no sound card, Bluetooth or camera is ever touched) and
# the Pulse server, on this sandbox's own bus and runtime folder, with one
# null sink named Speakers. Started inside the session bus, before Hyprland.
AUDIO=""
if [[ "${IRICE_HOST_AUDIO:-0}" != 1 ]] && command -v pipewire >/dev/null && command -v wireplumber >/dev/null; then
    cfg="$RT/audio-config"
    mkdir -p "$cfg/pipewire/pipewire.conf.d" "$cfg/wireplumber/wireplumber.conf.d"
    cat >"$cfg/pipewire/pipewire.conf.d/irice-speakers.conf" <<'CONF'
context.objects = [
    { factory = adapter
      args = { factory.name = support.null-audio-sink node.name = "irice-speakers" node.description = "Speakers"
               media.class = Audio/Sink audio.position = [ FL FR ] monitor.channel-volumes = true object.linger = true } }
]
CONF
    cat >"$cfg/wireplumber/wireplumber.conf.d/irice-silent.conf" <<'CONF'
wireplumber.profiles = {
    main = {
        monitor.alsa = disabled
        monitor.alsa-midi = disabled
        monitor.bluez = disabled
        monitor.bluez-midi = disabled
        monitor.libcamera = disabled
        monitor.v4l2 = disabled
    }
}
CONF
    AUDIO="XDG_CONFIG_HOME=$cfg pipewire & XDG_CONFIG_HOME=$cfg wireplumber & XDG_CONFIG_HOME=$cfg pipewire-pulse & sleep 0.5;"
fi
dbus-run-session -- bash -c "$AUDIO"' echo $$ > "$0/hyprland.pid"; echo "$DBUS_SESSION_BUS_ADDRESS" > "$0/session-bus"; exec start-hyprland -- --config "$1"' \
    "$RT" "$ROOT/sandbox/hypr/hyprland.lua" >"$LOG" 2>&1
