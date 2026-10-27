#!/usr/bin/env bash
# Sets up iRice to run from this folder on its own console (./dev.sh tty or
# ./demo.sh), without installing it as your desktop: your ~/.config and your
# usual session aren't touched; iRice keeps its settings in .sandbox/home.
#
#   tools/setup-tty.sh [--yes]     packages (asks first), then builds
#
# Run again after a Hyprland or Qt update (it rebuilds what needs it).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
YES=0
[[ "${1:-}" == "--yes" ]] && YES=1
say() { printf '\033[1m%s\033[0m\n' "$*"; }

command -v pacman >/dev/null || { echo "iRice needs Arch Linux (or an Arch-based system such as CachyOS or EndeavourOS)."; exit 1; }

PACKAGES=(
    hyprland hypridle hyprpicker hyprsunset quickshell xdg-desktop-portal-hyprland
    base-devel git pkgconf qt6-shadertools cmake ninja
    qt6-declarative qt6-svg qt6-imageformats qt6-multimedia qt6-multimedia-ffmpeg qt6-5compat qt6-wayland
    pipewire wireplumber networkmanager bluez upower power-profiles-daemon brightnessctl playerctl libnotify
    grim slurp wl-clipboard cliphist wtype python python-gobject jq libqalculate ripgrep zenity
    inter-font noto-fonts noto-fonts-emoji papirus-icon-theme
)
missing=()
for p in "${PACKAGES[@]}"; do pacman -Qq "$p" >/dev/null 2>&1 || missing+=("$p"); done
if (( ${#missing[@]} > 0 )); then
    say "Packages iRice needs: ${missing[*]}"
    if [[ $YES == 0 ]]; then
        read -rp "Install them now with sudo pacman? [Y/n] " a
        [[ "${a,,}" == n* ]] && { echo "Skipped. Install them yourself, then run this again."; exit 1; }
    fi
    sudo pacman -S --needed --noconfirm "${missing[@]}" || { echo "Package install failed: see above, then run this again."; exit 1; }
fi

say "Building the glass shaders…"
"$ROOT/tools/build-shaders.sh" || { echo "The shaders didn't build (needs qt6-shadertools)."; exit 1; }
say "Building the Liquid Glass plugin for Hyprland $(pkg-config --modversion hyprland 2>/dev/null)…"
make -C "$ROOT/hypr/plugin/irice-glass" -s || echo "The glass plugin didn't build: glass falls back to frosted. Check the Hyprland headers (hyprland package), then run this again."
say "Building the native helpers…"
"$ROOT/shell/native/build.sh" >/dev/null || echo "Native helpers skipped: the shell uses its slower fallbacks."

cat <<EOF

iRice is ready. To start it:
  1. Switch to a free console: Ctrl+Alt+F3, and log in
  2. cd $ROOT && ./demo.sh --check && ./dev.sh tty
  3. Super+Shift+Escape ends it; Ctrl+Alt+F1 or F2 takes you back.

Updates: Settings → Software Update (it never changes your settings or
shortcuts without asking).
EOF
