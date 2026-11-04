#!/usr/bin/env bash
# One independent idle/PrepareForSleep listener per compositor, across shell restarts.
#
# The timers come from Settings → Battery & Power: the shell writes them to
# $XDG_CONFIG_HOME/tessera/idle-listeners ("<ac|battery> <dim|lock|off|sleep>
# <seconds>" per line, plus "grace <seconds>" for the password after sleep)
# and ends hypridle (its pid is in the idle folder); this loop starts it again
# with the new list. A hypridle.conf of your own still wins over all of it.
# The loop ends with its compositor.
set -euo pipefail
[[ ${TESSERA_SANDBOX:-${IRICE_SANDBOX:-0}} != 1 || ${TESSERA_TTY:-${IRICE_TTY:-0}} == 1 ]] || exit 0
command -v hypridle >/dev/null || exit 0
runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
instance=${HYPRLAND_INSTANCE_SIGNATURE:?}
idle_dir="$runtime/tessera-idle-$instance"
umask 077
mkdir -p "$idle_dir"
# One started before iRice became Tessera (its folder: irice-idle-…) is
# replaced: two would each dim, lock and sleep
old_dir="$runtime/irice-idle-$instance"
if [[ -e "$old_dir/lock" ]] && command -v fuser >/dev/null; then
    for p in $(fuser "$old_dir/lock" 2>/dev/null); do
        if [[ $p != "$$" ]]; then kill "$p" 2>/dev/null || true; fi
    done
    rm -rf "$old_dir"
fi
exec 9>"$idle_dir/lock"
if ! flock -n 9; then
    # One runs already. One from before shell-ipc.sh locks through a single
    # shell folder, so it couldn't reach the Material look's root: replace it
    # (it and its hypridle hold the lock), else leave it be
    old=$(cat "$idle_dir/lock-command" 2>/dev/null || true)
    [[ -n $old && $old != *shell-ipc.sh* ]] && command -v fuser >/dev/null || exit 0
    # (set -e: a holder already gone, like the $(…) below, mustn't end this)
    for p in $(fuser "$idle_dir/lock" 2>/dev/null); do
        if [[ $p != "$$" ]]; then kill "$p" 2>/dev/null || true; fi
    done
    flock -w 3 9 || exit 0
fi
shell_dir=$(cd -- "$(dirname -- "$0")/../.." && pwd)
action="$shell_dir/assets/scripts/idle-action.sh"
# (through shell-ipc.sh: the shell may run either look's root)
printf -v lock_command 'QS=%q %q call lock lock' "${1:-qs}" "$shell_dir/assets/scripts/shell-ipc.sh"
printf '%s\n' "$lock_command" >"$idle_dir/lock-command"
export TESSERA_IDLE_DIR="$idle_dir"
user_config="${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hypridle.conf"
# (…/irice: Tessera's folder from before it was renamed, not moved yet)
listeners="${XDG_CONFIG_HOME:-$HOME/.config}/tessera/idle-listeners"
[[ ! -e "${XDG_CONFIG_HOME:-$HOME/.config}/tessera" && -d "${XDG_CONFIG_HOME:-$HOME/.config}/irice" ]] && listeners="${XDG_CONFIG_HOME:-$HOME/.config}/irice/idle-listeners"

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
