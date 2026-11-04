#!/usr/bin/env bash
# Removes Tessera and restores the Hyprland config backed up by install.sh.
#   ./uninstall.sh [--yes] [--dry-run]
# Packages are left installed; your Tessera settings in ~/.config/tessera are kept.
# What an install from before iRice became Tessera left under iRice's names
# (irice, /usr/lib/irice, org.irice.*, /usr/share/irice…) goes too.
set -euo pipefail

CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
# (TESSERA_SYSTEM_ROOT: where /etc and /usr are, for the tests)
S="${TESSERA_SYSTEM_ROOT:-}"
YES=0 DRY=0
for a in "$@"; do
    case "$a" in
        --yes|-y) YES=1 ;;
        --dry-run) DRY=1 ;;
        -h|--help) sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $a (see --help)"; exit 1 ;;
    esac
done
run() { if [[ $DRY == 1 ]]; then printf '  would run: %s\n' "$*"; else "$@"; fi; }

# The config from before Tessera (older installers made timestamped backups;
# the oldest of those is the one from before Tessera; .pre-irice: kept by an
# install from before the rename)
BACKUP=""
if [[ -d "$CONFIG/hypr.pre-tessera" ]]; then
    BACKUP="$CONFIG/hypr.pre-tessera"
elif [[ -d "$CONFIG/hypr.pre-irice" ]]; then
    BACKUP="$CONFIG/hypr.pre-irice"
else
    BACKUP="$(ls -d "$CONFIG"/hypr.backup-* 2>/dev/null | sort | head -1 || true)"
fi
# Already uninstalled (no Tessera config in place): nothing to swap back
[[ -d "$CONFIG/hypr/tessera" || -d "$CONFIG/hypr/irice" ]] || BACKUP=""
echo "This will remove $CONFIG/quickshell/tessera, the glass plugin, the tessera command and the Tessera login screen"
if [[ -n "$BACKUP" ]]; then
    echo "and restore $CONFIG/hypr from $BACKUP (the current one moves to hypr.tessera-removed)."
else
    echo "No Hyprland backup was found, so $CONFIG/hypr is left as it is."
fi
if [[ $YES == 0 && $DRY == 0 ]]; then
    read -r -p "Continue? [y/N] " reply
    [[ "$reply" == [yY]* ]] || { echo "Nothing changed."; exit 0; }
fi

run rm -rf "$CONFIG/quickshell/tessera" "$CONFIG/quickshell/irice"
if [[ -n "$BACKUP" ]]; then
    run rm -rf "$CONFIG/hypr.tessera-removed"
    run mv "$CONFIG/hypr" "$CONFIG/hypr.tessera-removed"
    run cp -a "$BACKUP" "$CONFIG/hypr"
fi
# The Tessera login screen: switch back to the login manager it replaced
# (or none: then log in on a console and start your desktop yourself)
if [[ -d "$S/usr/share/tessera" || -d "$S/usr/share/irice" ]]; then
    PREV="$(cat "$DATA/tessera/previous-login-manager" 2>/dev/null || cat "$DATA/irice/previous-login-manager" 2>/dev/null || true)"
    if systemctl is-enabled --quiet greetd 2>/dev/null && grep -q -e tessera -e irice "$S/etc/greetd/config.toml" 2>/dev/null; then
        run sudo systemctl disable greetd
        [[ -n "$PREV" ]] && run sudo systemctl enable "$PREV" && echo "Your login screen ($PREV) is back."
    fi
    run sudo rm -rf "$S/usr/share/tessera" "$S/usr/share/irice"
    for b in "$S/etc/greetd/config.toml.pre-tessera" "$S/etc/greetd/config.toml.pre-irice"; do
        if [[ -f $b ]]; then
            run sudo mv "$b" "$S/etc/greetd/config.toml"
            break
        fi
    done
fi
# The login screen's entry for a Tessera run from its folder (tools/setup-session.sh)
run sudo rm -f "$S/usr/share/wayland-sessions/tessera.desktop" "$S/usr/share/wayland-sessions/irice.desktop"
# The quiet start (install.sh's quiet_boot): Plymouth quits as before, and
# GRUB's kernel line is as it was
for d in "$S/etc/systemd/system/plymouth-quit.service.d/tessera.conf" "$S/etc/systemd/system/plymouth-quit.service.d/irice.conf"; do
    if [[ -f $d ]]; then
        run sudo rm -f "$d"
        run sudo systemctl daemon-reload
    fi
done
for b in "$S/etc/default/grub.pre-tessera" "$S/etc/default/grub.pre-irice"; do
    if [[ -f $b ]]; then
        run sudo mv "$b" "$S/etc/default/grub"
        command -v grub-mkconfig >/dev/null && [[ -z $S ]] && run sudo grub-mkconfig -o /boot/grub/grub.cfg >/dev/null
        break
    fi
done
# Unlocking the keyring at login (install.sh's lines under its marker, in the
# login screen's or the console's PAM file), and the copies made before
for f in "$S/etc/pam.d/greetd" "$S/etc/pam.d/login"; do
    for mark in tessera irice; do
        if grep -q "# $mark: unlock the keyring" "$f" 2>/dev/null; then
            run sudo sed -i "/# $mark: unlock the keyring/,+1d" "$f"
            run sudo rm -f "$f.pre-tessera" "$f.pre-irice"
        fi
    done
done
# The boot animation: the system's own again when it's one of Tessera's (the
# helper rebuilds the boot image, and puts it back as it was if that fails)
helper=""
for h in "$S/usr/lib/tessera/tessera-root" "$S/usr/lib/irice/irice-root"; do
    [[ -x $h ]] && { helper=$h; break; }
done
if [[ -n $helper ]] && grep -qE '^[[:space:]]*Theme[[:space:]]*=[[:space:]]*(tessera|irice)-' "$S/etc/plymouth/plymouthd.conf" 2>/dev/null; then
    run sudo "$helper" bootsplash system ||
        echo "  The boot animation stays Tessera's (it still works). To change it: sudo plymouth-set-default-theme -R <theme>"
fi
# Tessera's root helper and its polkit actions and rules, its ananicy rule
# and what it keeps in /etc (iRice's names too)
for p in /usr/lib/tessera /usr/lib/irice /etc/polkit-1/rules.d/49-tessera.rules /etc/polkit-1/rules.d/49-irice.rules \
    /etc/polkit-1/rules.d/49-tessera-quickshare.rules /etc/polkit-1/rules.d/49-irice-quickshare.rules \
    /usr/share/polkit-1/actions/org.tessera.root.policy /usr/share/polkit-1/actions/org.irice.root.policy \
    /etc/ananicy.d/tessera.rules /etc/ananicy.d/irice.rules /etc/tessera /etc/irice; do
    [[ -e "$S$p" || -L "$S$p" ]] && run sudo rm -rf "$S$p"
done
# The glass plugin and its source (the running session keeps its loaded copy
# until you log out), and the installer's bookkeeping. Settings in ~/.config/tessera stay.
for d in "$DATA/tessera" "$DATA/irice"; do
    run rm -f "$d/tessera-glass.so" "$d/tessera-glass.so.new" "$d/tessera-glass.built-for" \
        "$d/irice-glass.so" "$d/irice-glass.so.new" "$d/irice-glass.built-for"
    run rm -rf "$d/src"
    run rm -f "$d/bin/tessera-events" "$d/bin/tessera-events.new" "$d/bin/irice-events" "$d/bin/irice-events.new"
    run rm -f "$d/source" "$d/previous-login-manager"
done
# The tessera command (irice, before the rename), and starting Hyprland from tty1
run rm -f "$HOME/.local/bin/tessera" "$HOME/.local/bin/irice" \
    "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/tessera.fish" "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/irice.fish" \
    "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/tessera-start-hyprland.fish" "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/irice-start-hyprland.fish"
for f in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile" "${ZDOTDIR:-$HOME}/.zprofile"; do
    for mark in tessera irice; do
        if grep -q "$mark: start Hyprland" "$f" 2>/dev/null; then
            run sed -i "/# $mark: start Hyprland/,+1d" "$f"
        fi
    done
done
echo "Done. Log out and back in to start your previous setup."
