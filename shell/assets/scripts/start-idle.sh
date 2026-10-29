#!/usr/bin/env bash
# One independent idle/PrepareForSleep listener per compositor, across shell restarts.
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
printf -v lock_command '%q -p %q ipc call lock lock' "${1:-qs}" "$shell_dir"
# Keep installed/custom listener timeouts and DPMS behavior. Only the lock
# target and secure-before-sleep requirement are owned by this launcher.
user_config="${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hypridle.conf"
if [[ -r "$user_config" ]]; then
    cat -- "$user_config" >"$idle_dir/hypridle.conf"
else
    cat >"$idle_dir/hypridle.conf" <<EOF
general {
    after_sleep_cmd = hyprctl dispatch 'hl.dsp.dpms({ action = "enable" })'
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
cat >>"$idle_dir/hypridle.conf" <<EOF

general {
    lock_cmd = $lock_command
    before_sleep_cmd = $lock_command
    inhibit_sleep = 3
}
EOF
exec hypridle -c "$idle_dir/hypridle.conf"
