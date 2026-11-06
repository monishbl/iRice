# Sourced by dev.sh, inner.sh and reload-shell.sh (with ROOT set).
# distrobox shares the host PID namespace, so sandbox processes are identified by
# their environment: only they run with XDG_RUNTIME_DIR=/tmp/irice-rt. Host
# processes (e.g. the real end4 `qs -c ii`) can never match.
# TESSERA_ARENA=2, 3, … is another identical sandbox beside the first, with its
# own runtime folder (/tmp/irice-rt-N) and home (.sandbox/arena-N/home), so
# checks can run side by side. 1 (the default) is the usual sandbox.

# (IRICE_ARENA: its name before iRice became Tessera. The runtime folders,
# /tmp/irice-rt*, and the headless IRICE-TEST output keep their names: every
# checkout, from before the rename or after, shares the same arenas safely)
TESSERA_ARENA="${TESSERA_ARENA:-${IRICE_ARENA:-1}}"
[[ "$TESSERA_ARENA" =~ ^[1-9]$ ]] || { echo "TESSERA_ARENA must be 1-9" >&2; exit 2; }
if [[ "$TESSERA_ARENA" == 1 ]]; then
    TESSERA_RT=/tmp/irice-rt
    TESSERA_HOME="$ROOT/.sandbox/home"
else
    TESSERA_RT=/tmp/irice-rt-$TESSERA_ARENA
    TESSERA_HOME="$ROOT/.sandbox/arena-$TESSERA_ARENA/home"
fi
# (the same under their old names, for a checkout's scripts from before the
# rename that source this file: its reload-shell.sh, given this checkout)
IRICE_ARENA=$TESSERA_ARENA IRICE_RT=$TESSERA_RT IRICE_HOME=$TESSERA_HOME

# PIDs of sandbox processes with the given executable name.
sandbox_pids() {
    local name="$1" pid
    for pid in $(pgrep -x "$name"); do
        # Unreadable environ (host processes, from inside the container) = not ours.
        if { tr '\0' '\n' <"/proc/$pid/environ"; } 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=$TESSERA_RT"; then
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
            Hyprland | start-hyprland | tessera-shell | irice-shell | qs | quickshell | "") continue ;;
        esac
        grep -qzx "XDG_RUNTIME_DIR=$TESSERA_RT" "/proc/$pid/environ" 2>/dev/null && pids+=" $pid"
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
    # (tessera-ui too: standalone it is the shell, and a reload must restart it;
    # beside the QML it would end with its shell anyway. "tessera-ui.real" is
    # the verification script's wrapped binary)
    pids="$(for p in $(sandbox_pids tessera-shell; sandbox_pids irice-shell; sandbox_pids qs; sandbox_pids quickshell; sandbox_pids tessera-ui; sandbox_pids tessera-ui.real); do
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -q -e '/tessera-look\.' -e '/tessera-update\.' || echo "$p"
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
