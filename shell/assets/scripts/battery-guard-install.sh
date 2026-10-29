#!/usr/bin/env bash
# Turns the critical-battery guard (battery-guard.py) on or off as a systemd
# user service, so it keeps watching even when the shell isn't running. Run
# by the shell (services/Power.qml) at start and when the setting changes;
# it only touches its own unit, and only rewrites it when it changed.
#   battery-guard-install.sh enable | disable | status
set -uo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
units="$HOME/.config/systemd/user" # the user manager's own folder, whatever the session's XDG_CONFIG_HOME
unit="$units/irice-battery-guard.service"
command -v systemctl >/dev/null || { echo "no systemd"; exit 1; }

case "${1:-status}" in
    enable)
        mkdir -p "$units"
        new=$(cat <<EOF
[Unit]
Description=iRice: sleep before the battery runs out
After=upower.service

[Service]
ExecStart=/usr/bin/env python3 -I $here/battery-guard.py --sleep-at 3
Restart=on-failure
RestartSec=30

[Install]
WantedBy=default.target
EOF
)
        if [[ ! -f $unit || $(cat "$unit") != "$new" ]]; then
            printf '%s\n' "$new" >"$unit"
            systemctl --user daemon-reload
            systemctl --user try-restart irice-battery-guard.service 2>/dev/null
        fi
        systemctl --user enable --now irice-battery-guard.service >/dev/null 2>&1
        systemctl --user is-active irice-battery-guard.service ;;
    disable)
        systemctl --user disable --now irice-battery-guard.service >/dev/null 2>&1
        echo inactive ;;
    status)
        systemctl --user is-active irice-battery-guard.service ;;
esac
