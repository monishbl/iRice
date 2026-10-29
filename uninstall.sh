#!/usr/bin/env bash
# Removes iRice and restores the Hyprland config backed up by install.sh.
#   ./uninstall.sh [--yes] [--dry-run]
# Packages are left installed; your iRice settings in ~/.config/irice are kept.
set -euo pipefail

CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
YES=0 DRY=0
for a in "$@"; do
    case "$a" in
        --yes|-y) YES=1 ;;
        --dry-run) DRY=1 ;;
        -h|--help) sed -n '2,4p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $a (see --help)"; exit 1 ;;
    esac
done
run() { if [[ $DRY == 1 ]]; then printf '  would run: %s\n' "$*"; else "$@"; fi; }

# The config from before iRice (older installers made timestamped backups;
# the oldest of those is the one from before iRice)
BACKUP=""
if [[ -d "$CONFIG/hypr.pre-irice" ]]; then
    BACKUP="$CONFIG/hypr.pre-irice"
else
    BACKUP="$(ls -d "$CONFIG"/hypr.backup-* 2>/dev/null | sort | head -1 || true)"
fi
# Already uninstalled (no iRice config in place): nothing to swap back
[[ -d "$CONFIG/hypr/irice" ]] || BACKUP=""
echo "This will remove $CONFIG/quickshell/irice, the glass plugin, the irice command and the iRice login screen"
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
# The iRice login screen: switch back to the login manager it replaced
# (or none: then log in on a console and start your desktop yourself)
if [[ -d /usr/share/irice ]]; then
    PREV="$(cat "$DATA/irice/previous-login-manager" 2>/dev/null || true)"
    if systemctl is-enabled --quiet greetd 2>/dev/null && grep -q irice /etc/greetd/config.toml 2>/dev/null; then
        run sudo systemctl disable greetd
        [[ -n "$PREV" ]] && run sudo systemctl enable "$PREV" && echo "Your login screen ($PREV) is back."
    fi
    run sudo rm -rf /usr/share/irice
    if [[ -f /etc/greetd/config.toml.pre-irice ]]; then
        run sudo mv /etc/greetd/config.toml.pre-irice /etc/greetd/config.toml
    fi
fi
# iRice's root helper and its polkit action and rule
if [[ -e /usr/lib/irice || -e /etc/polkit-1/rules.d/49-irice.rules || -e /usr/share/polkit-1/actions/org.irice.root.policy ]]; then
    run sudo rm -rf /usr/lib/irice
    run sudo rm -f /etc/polkit-1/rules.d/49-irice.rules /usr/share/polkit-1/actions/org.irice.root.policy
fi
# The glass plugin and its source (the running session keeps its loaded copy
# until you log out), and the installer's bookkeeping. Settings in ~/.config/irice stay.
run rm -f "$DATA/irice/irice-glass.so" "$DATA/irice/irice-glass.so.new" "$DATA/irice/irice-glass.built-for"
run rm -rf "$DATA/irice/src"
run rm -f "$DATA/irice/source" "$DATA/irice/previous-login-manager"
# The irice command, and starting Hyprland from tty1
run rm -f "$HOME/.local/bin/irice" "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/irice.fish"
if grep -q 'irice: start Hyprland' "$HOME/.bash_profile" 2>/dev/null; then
    run sed -i '/# irice: start Hyprland/,+1d' "$HOME/.bash_profile"
fi
echo "Done. Log out and back in to start your previous setup."
