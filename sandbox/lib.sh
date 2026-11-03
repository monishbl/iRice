# Sourced by dev.sh, inner.sh and reload-shell.sh (with ROOT set).
# distrobox shares the host PID namespace, so sandbox processes are identified by
# their environment: only they run with XDG_RUNTIME_DIR=/tmp/irice-rt. Host
# processes (e.g. the real end4 `qs -c ii`) can never match.
# IRICE_ARENA=2, 3, … is another identical sandbox beside the first, with its
# own runtime folder (/tmp/irice-rt-N) and home (.sandbox/arena-N/home), so
# checks can run side by side. 1 (the default) is the usual sandbox.

IRICE_ARENA="${IRICE_ARENA:-1}"
[[ "$IRICE_ARENA" =~ ^[1-9]$ ]] || { echo "IRICE_ARENA must be 1-9" >&2; exit 2; }
if [[ "$IRICE_ARENA" == 1 ]]; then
    IRICE_RT=/tmp/irice-rt
    IRICE_HOME="$ROOT/.sandbox/home"
else
    IRICE_RT=/tmp/irice-rt-$IRICE_ARENA
    IRICE_HOME="$ROOT/.sandbox/arena-$IRICE_ARENA/home"
fi

# PIDs of sandbox processes with the given executable name.
sandbox_pids() {
    local name="$1" pid
    for pid in $(pgrep -x "$name"); do
        # Unreadable environ (host processes, from inside the container) = not ours.
        if { tr '\0' '\n' <"/proc/$pid/environ"; } 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=$IRICE_RT"; then
            echo "$pid"
        fi
    done
    return 0
}

# Stops the apps opened inside the sandbox (terminals, image viewers, …): every
# process of ours running with the sandbox's runtime dir, except the compositor
# and shells, which their own stops handle. Apps left behind when the nested
# compositor goes can spin at full CPU (imv does), so `down` and `up` clear them.
stop_sandbox_apps() {
    local pid pids=""
    for pid in $(pgrep -u "$(id -u)"); do
        [[ $pid == "$$" || $pid == "$PPID" ]] && continue
        case "$(cat "/proc/$pid/comm" 2>/dev/null)" in
            Hyprland | start-hyprland | irice-shell | qs | quickshell | "") continue ;;
        esac
        grep -qzx "XDG_RUNTIME_DIR=$IRICE_RT" "/proc/$pid/environ" 2>/dev/null && pids+=" $pid"
    done
    [[ -z "$pids" ]] && return 0
    kill $pids 2>/dev/null || true
    for _ in $(seq 30); do
        pids="$(for p in $pids; do if kill -0 "$p" 2>/dev/null; then echo "$p"; fi; done)"
        [[ -z "$pids" ]] && return 0
        sleep 0.1
    done
    kill -9 $pids 2>/dev/null || true
}

# Stops every sandbox Quickshell and waits for them to exit.
stop_sandbox_shells() {
    local pids
    # (not the look-switching or update screens: they cover the restart)
    pids="$(for p in $(sandbox_pids irice-shell; sandbox_pids qs; sandbox_pids quickshell); do
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -q -e '/irice-look\.' -e '/irice-update\.' || echo "$p"
    done)"
    [[ -z "$pids" ]] && return 0
    kill $pids 2>/dev/null || true
    for _ in $(seq 30); do
        pids="$(for p in $pids; do if kill -0 "$p" 2>/dev/null; then echo "$p"; fi; done)"
        [[ -z "$pids" ]] && return 0
        sleep 0.1
    done
    kill -9 $pids 2>/dev/null || true
}
