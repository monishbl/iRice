#!/usr/bin/env bash
# One idle step from hypridle (start-idle.sh writes the listeners from
# Settings → Battery & Power): the step only happens on the power source it
# was set for, so the charger's and the battery's timers can differ while
# hypridle has one list.
#   idle-action.sh <ac|battery|any> <dim|undim|lock|off|on|sleep>
#   idle-action.sh before-sleep | after-sleep     (the password-after-sleep grace)
# The lock command and the grace come from the files start-idle.sh writes
# next to its hypridle config ($IRICE_IDLE_DIR).
set -uo pipefail
dir=${IRICE_IDLE_DIR:-}

# On the charger? A computer with no mains supply at all (a desktop) counts
# as on the charger.
on_ac() {
    local seen=0 f
    for f in /sys/class/power_supply/*; do
        [[ $(cat "$f/type" 2>/dev/null) == Mains ]] || continue
        seen=1
        [[ $(cat "$f/online" 2>/dev/null) == 1 ]] && return 0
    done
    [[ $seen == 0 ]]
}
lock() {
    [[ -n $dir && -r $dir/lock-command ]] && sh -c "$(cat "$dir/lock-command")"
}

case "${1:-}" in
    before-sleep)
        # Asleep longer than the grace (or no grace): locked before it sleeps,
        # as always. With a grace, the time is noted and the check waits for
        # the wake (after-sleep).
        grace=$(cat "$dir/grace" 2>/dev/null || echo 0)
        if [[ ${grace:-0} -le 0 ]]; then
            lock
        else
            date +%s >"$dir/slept-at"
        fi
        exit 0 ;;
    after-sleep)
        hyprctl dispatch 'hl.dsp.dpms({ action = "enable" })' >/dev/null 2>&1
        grace=$(cat "$dir/grace" 2>/dev/null || echo 0)
        if [[ ${grace:-0} -gt 0 ]]; then
            slept=$(cat "$dir/slept-at" 2>/dev/null || echo 0)
            rm -f "$dir/slept-at"
            (( $(date +%s) - slept >= grace )) && lock
        fi
        exit 0 ;;
esac

source=${1:-any} action=${2:-}
case "$source" in
    ac) on_ac || exit 0 ;;
    battery) on_ac && exit 0 ;;
esac
case "$action" in
    dim)
        # Down to a tenth (never brighter), remembered so undim puts it back
        if command -v brightnessctl >/dev/null; then
            cur=$(brightnessctl get 2>/dev/null) max=$(brightnessctl max 2>/dev/null)
            [[ -n $cur && -n $max ]] && (( cur > max / 10 )) && brightnessctl -q -s set "$((max / 10))"
        fi ;;
    undim) command -v brightnessctl >/dev/null && brightnessctl -q -r ;;
    lock) lock ;;
    off) hyprctl dispatch 'hl.dsp.dpms({ action = "disable" })' >/dev/null 2>&1 ;;
    on) hyprctl dispatch 'hl.dsp.dpms({ action = "enable" })' >/dev/null 2>&1 ;;
    sleep) systemctl suspend ;;
esac
exit 0
