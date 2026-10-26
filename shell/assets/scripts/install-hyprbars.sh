#!/usr/bin/env bash
# Builds and enables the official hyprbars plugin (macOS-style title bars) with
# hyprpm. Run by you, from a terminal: it compiles code from hyprwm's repository.
set -euo pipefail
if ! command -v hyprpm >/dev/null; then
    echo "hyprpm (Hyprland's plugin manager) isn't installed."
    echo "On Arch-based systems it's its own package (matching your Hyprland), plus the build tools:"
    echo "  sudo pacman -S --needed hyprpm cmake meson cpio pkgconf git gcc"
    read -rp "Press Enter to close" _
    exit 1
fi
echo "Building hyprbars for Hyprland $(hyprctl version -j | grep -o '"tag": *"[^"]*"' | cut -d'"' -f4)…"
hyprpm update
hyprpm add https://github.com/hyprwm/hyprland-plugins || true
hyprpm enable hyprbars
hyprpm reload
hyprctl reload
echo "Done. Choose Title bars in Settings → Appearance → Decorations."
read -rp "Press Enter to close" _
