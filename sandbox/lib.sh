# Sourced by dev.sh and reload-shell.sh.
# distrobox shares the host PID namespace, so sandbox processes are identified by
# their environment: only they run with XDG_RUNTIME_DIR=/tmp/irice-rt. Host
# processes (e.g. the real end4 `qs -c ii`) can never match.

IRICE_RT=/tmp/irice-rt

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
            Hyprland | start-hyprland | qs | quickshell | "") continue ;;
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
    pids="$(sandbox_pids qs; sandbox_pids quickshell)"
    [[ -z "$pids" ]] && return 0
    kill $pids 2>/dev/null || true
    for _ in $(seq 30); do
        pids="$(for p in $pids; do if kill -0 "$p" 2>/dev/null; then echo "$p"; fi; done)"
        [[ -z "$pids" ]] && return 0
        sleep 0.1
    done
    kill -9 $pids 2>/dev/null || true
}
