#!/usr/bin/env bash
# iRice dev entry point (run on the host). Everything runs inside the `irice`
# distrobox against a nested Hyprland; the host session is never touched.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# This folder, always: inside an iRice session the environment says where
# *that* iRice is (IRICE_ROOT), and the sandbox would run that copy instead
export IRICE_ROOT="$ROOT"
BOX=irice
RT=/tmp/irice-rt
source "$ROOT/sandbox/lib.sh"
PKGS=(hyprland hypridle hyprpicker hyprsunset xdg-desktop-portal-hyprland
      quickshell qt6-declarative qt6-svg qt6-imageformats qt6-multimedia qt6-5compat qt6-wayland qt6-shadertools
      pipewire wireplumber pipewire-pulse networkmanager bluez-utils upower libsecret brightnessctl playerctl
      libnotify mesa-utils mesa vulkan-intel intel-media-driver dbus
      grim slurp wl-clipboard cliphist wf-recorder tesseract tesseract-data-eng wtype translate-shell curl jq
      inter-font ttf-jetbrains-mono noto-fonts noto-fonts-emoji papirus-icon-theme
      foot thunar mousepad gnome-calculator imagemagick ffmpeg
      libqalculate ripgrep plocate swappy psmisc poppler ffmpegthumbnailer python-gobject matugen)

box() { distrobox enter "$BOX" -- "$@"; }
# Runs a command inside the box, pointed at the nested compositor.
nested() {
    local wl; wl="$(cd "$RT" && ls -d wayland-[0-9] 2>/dev/null | head -1)"
    box env -u QT_QPA_PLATFORMTHEME -u ANTHROPIC_API_KEY -u ANTHROPIC_AUTH_TOKEN -u GEMINI_API_KEY -u OPENAI_API_KEY -u OPENROUTER_API_KEY \
        XDG_RUNTIME_DIR="$RT" WAYLAND_DISPLAY="$wl" SHELL=/bin/bash LANG=C.UTF-8 LC_ALL=C.UTF-8 \
        DBUS_SYSTEM_BUS_ADDRESS=unix:path=/run/host/run/dbus/system_bus_socket \
        DBUS_SESSION_BUS_ADDRESS="$(cat "$RT/session-bus" 2>/dev/null)" \
        HYPRLAND_INSTANCE_SIGNATURE="$(ls "$RT/hypr" 2>/dev/null | head -1)" "$@"
}
# The sandbox compositor's pid, only if the pid file (in a shared /tmp) really
# names our own nested Hyprland: a real directory we own, a number, our user,
# and a start-hyprland running one of this repo's sandbox configs.
sandbox_pid() {
    local p
    [[ -d "$RT" && ! -L "$RT" && -O "$RT" && -f "$RT/hyprland.pid" ]] || return 1
    p="$(head -c 16 "$RT/hyprland.pid")"
    [[ "$p" =~ ^[0-9]+$ && -O "/proc/$p" ]] || return 1
    [[ "$(cat "/proc/$p/comm" 2>/dev/null)" == start-hyprland ]] || return 1
    [[ "$(tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null)" == *"--config $ROOT/sandbox/hypr/"* ]] || return 1
    echo "$p"
}
running() { sandbox_pid >/dev/null; }

case "${1:-help}" in
    create)
        mkdir -p "$ROOT/.sandbox/home"
        distrobox create --yes --name "$BOX" --image docker.io/library/archlinux:latest \
            --home "$ROOT/.sandbox/home" --additional-flags "--device /dev/dri"
        box sudo pacman -Syu --noconfirm --needed "${PKGS[@]}"
        ;;
    up)
        running && { echo "Sandbox already running."; exit 0; }
        stop_sandbox_apps # left behind by a sandbox that crashed
        : "${WAYLAND_DISPLAY:?host WAYLAND_DISPLAY not set}"
        setsid -f distrobox enter "$BOX" -- env IRICE_HOST_WAYLAND="$WAYLAND_DISPLAY" \
            "$ROOT/sandbox/inner.sh" >/dev/null 2>&1
        for _ in $(seq 50); do running && [[ -d "$RT/hypr" ]] && break; sleep 0.2; done
        running || { echo "Sandbox failed to start, see $RT/hyprland.log"; exit 1; }
        # Park the sandbox window alone on host workspace 1 (only touches that window).
        hpid="$(pgrep -P "$(sandbox_pid)" -x Hyprland | head -1 || true)"
        found=0
        if [[ $hpid =~ ^[0-9]+$ ]]; then
            for _ in $(seq 25); do
                hyprctl -j clients | jq -e ".[] | select(.pid == $hpid)" >/dev/null && { found=1; break; }
                sleep 0.2
            done
        fi
        # Only when that exact window exists; never a bare "pid:" to the real compositor
        [[ $found == 1 ]] && { hyprctl dispatch "hl.dsp.window.move({ workspace = ${IRICE_HOST_WORKSPACE:-1}, follow = false, window = \"pid:$hpid\" })" >/dev/null || true; }
        nested hyprctl output create headless IRICE-TEST >/dev/null
        # Layer surfaces in the nested output keep their first size after the host
        # resizes the window, so restart the shell once it has settled.
        sleep 1.5 && nested "$ROOT/sandbox/reload-shell.sh" >/dev/null
        echo "Sandbox up (pid $(sandbox_pid)). Mod key is ALT."
        ;;
    down)
        stop_sandbox_apps
        stop_sandbox_shells
        running && kill "$(sandbox_pid)" && echo "Sandbox stopped." \
            || echo "Sandbox not running."
        ;;
    status)  running && echo "running (pid $(sandbox_pid))" || echo "stopped" ;;
    reload)  "$ROOT/tools/build-shaders.sh" && nested env IRICE_LAB="${IRICE_LAB:-0}" "$ROOT/sandbox/reload-shell.sh" ;;
    # Screenshots come from the headless IRICE-TEST output, which renders even
    # while the sandbox window is on a hidden host workspace.
    shot)    nested timeout 15 grim -o IRICE-TEST "${2:-/tmp/irice-rt/shot.png}" && echo "${2:-/tmp/irice-rt/shot.png}" ;;
    hyprctl) shift; nested hyprctl "$@" ;;
    run)     shift; nested "$@" ;;
    ipc)     shift; nested qs -p "$ROOT/shell" ipc call "$@" ;;
    pointer) shift; nested "$ROOT/.sandbox/bin/vptr" "$@" ;;
    enter)   box ;;
    # CPU / memory / frames per scripted phase: ./dev.sh perf [secs] [phase...]
    perf)    shift; "$ROOT/sandbox/perf.sh" "$@" ;;
    logs)    tail -n "${2:-60}" "$RT/hyprland.log" "$RT/quickshell.log" 2>/dev/null ;;
    tty-stop)
        # Ends a ./dev.sh tty session from anywhere (another console, or your
        # normal desktop) when it's frozen. Matches only processes whose
        # executable is Hyprland/start-hyprland/dbus-run-session started with
        # sandbox/hypr/tty.lua, or that carry the session's IRICE_TTY=1 marker.
        #   ./dev.sh tty-stop [--force] [--dry-run]
        sig=TERM dry=0
        for a in "${@:2}"; do
            case "$a" in --force) sig=KILL ;; --dry-run) dry=1 ;; esac
        done
        self_tree=" $$ $PPID "
        pids=""
        for d in /proc/[0-9]*; do
            p=${d#/proc/}
            [[ "$self_tree" == *" $p "* ]] && continue
            comm=$(cat "$d/comm" 2>/dev/null) || continue
            case "$comm" in
                Hyprland|start-hyprland|dbus-run-sessio*)
                    tr '\0' ' ' <"$d/cmdline" 2>/dev/null | grep -q "sandbox/hypr/tty.lua" && pids="$pids $p" ;;
                qs|quickshell|bash|sh|wl-paste)
                    # The marker, or the session's own tty-shell.sh (a script's
                    # exports don't show in its own /proc environ)
                    { { tr '\0' '\n' <"$d/environ"; } 2>/dev/null | grep -qx "IRICE_TTY=1" ||
                        tr '\0' ' ' <"$d/cmdline" 2>/dev/null | grep -q "sandbox/tty-shell.sh"; } && pids="$pids $p" ;;
            esac
        done
        if [[ -z "${pids// /}" ]]; then echo "No tty session is running."; exit 0; fi
        for p in $pids; do printf '  %s %s\n' "$p" "$(tr '\0' ' ' </proc/$p/cmdline 2>/dev/null | cut -c1-100)"; done
        [[ $dry == 1 ]] && exit 0
        echo "Stopping the tty session (SIG$sig)"
        kill -$sig $pids 2>/dev/null || true
        if [[ $sig == TERM ]]; then
            alive=""
            for _ in $(seq 50); do
                sleep 0.1
                alive="$(for p in $pids; do kill -0 "$p" 2>/dev/null && echo "$p"; done)"
                [[ -z "$alive" ]] && break
            done
            [[ -n "$alive" ]] && { echo "Still running, forcing:" $alive; kill -9 $alive 2>/dev/null || true; }
        fi
        echo "Done. Your normal desktop wasn't touched."
        ;;
    tty)
        # Real (non-nested) test session. Run from a text console (Ctrl+Alt+F3,
        # log in, cd here, ./dev.sh tty). Your graphical session keeps running
        # on its own VT; switch back with Ctrl+Alt+F<n>.
        if [[ -n "${WAYLAND_DISPLAY:-}" || -n "${DISPLAY:-}" ]]; then
            echo "Run this from a text console, not inside a graphical session:"
            echo "  Ctrl+Alt+F3, log in, then: cd $ROOT && ./dev.sh tty"
            exit 1
        fi
        running && kill "$(sandbox_pid)" 2>/dev/null || true
        export IRICE_ROOT="$ROOT"
        # Don't redirect XDG dirs here: podman keeps its containers under
        # XDG_DATA_HOME. The shell runs in the container, whose HOME is
        # .sandbox/home, so its config/state/cache stay isolated anyway.
        unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN GEMINI_API_KEY OPENAI_API_KEY OPENROUTER_API_KEY
        # Hyprland would put its display into the user systemd, which your
        # usual desktop shares (and clear it on exit): keep it out of there
        export HYPRLAND_NO_SD_VARS=1
        exec dbus-run-session -- start-hyprland -- --config "$ROOT/sandbox/hypr/tty.lua"
        ;;
    reset-home)
        running && { echo "Stop the sandbox first."; exit 1; }
        rm -rf "$ROOT/.sandbox/home" && mkdir -p "$ROOT/.sandbox/home" && echo "Sandbox home wiped."
        ;;
    *)
        cat <<EOF
Usage: ./dev.sh <command>
  create       Create the irice distrobox and install the toolchain
  up | down    Start / stop the nested Hyprland sandbox
  tty          Start a real (non-nested) iRice session; run from a text console
  tty-stop     End a (frozen) tty session from anywhere [--force] [--dry-run]
  status       Show whether the sandbox is running
  reload       Restart the sandbox Quickshell
  hyprctl ...  Run hyprctl against the sandbox compositor
  run ...      Run any command inside the sandbox session
  ipc ...      Call the shell's IPC, e.g. ./dev.sh ipc panels open controlCenter IRICE-TEST
  shot [file]  Screenshot the headless IRICE-TEST output
  pointer ...  Move/click/scroll a virtual pointer on IRICE-TEST, e.g. pointer move 960 15 click
               (build it once with: distrobox enter irice -- sandbox/tools/build.sh)
  enter        Open a shell in the container
  logs [n]     Tail the sandbox Hyprland and Quickshell logs
  reset-home   Wipe the sandbox home (.sandbox/home)
EOF
        ;;
esac
