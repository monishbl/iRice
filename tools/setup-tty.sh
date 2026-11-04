#!/usr/bin/env bash
# Sets up Tessera to run from this folder on its own console (./dev.sh tty or
# ./demo.sh), without installing it as your desktop: your ~/.config and your
# usual session aren't touched; Tessera keeps its settings in .sandbox/home.
#
#   tools/setup-tty.sh [--yes]     packages (asks first), then builds
#
# Run again after a Hyprland or Qt update (it rebuilds what needs it).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
YES=0
[[ "${1:-}" == "--yes" ]] && YES=1
say() { printf '\033[1m%s\033[0m\n' "$*"; }

command -v pacman >/dev/null || { echo "Tessera needs Arch Linux (or an Arch-based system such as CachyOS or EndeavourOS)."; exit 1; }

source "$ROOT/tools/packages.sh"   # the same list as install.sh
missing=()
for p in "${PACKAGES[@]}"; do pacman -Qq "$p" >/dev/null 2>&1 || missing+=("$p"); done
if (( ${#missing[@]} > 0 )); then
    say "Packages Tessera needs: ${missing[*]}"
    if [[ $YES == 0 ]]; then
        read -rp "Install them now with sudo pacman? [Y/n] " a
        [[ "${a,,}" == n* ]] && { echo "Skipped. Install them yourself, then run this again."; exit 1; }
    fi
    sudo pacman -S --needed --noconfirm "${missing[@]}" || { echo "Package install failed: see above, then run this again."; exit 1; }
fi

# Your keyboard layout (localectl) for the test session (sandbox/hypr/tty.lua)
"$ROOT/tools/keyboard-lua.sh" "$ROOT/.sandbox/keyboard.lua"

# The Bibata pointer (from the AUR, through yay): without it apps fall back
# to the default pointer
missing=()
for p in "${AUR[@]}"; do pacman -Qq "$p" >/dev/null 2>&1 || missing+=("$p"); done
if (( ${#missing[@]} > 0 )); then
    say "The Bibata pointer (AUR): ${missing[*]}"
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
            TERM=${TERM:-xterm} "$helper" -S --needed --noconfirm --answerdiff None --answerclean None "${missing[@]}" ||
                echo "The pointer didn't install: see above. Tessera works without it; run this again to retry."
        else
            echo "yay didn't install: see above. Tessera works without the pointer; run this again to retry."
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
        # ...but not its Ranging and LE Audio control profiles: they make phones
        # ask to pair when they send with Quick Share (shell/assets/scripts/sharing-check.py)
        sudo mkdir -p /etc/systemd/system/bluetooth.service.d
        printf '# Tessera: no Ranging or LE Audio control profiles, which make phones ask to pair for Quick Share\n[Service]\nExecStart=\nExecStart=/usr/lib/bluetooth/bluetoothd --noplugin=rap,vcp,micp,mcp,bass\n' |
            sudo tee /etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf >/dev/null
        sudo rm -f /etc/systemd/system/bluetooth.service.d/irice-no-rap.conf # (its name before iRice became Tessera)
        sudo systemctl daemon-reload
        sudo systemctl restart bluetooth && echo "Done: reconnect your headphones to see their battery."
    fi
fi

# Tessera on the login screen (ly, SDDM, GDM…), next to your usual Hyprland
if [[ ! -f /usr/share/wayland-sessions/tessera.desktop ]] || ! grep -q "$ROOT/sandbox/tessera-session.sh" /usr/share/wayland-sessions/tessera.desktop; then
    say "Add \"Tessera\" to the login screen, so you can pick it instead of your usual desktop?"
    a=y
    [[ $YES == 0 ]] && read -rp "Add it? [Y/n] " a
    [[ "${a,,}" != n* ]] && "$ROOT/tools/setup-session.sh"
fi

say "Building the glass shaders…"
"$ROOT/tools/build-shaders.sh" || { echo "The shaders didn't build (needs qt6-shadertools)."; exit 1; }
say "Building the glass plugin for Hyprland $(pkg-config --modversion hyprland 2>/dev/null)…"
make -C "$ROOT/hypr/plugin/tessera-glass" -s || echo "The glass plugin didn't build: glass falls back to frosted. Check the Hyprland headers (hyprland package), then run this again."
say "Building the native helpers…"
"$ROOT/shell/native/build.sh" >/dev/null || echo "Native helpers skipped: the shell uses its slower fallbacks."

cat <<EOF

Tessera is ready. To start it:
  1. Switch to a free console: Ctrl+Alt+F3, and log in
  2. cd $ROOT && ./demo.sh --check && ./dev.sh tty
  3. Super+Shift+Escape ends it; Ctrl+Alt+F1 or F2 takes you back.

Updates: Settings → Software Update (it never changes your settings or
shortcuts without asking).
EOF
