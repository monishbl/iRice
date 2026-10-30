#!/usr/bin/env bash
# One independent idle/PrepareForSleep listener per compositor, across shell restarts.
#
# The timers come from Settings → Battery & Power: the shell writes them to
# $XDG_CONFIG_HOME/irice/idle-listeners ("<ac|battery> <dim|lock|off|sleep>
# <seconds>" per line, plus "grace <seconds>" for the password after sleep)
# and ends hypridle (its pid is in the idle folder); this loop starts it again
# with the new list. A hypridle.conf of your own still wins over all of it.
# The loop ends with its compositor.
set -euo pipefail
[[ ${IRICE_SANDBOX:-0} != 1 || ${IRICE_TTY:-0} == 1 ]] || exit 0
command -v hypridle >/dev/null || exit 0
runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
instance=${HYPRLAND_INSTANCE_SIGNATURE:?}
idle_dir="$runtime/irice-idle-$instance"
umask 077
mkdir -p "$idle_dir"
exec 9>"$idle_dir/lock"
flock -n 9 || exit 0
shell_dir=$(cd -- "$(dirname -- "$0")/../.." && pwd)
action="$shell_dir/assets/scripts/idle-action.sh"
printf -v lock_command '%q -p %q ipc call lock lock' "${1:-qs}" "$shell_dir"
printf '%s\n' "$lock_command" >"$idle_dir/lock-command"
export IRICE_IDLE_DIR="$idle_dir"
user_config="${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hypridle.conf"
listeners="${XDG_CONFIG_HOME:-$HOME/.config}/irice/idle-listeners"

write_config() {
    local grace=0
    # Keep installed/custom listener timeouts and DPMS behavior. Only the lock
    # target and secure-before-sleep requirement are owned by this launcher.
    # (a copy an earlier installer put there doesn't count: own-hypridle.sh)
    if "$shell_dir/assets/scripts/own-hypridle.sh" "$user_config"; then
        cat -- "$user_config" >"$idle_dir/hypridle.conf"
    elif [[ -r "$listeners" ]]; then
        {
            echo "general {"
            echo "    ignore_dbus_inhibit = false"
            echo "}"
            local src act secs resume
            while read -r src act secs; do
                if [[ $src == grace ]]; then
                    [[ $act =~ ^[0-9]+$ ]] && grace=$act
                    continue
                fi
                [[ $src =~ ^(ac|battery)$ && $act =~ ^(dim|lock|off|sleep)$ && $secs =~ ^[0-9]+$ ]] || continue
                resume=""
                [[ $act == dim ]] && resume="$action any undim"
                [[ $act == off ]] && resume="$action any on"
                echo "listener {"
                echo "    timeout = $secs"
                echo "    on-timeout = $action $src $act"
                [[ -n $resume ]] && echo "    on-resume = $resume"
                echo "}"
            done <"$listeners"
        } >"$idle_dir/hypridle.conf"
    else
        cat >"$idle_dir/hypridle.conf" <<EOF
general {
    ignore_dbus_inhibit = false
}
listener {
    timeout = 300
    on-timeout = $lock_command
}
listener {
    timeout = 600
    on-timeout = hyprctl dispatch 'hl.dsp.dpms({ action = "disable" })'
    on-resume = hyprctl dispatch 'hl.dsp.dpms({ action = "enable" })'
}
listener {
    timeout = 1800
    on-timeout = systemctl suspend
}
EOF
    fi
    printf '%s\n' "$grace" >"$idle_dir/grace"
    # Locked before sleeping, unless there's a grace: then the wake decides
    local before="$lock_command"
    [[ $grace -gt 0 ]] && printf -v before '%q before-sleep' "$action"
    cat >>"$idle_dir/hypridle.conf" <<EOF

general {
    lock_cmd = $lock_command
    before_sleep_cmd = $before
    after_sleep_cmd = $action after-sleep
    inhibit_sleep = 3
}
EOF
}

while :; do
    write_config
    hypridle -c "$idle_dir/hypridle.conf" &
    echo $! >"$idle_dir/hypridle.pid"
    wait $! || true
    rm -f "$idle_dir/hypridle.pid"
    # Its compositor is gone: so is this loop
    [[ -S "$runtime/hypr/$instance/.socket.sock" ]] || exit 0
    sleep 1
done
