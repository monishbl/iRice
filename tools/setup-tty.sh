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

source "$ROOT/tools/packages.sh"   # the same list as install.sh
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

# The Tahoe icons, cursors and GTK theme (from the AUR, through yay): without
# them apps fall back to plain icons and the default GTK look
missing=()
for p in "${AUR[@]}"; do pacman -Qq "$p" >/dev/null 2>&1 || missing+=("$p"); done
if (( ${#missing[@]} > 0 )); then
    say "Tahoe icons, cursors and GTK theme (AUR): ${missing[*]}"
    a=y
    [[ $YES == 0 ]] && read -rp "Install them now with yay? [Y/n] " a
    if [[ "${a,,}" != n* ]]; then
        if ! command -v yay >/dev/null && ! command -v paru >/dev/null; then
            say "Installing yay (an AUR helper)…"
            tmp=$(mktemp -d)
            git clone --depth 1 https://aur.archlinux.org/yay-bin.git "$tmp/yay-bin" && (cd "$tmp/yay-bin" && makepkg -si --noconfirm)
            rm -rf "$tmp"
        fi
        helper=$(command -v yay || command -v paru || true)
        if [[ -n "$helper" ]]; then
            # The GTK theme's own script needs sassc, which its package doesn't list
            sudo pacman -S --needed --noconfirm sassc
            TERM=${TERM:-xterm} "$helper" -S --needed --noconfirm --answerdiff None --answerclean None "${missing[@]}" ||
                echo "Some themes didn't install: see above. iRice works without them; run this again to retry."
        else
            echo "yay didn't install: see above. iRice works without the themes; run this again to retry."
        fi
    fi
fi

# Headphone battery levels: most headphones report them over the hands-free
# channel, which BlueZ only passes on with its experimental features on
conf=/etc/bluetooth/main.conf
if [[ -f $conf ]] && ! grep -qE '^\s*Experimental\s*=\s*true' "$conf"; then
    say "Show Bluetooth headphone battery levels (turns on BlueZ's experimental features)?"
    a=y
    [[ $YES == 0 ]] && read -rp "Turn it on? [Y/n] " a
    if [[ "${a,,}" != n* ]]; then
        sudo sed -i -E 's/^\s*#?\s*Experimental\s*=.*/Experimental = true/' "$conf"
        grep -qE '^Experimental = true' "$conf" || printf '\n[General]\nExperimental = true\n' | sudo tee -a "$conf" >/dev/null
        sudo systemctl restart bluetooth && echo "Done: reconnect your headphones to see their battery."
    fi
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
