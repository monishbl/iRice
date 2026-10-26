#!/usr/bin/env bash
# ./dev.sh perf [seconds per phase] [phase...]
# Runs a scripted session in the sandbox and prints, per phase, what the shell
# and the nested Hyprland cost: CPU %, memory, frames drawn, glass copies.
# A phase that should be still (idle, hot) should draw ~0 frames.
# Results are appended to .sandbox/perf.log, so changes can be compared.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV="$ROOT/dev.sh"
SECS="${1:-8}"
shift || true
PHASES=("${@:-idle controlCenter overview missionControl launchpad island hot music}")
read -ra PHASES <<<"${PHASES[*]}"
HZ=$(getconf CLK_TCK)

hypr_pid() { pgrep -P "$("$DEV" status | grep -o '[0-9]\+')" -x Hyprland | head -1; }
shell_pid() {
    local p
    for p in $(pgrep -x qs; pgrep -x quickshell); do
        tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | grep -q -- "-p $ROOT/shell" &&
            tr '\0' '\n' <"/proc/$p/environ" 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=/tmp/irice-rt" && { echo "$p"; return; }
    done
}
ticks() { awk '{print $14 + $15}' "/proc/$1/stat" 2>/dev/null || echo 0; }
rss_mb() { awk '/^VmRSS/ {printf "%.0f", $2 / 1024}' "/proc/$1/status" 2>/dev/null || echo 0; }
stats() { "$DEV" hyprctl irice-glass stats 2>/dev/null; }
field() { grep -o "$1 [0-9]*" <<<"$2" | head -1 | awk '{print $2}'; }
frames() { grep -o 'frames:IRICE-TEST [0-9]*' <<<"$1" | awk '{print $2}'; }
# Whole-GPU busy share from the render engine's idle (RC6) time: includes your
# own desktop, so compare phases rather than reading it as the shell's alone
rc6() { cat /sys/class/drm/card*/power/rc6_residency_ms 2>/dev/null | head -1 || echo 0; }

ipc() { "$DEV" ipc "$@" >/dev/null 2>&1; }
start_phase() {
    case "$1" in
        idle) ;;
        controlCenter | overview | missionControl | launchpad | notificationCenter | spotlight) ipc panels open "$1" "" ;;
        island) ipc island demo burst ;;
        hot) ipc island hot true ;;
        music)
            DBUS_SESSION_BUS_ADDRESS="$(cat /tmp/irice-rt/session-bus)" python3 "$ROOT/sandbox/tools/fakeplayer.py" >/dev/null 2>&1 &
            FAKE=$!
            ;;
    esac
}
end_phase() {
    case "$1" in
        hot) ipc island hot false ;;
        music) kill "${FAKE:-}" 2>/dev/null ;;
        idle | island) ;;
        *) ipc panels close ;;
    esac
}

HP=$(hypr_pid)
SP=$(shell_pid)
[[ -n "$HP" && -n "$SP" ]] || { echo "Sandbox or shell not running (./dev.sh up)"; exit 1; }
[[ -n "$(frames "$(stats)")" ]] || echo "(glass plugin not loaded: no frame counts)"

printf '%-15s %8s %8s %8s %8s %7s %8s %6s\n' phase "shell%" "hypr%" "shellMB" "frames/s" blits/s "copyMP/s" "gpu%"
{
    echo "== $(date '+%F %T') $(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null) ${SECS}s/phase"
} >>"$ROOT/.sandbox/perf.log"
for ph in "${PHASES[@]}"; do
    ipc panels close
    sleep 1
    start_phase "$ph"
    sleep 1.5 # let open animations finish; measure the steady state
    s0=$(stats); a0=$(ticks "$SP"); b0=$(ticks "$HP"); g0=$(rc6); t0=$(date +%s.%N)
    sleep "$SECS"
    s1=$(stats); a1=$(ticks "$SP"); b1=$(ticks "$HP"); g1=$(rc6); t1=$(date +%s.%N)
    end_phase "$ph"
    line=$(awk -v ph="$ph" -v hz="$HZ" -v dt="$(echo "$t1 - $t0" | bc)" \
        -v a="$((a1 - a0))" -v b="$((b1 - b0))" -v mb="$(rss_mb "$SP")" \
        -v f="$(( $(frames "$s1" || echo 0) - $(frames "$s0" || echo 0) ))" \
        -v bl="$(( $(field blits "$s1" || echo 0) - $(field blits "$s0" || echo 0) ))" \
        -v px="$(( $(field blitpx "$s1" || echo 0) - $(field blitpx "$s0" || echo 0) ))" \
        -v g="$((g1 - g0))" \
        'BEGIN { printf "%-15s %8.1f %8.1f %8d %8.1f %7.1f %8.2f %6.0f\n", ph, 100*a/hz/dt, 100*b/hz/dt, mb, f/dt, bl/dt, px/dt/1e6, 100 - g/(dt*10) }')
    echo "$line"
    echo "$line" >>"$ROOT/.sandbox/perf.log"
done
ipc panels close
