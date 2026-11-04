#!/bin/sh
# greetd's greeter command (/etc/greetd/config.toml, from ./install.sh): the
# login screen's compositor (greeter.lua), on the console the boot ends on.
# That console is cleared and its cursor hidden first, and the compositor's
# log goes to the journal (journalctl -t tessera-greeter), not onto the
# screen, so nothing but the splash, the login screen and your desktop shows.
printf '\033[2J\033[H\033[?25l'
if command -v start-hyprland >/dev/null; then
    set -- start-hyprland -- --config /usr/share/tessera/greeter.lua
else
    set -- Hyprland --config /usr/share/tessera/greeter.lua
fi
if command -v systemd-cat >/dev/null; then
    exec systemd-cat -t tessera-greeter "$@"
fi
exec "$@" >/dev/null 2>&1
