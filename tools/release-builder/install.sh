#!/bin/bash
# Installs the release builder on the build machine (server1) from this
# checkout: ~/bin/tessera-release and its systemd user timer. Run it there:
#   tools/release-builder/install.sh             install or update, timer on
#   tools/release-builder/install.sh --no-timer  the script only
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
install -Dm755 "$here/tessera-release" "$HOME/bin/tessera-release"
[[ "${1:-}" == --no-timer ]] && exit 0
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
install -Dm644 "$here/tessera-release.service" "$units/tessera-release.service"
install -Dm644 "$here/tessera-release.timer" "$units/tessera-release.timer"
systemctl --user daemon-reload
systemctl --user enable --now tessera-release.timer
systemctl --user list-timers tessera-release.timer --no-pager
