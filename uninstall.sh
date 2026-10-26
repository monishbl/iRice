#!/usr/bin/env bash
# Removes iRice and restores the Hyprland config backed up by install.sh.
#   ./uninstall.sh [--yes] [--dry-run]
# Packages are left installed; your iRice settings in ~/.config/irice are kept.
set -euo pipefail

CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
YES=0 DRY=0
for a in "$@"; do
    case "$a" in
        --yes|-y) YES=1 ;;
        --dry-run) DRY=1 ;;
        *) echo "Unknown option: $a"; exit 1 ;;
    esac
done
run() { if [[ $DRY == 1 ]]; then printf '  would run: %s\n' "$*"; else "$@"; fi; }

BACKUP="$(ls -d "$CONFIG"/hypr.backup-* 2>/dev/null | sort | tail -1 || true)"
echo "This will remove $CONFIG/quickshell/irice"
if [[ -n "$BACKUP" ]]; then
    echo "and restore $CONFIG/hypr from $BACKUP (the current one moves to hypr.irice-removed)."
else
    echo "No Hyprland backup was found, so $CONFIG/hypr is left as it is."
fi
if [[ $YES == 0 && $DRY == 0 ]]; then
    read -r -p "Continue? [y/N] " reply
    [[ "$reply" == [yY]* ]] || { echo "Nothing changed."; exit 0; }
fi

run rm -rf "$CONFIG/quickshell/irice"
if [[ -n "$BACKUP" ]]; then
    run rm -rf "$CONFIG/hypr.irice-removed"
    run mv "$CONFIG/hypr" "$CONFIG/hypr.irice-removed"
    run cp -a "$BACKUP" "$CONFIG/hypr"
fi
# Login screen (./install.sh --greeter): only remove it when greetd isn't in use
if [[ -d /usr/share/irice ]]; then
    if systemctl is-enabled --quiet greetd 2>/dev/null; then
        echo "The iRice login screen is still switched on. Switch back to your previous display"
        echo "manager first (e.g. sudo systemctl disable greetd && sudo systemctl enable sddm),"
        echo "then run this again to remove /usr/share/irice."
    else
        run sudo rm -rf /usr/share/irice
        if [[ -f /etc/greetd/config.toml.pre-irice ]]; then
            run sudo mv /etc/greetd/config.toml.pre-irice /etc/greetd/config.toml
        fi
    fi
fi
echo "Done. Log out and back in to start your previous setup."
