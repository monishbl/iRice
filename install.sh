#!/usr/bin/env bash
# iRice installer (Arch-based systems).
#
#   ./install.sh            install dependencies and iRice, asking before changes
#   ./install.sh --yes      don't ask
#   ./install.sh --link     symlink the shell instead of copying (for development)
#   ./install.sh --no-deps  skip pacman
#   ./install.sh --dry-run  print what would happen, change nothing
#   ./install.sh --greeter  also set up the iRice login screen (greetd; needs sudo)
#
# Your current ~/.config/hypr is backed up first; ./uninstall.sh restores it.
# Other Quickshell configs (e.g. end4's ii) are left untouched.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
YES=0 LINK=0 DEPS=1 DRY=0 GREETER=0
for a in "$@"; do
    case "$a" in
        --yes|-y) YES=1 ;;
        --link) LINK=1 ;;
        --no-deps) DEPS=0 ;;
        --dry-run) DRY=1 ;;
        --greeter) GREETER=1 ;;
        -h|--help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $a (see --help)"; exit 1 ;;
    esac
done

PACKAGES=(
    # Compositor and shell
    hyprland hypridle hyprpicker hyprsunset xdg-desktop-portal-hyprland
    # Building the Liquid Glass plugin (against the hyprland package's headers)
    base-devel pkgconf
    qt6-declarative qt6-svg qt6-imageformats qt6-multimedia qt6-multimedia-ffmpeg ffmpeg qt6-5compat qt6-wayland qt6-shadertools
    # System services the shell talks to
    pipewire wireplumber pipewire-pulse networkmanager bluez bluez-utils upower power-profiles-daemon
    gnome-keyring libsecret brightnessctl playerctl libnotify mesa-utils
    # Screen tools, clipboard, input
    grim slurp wl-clipboard cliphist wf-recorder tesseract tesseract-data-eng wtype pacman-contrib
    # Sidebar
    translate-shell curl
    # Look
    inter-font ttf-jetbrains-mono noto-fonts noto-fonts-emoji papirus-icon-theme
)

say() { printf '\033[1m%s\033[0m\n' "$*"; }
run() {
    if [[ $DRY == 1 ]]; then printf '  would run: %s\n' "$*"; else "$@"; fi
}
confirm() {
    [[ $YES == 1 || $DRY == 1 ]] && return 0
    read -r -p "$1 [y/N] " reply
    [[ "$reply" == [yY]* ]]
}

[[ $EUID == 0 ]] && { echo "Run this as your user, not root."; exit 1; }
command -v pacman >/dev/null || { echo "This installer supports Arch-based systems (pacman). See README for manual steps."; exit 1; }

say "iRice will:"
[[ $DEPS == 1 ]] && echo "  • install ${#PACKAGES[@]} packages with pacman (already installed ones are skipped)"
echo "  • install the shell to $CONFIG/quickshell/irice ($([[ $LINK == 1 ]] && echo symlink || echo copy))"
echo "  • back up $CONFIG/hypr, then install iRice's Hyprland config"
echo "  • keep any $CONFIG/hypr/custom.lua for your own tweaks"
[[ $GREETER == 1 ]] && echo "  • set up the iRice login screen in /usr/share/irice and /etc/greetd (sudo; your display manager isn't switched)"
confirm "Continue?" || { echo "Nothing changed."; exit 0; }

if [[ $DEPS == 1 ]]; then
    say "Installing packages"
    # Reuse any installed Quickshell (e.g. end4's pinned build) instead of
    # replacing it; iRice works with 0.2 and newer.
    if ! command -v qs >/dev/null; then
        PACKAGES+=(quickshell)
    else
        echo "  using the installed Quickshell ($(qs --version 2>/dev/null | head -1))"
    fi
    [[ $GREETER == 1 ]] && PACKAGES+=(greetd)
    run sudo pacman -S --needed "${PACKAGES[@]}"
fi

say "Compiling shaders"
run "$ROOT/tools/build-shaders.sh"

say "Installing the shell"
run mkdir -p "$CONFIG/quickshell"
if [[ -e "$CONFIG/quickshell/irice" || -L "$CONFIG/quickshell/irice" ]]; then
    run rm -rf "$CONFIG/quickshell/irice"
fi
if [[ $LINK == 1 ]]; then
    run ln -s "$ROOT/shell" "$CONFIG/quickshell/irice"
else
    run cp -r "$ROOT/shell" "$CONFIG/quickshell/irice"
fi

say "Building the Liquid Glass plugin"
# Kept with its source so Settings can rebuild it after Hyprland updates
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
run mkdir -p "$DATA/irice/src/tools" "$DATA/irice/src/hypr"
run rm -rf "$DATA/irice/src/hypr/plugin"
run cp -r "$ROOT/hypr/plugin" "$DATA/irice/src/hypr/plugin"
run cp "$ROOT/tools/build-plugin.sh" "$DATA/irice/src/tools/"
if [[ $DRY == 1 ]]; then
    echo "  would run: $DATA/irice/src/tools/build-plugin.sh"
elif ! "$DATA/irice/src/tools/build-plugin.sh"; then
    echo "  (skipped: glass will look Frosted until it's built from Settings → Appearance)"
fi

say "Installing the Hyprland config"
if [[ -d "$CONFIG/hypr" ]]; then
    BACKUP="$CONFIG/hypr.backup-$(date +%Y%m%d-%H%M%S)"
    run cp -a "$CONFIG/hypr" "$BACKUP"
    echo "  backed up to $BACKUP"
fi
run mkdir -p "$CONFIG/hypr/irice"
run cp "$ROOT/hypr/hyprland.lua" "$CONFIG/hypr/hyprland.lua"
run cp "$ROOT/hypr/irice/"*.lua "$CONFIG/hypr/irice/"
run cp "$ROOT/hypr/hypridle.conf" "$CONFIG/hypr/hypridle.conf"
# A hyprland.conf next to hyprland.lua would be ambiguous; move it aside
if [[ -f "$CONFIG/hypr/hyprland.conf" ]]; then
    run mv "$CONFIG/hypr/hyprland.conf" "$CONFIG/hypr/hyprland.conf.pre-irice"
fi

if [[ $GREETER == 1 ]]; then
    say "Installing the login screen"
    # The greeter user can't read your home, so it gets its own copy
    WALL=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["wallpaper"]["path"])' "$CONFIG/irice/config.json" 2>/dev/null || true)
    [[ -f "$WALL" ]] || WALL="$ROOT/shell/assets/wallpapers/tahoe-dusk.jpg"
    run sudo mkdir -p /usr/share/irice
    run sudo rm -rf /usr/share/irice/shell
    run sudo cp -r "$ROOT/shell" /usr/share/irice/shell
    run sudo cp "$ROOT/hypr/greeter.lua" /usr/share/irice/greeter.lua
    run sudo cp "$WALL" /usr/share/irice/wallpaper.jpg
    run sudo chmod -R a+rX /usr/share/irice
    if [[ -f /etc/greetd/config.toml ]]; then
        run sudo cp /etc/greetd/config.toml /etc/greetd/config.toml.pre-irice
        echo "  backed up /etc/greetd/config.toml to config.toml.pre-irice"
    fi
    GREETD_CONF='[terminal]
vt = 1

[default_session]
command = "Hyprland --config /usr/share/irice/greeter.lua"
user = "greeter"'
    if [[ $DRY == 1 ]]; then
        echo "  would write /etc/greetd/config.toml"
    else
        printf '%s\n' "$GREETD_CONF" | sudo tee /etc/greetd/config.toml >/dev/null
    fi
    CURRENT_DM=$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service 2>/dev/null || true)
fi

say "Done."
cat <<EOF
Next steps:
  • Enable services if they aren't already:
      sudo systemctl enable --now NetworkManager bluetooth power-profiles-daemon
  • Log out and back in to Hyprland (or run: hyprctl reload && qs -c irice).
  • Open Settings with Super+, — keyboard shortcuts are under Super+/.
  • To go back: ./uninstall.sh
EOF
if [[ $GREETER == 1 ]]; then
    echo "  • The login screen is installed but not switched on. To use it, run:"
    [[ -n "$CURRENT_DM" ]] && echo "      sudo systemctl disable $CURRENT_DM"
    echo "      sudo systemctl enable greetd"
    echo "    then reboot. To undo: sudo systemctl disable greetd${CURRENT_DM:+ && sudo systemctl enable $CURRENT_DM}"
fi
