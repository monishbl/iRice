#!/usr/bin/env bash
# Starts an app in a scope of its own, in the user's app.slice (as GNOME and
# KDE do), for the shell (common/Launch.qml), keybinds and autostart:
#   app-scope.sh [--id ID] -- command [args...]
# systemd then shares the CPU, memory and I/O between the desktop (Hyprland
# and this shell, in the login session) and the apps as separate groups, so
# an app working flat out can't take the frames, the menus or typing away
# from the desktop, whatever priority anything was given (an auto-nice
# daemon's rules apply within a group, not across them). Without a systemd
# user manager (or inside the dev sandbox's box) it just runs the app.
id=""
if [[ "${1:-}" == --id ]]; then
    id="$2"
    shift 2
fi
[[ "${1:-}" == -- ]] && shift
[[ $# -gt 0 ]] || exit 0
if [[ -S "${XDG_RUNTIME_DIR:-/nonexistent}/systemd/private" ]] && command -v systemd-run >/dev/null 2>&1; then
    # The name systemd's desktop convention uses: app-<launcher>-<app>-<random>.scope
    [[ -n "$id" ]] || id="${1##*/}"
    id=$(printf '%s' "${id%.desktop}" | tr -c 'A-Za-z0-9_.' '_' | cut -c1-60)
    exec systemd-run --user --scope --quiet --collect --slice=app.slice \
        --unit="app-irice-${id:-app}-$(printf '%04x%04x' $RANDOM $RANDOM).scope" --description="${id:-app}" -- "$@"
fi
exec "$@"
