#!/usr/bin/env bash
# iRice installer for Arch Linux. Installs everything iRice needs, builds it,
# sets it up and checks the result. Safe to run again (it updates).
#
#   ./install.sh               ask before each choice (sensible defaults)
#   ./install.sh --yes         no questions: recommended choices
#   ./install.sh --dev         link the shell to this folder instead of copying it
#   ./install.sh --no-deps     skip installing packages
#   ./install.sh --no-greeter  don't set up the iRice login screen
#   ./install.sh --no-extras   skip the optional extras (apps, AUR themes, wallpapers)
#   ./install.sh --verify      only check an existing install (same as: irice doctor)
#   ./install.sh --dry-run     show what would happen, change nothing
#
# Your Hyprland config is backed up once (~/.config/hypr.pre-irice) and
# `irice uninstall` puts it back. Other Quickshell configs are left alone.
# Everything is logged to ~/.cache/irice/install.log.
set -Euo pipefail
# Some builds (the MacTahoe GTK theme) fail without a terminal type, which
# serial consoles and non-interactive runs leave unset
export TERM="${TERM:-xterm-256color}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
BIN="$HOME/.local/bin"
LOG="$CACHE/irice/install.log"

YES=0 DEV=0 DEPS=1 DRY=0 GREETER=ask EXTRAS=1 VERIFY_ONLY=0
for a in "$@"; do
    case "$a" in
        --yes|-y) YES=1 ;;
        --dev|--link) DEV=1 ;;
        --no-deps) DEPS=0 ;;
        --dry-run) DRY=1 ;;
        --greeter) GREETER=yes ;;
        --no-greeter) GREETER=no ;;
        --no-extras) EXTRAS=0 ;;
        --verify) VERIFY_ONLY=1 ;;
        -h|--help) sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $a (see --help)"; exit 1 ;;
    esac
done

# ── Output ──────────────────────────────────────────────────────────────
mkdir -p "$(dirname "$LOG")"
if [[ $VERIFY_ONLY == 0 && $DRY == 0 ]]; then
    exec > >(tee -a "$LOG") 2>&1
    echo "=== $(date '+%F %T') iRice install from $ROOT ($*)"
fi
if [[ -t 1 ]]; then
    B=$'\033[1m' DIM=$'\033[2m' RED=$'\033[1;31m' GRN=$'\033[1;32m' YEL=$'\033[1;33m' BLU=$'\033[1;34m' R=$'\033[0m'
else
    B="" DIM="" RED="" GRN="" YEL="" BLU="" R=""
fi
say() { printf '%s\n' "$*"; }
note() { printf '  %s%s%s\n' "$DIM" "$*" "$R"; }
warn() { printf '  %s!%s %s\n' "$YEL" "$R" "$*"; }
die() { printf '\n%s✗ %s%s\n' "$RED" "$*" "$R"; exit 1; }
run() {
    if [[ $DRY == 1 ]]; then printf '  %swould run:%s %s\n' "$DIM" "$R" "$*"; else "$@"; fi
}
ask() { # question, default (y|n) -> 0 for yes
    local def="${2:-y}" reply
    [[ $YES == 1 || $DRY == 1 ]] && [[ $def == y ]] && return 0
    [[ $YES == 1 || $DRY == 1 ]] && return 1
    read -r -p "  $1 [$([[ $def == y ]] && echo Y/n || echo y/N)] " reply </dev/tty || reply=""
    [[ -z "$reply" ]] && reply=$def
    [[ "$reply" == [yY]* ]]
}

STEP=0 TOTAL=9
# step "Title" function [optional]: runs it; on failure, offer Retry / Skip / Abort
step() {
    local title="$1" fn="$2" optional="${3:-}"
    STEP=$((STEP + 1))
    printf '\n%s[%d/%d]%s %s%s%s\n' "$BLU" "$STEP" "$TOTAL" "$R" "$B" "$title" "$R"
    while true; do
        if "$fn"; then
            return 0
        fi
        printf '  %s✗ %s didn'"'"'t finish.%s Details are in %s\n' "$RED" "$title" "$R" "$LOG"
        if [[ $YES == 1 ]]; then
            [[ -n "$optional" ]] && { warn "Skipped (optional)."; return 0; }
            die "Stopped. Fix the problem above and run the installer again; finished steps are quick the second time."
        fi
        local reply
        read -r -p "  [R]etry, [s]kip$([[ -z "$optional" ]] && echo " (iRice may not work)"), or [a]bort? " reply </dev/tty || reply=a
        case "$reply" in
            [sS]*) warn "Skipped."; return 0 ;;
            [aA]*) die "Stopped. Run the installer again when ready; finished steps are quick the second time." ;;
        esac
    done
}

# ── What's needed ───────────────────────────────────────────────────────
PACKAGES=(
    # Compositor, shell, portals
    hyprland hypridle hyprpicker hyprsunset quickshell xdg-desktop-portal-hyprland xdg-desktop-portal-gtk
    # Building the glass plugin and shaders
    base-devel git pkgconf qt6-shadertools
    qt6-declarative qt6-svg qt6-imageformats qt6-multimedia qt6-multimedia-ffmpeg ffmpeg qt6-5compat qt6-wayland
    # Sound, network, Bluetooth, power, keyring, polkit
    pipewire pipewire-pulse pipewire-alsa wireplumber networkmanager bluez bluez-utils upower power-profiles-daemon
    gnome-keyring libsecret polkit brightnessctl playerctl libnotify mesa mesa-utils xdg-user-dirs
    # Screen tools, clipboard, input
    grim slurp wl-clipboard cliphist wf-recorder tesseract tesseract-data-eng wtype swappy imagemagick hyprpicker
    # Title bars (hyprbars, built with hyprpm when chosen in Settings)
    hyprpm
    # Spotlight, Quick Look, sharing, widgets, privacy dots, updates
    libqalculate ripgrep plocate poppler ffmpegthumbnailer file psmisc python jq curl translate-shell pacman-contrib matugen
    # Fonts and icons
    inter-font ttf-jetbrains-mono noto-fonts noto-fonts-emoji papirus-icon-theme
)
APPS=(kitty nautilus gvfs firefox mpv loupe pavucontrol)     # if you have none: terminal, files (the most Finder-like), browser, player, viewer, volume mixer
AUR=(mactahoe-icon-theme-git apple_cursor apple_hyprcursor mactahoe-gtk-theme bibata-cursor-theme-bin)
NEED_HYPRLAND=0.56   # the Lua config
NEED_QUICKSHELL=0.2

version_ge() { [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" == "$2" ]]; }
# Hyprland's own commands need a runtime directory (missing over some SSH
# logins and in containers): give them a private one when there isn't one
hypr() {
    if [[ -n "${XDG_RUNTIME_DIR:-}" ]]; then
        Hyprland "$@"
    else
        local rt
        rt=$(mktemp -d)
        XDG_RUNTIME_DIR="$rt" Hyprland "$@"
        rm -rf "$rt"
    fi
}
# From the package when there is one (no need to run Hyprland at all)
hypr_version() {
    local v
    v=$(pacman -Q hyprland 2>/dev/null | awk '{print $2}' | sed 's/-[0-9.]*$//; s/^[0-9]*://; s/\.r[0-9].*//')
    [[ -n "$v" ]] || v=$(hypr --version 2>/dev/null | sed -n 's/^Hyprland \([0-9.]*\).*/\1/p' | head -1)
    printf '%s\n' "$v"
}
qs_version() { qs --version 2>/dev/null | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+' | head -1; }

# ── Steps ───────────────────────────────────────────────────────────────
VIRT=none
preflight() {
    command -v pacman >/dev/null || { say "  iRice installs on Arch Linux (pacman)."; return 1; }
    [[ $EUID == 0 ]] && { say "  Run this as your normal user, not root; it asks for sudo when needed."; return 1; }
    command -v sudo >/dev/null || { say "  sudo is needed. As root: pacman -S sudo, then add your user to wheel (EDITOR=nano visudo)."; return 1; }
    if [[ $DRY == 0 ]]; then
        sudo -v || { say "  Your user can't use sudo yet (add it to the wheel group)."; return 1; }
        # Keep sudo from timing out during long builds
        (while kill -0 $$ 2>/dev/null; do sudo -n true 2>/dev/null; sleep 50; done) &
    fi
    if [[ $DEPS == 1 ]] && ! curl -fsS --max-time 10 -o /dev/null https://archlinux.org; then
        say "  No internet connection. Connect first (e.g. nmtui or iwctl), then run this again."
        return 1
    fi
    local free
    free=$(df -Pm "$HOME" | awk 'NR==2 {print $4}')
    (( free > 3000 )) || warn "Only ${free} MB free; the install needs about 3 GB."
    if [[ -f /var/lib/pacman/db.lck ]] && ! pgrep -x pacman >/dev/null; then
        say "  pacman's lock is left over from an interrupted run. Remove it with: sudo rm /var/lib/pacman/db.lck"
        return 1
    fi
    VIRT=$(systemd-detect-virt --vm 2>/dev/null || echo none)
    note "System: $(. /etc/os-release && echo "$PRETTY_NAME"), $(uname -m)$([[ $VIRT != none ]] && echo ", virtual machine ($VIRT)")"
    if ! compgen -G "/dev/dri/renderD*" >/dev/null; then
        warn "No GPU render device found. Hyprland needs one; in a VM, turn on 3D acceleration"
        warn "(virt-manager: Video Virtio + 3D acceleration and OpenGL display; VirtualBox: VMSVGA + 3D)."
    fi
    return 0
}

pkg_installed() { pacman -Qq "$1" >/dev/null 2>&1; }
install_packages() {
    [[ $DEPS == 0 ]] && { note "Skipped (--no-deps)."; return 0; }
    local want=("${PACKAGES[@]}")
    # Things that would conflict with what iRice uses
    if pkg_installed pulseaudio; then
        warn "PulseAudio is installed; iRice uses PipeWire, which replaces it."
        ask "Replace PulseAudio with PipeWire?" y && run sudo pacman -Rdd --noconfirm pulseaudio pulseaudio-bluetooth 2>/dev/null
    fi
    if pkg_installed tlp || pkg_installed tuned-ppd; then
        note "Keeping your power manager (tlp/tuned); power-profiles-daemon isn't installed."
        want=("${want[@]/power-profiles-daemon/}")
    fi
    # Some repositories don't carry every package; leave those out with a note
    local avail=() p
    for p in "${want[@]}"; do
        [[ -z "$p" ]] && continue
        if pacman -Si "$p" >/dev/null 2>&1 || pacman -Qq "$p" >/dev/null 2>&1; then avail+=("$p"); else warn "$p isn't in your repositories (left out)"; fi
    done
    # Guest tools so the VM resizes and shares the clipboard with its host
    case "$VIRT" in
        kvm|qemu) avail+=(qemu-guest-agent spice-vdagent) ;;
        oracle) avail+=(virtualbox-guest-utils) ;;
        vmware) avail+=(open-vm-tools) ;;
    esac
    # A terminal, file manager, browser, player and viewer if there aren't any
    if [[ $EXTRAS == 1 ]]; then
        local have_term have_files have_browser
        have_term=$(command -v foot kitty alacritty wezterm konsole gnome-terminal ghostty 2>/dev/null | head -1)
        have_files=$(command -v thunar nautilus dolphin nemo pcmanfm 2>/dev/null | head -1)
        have_browser=$(command -v firefox chromium google-chrome-stable brave zen-browser librewolf 2>/dev/null | head -1)
        if [[ -z "$have_term" || -z "$have_files" || -z "$have_browser" ]] && ask "Install basic apps (foot terminal, Files, Firefox, mpv, image viewer)?" y; then
            avail+=("${APPS[@]}")
        fi
    fi
    say "  Updating the system and installing ${#avail[@]} packages (already installed ones are kept)…"
    note "Arch needs the system up to date for new packages (pacman -Syu)."
    run sudo pacman -Syu --needed --noconfirm "${avail[@]}" || return 1
    [[ $DRY == 1 ]] && return 0
    local hv qv
    hv=$(hypr_version) qv=$(qs_version)
    version_ge "${hv:-0}" "$NEED_HYPRLAND" || { say "  Hyprland ${hv:-isn't installed}; iRice needs $NEED_HYPRLAND or newer."; return 1; }
    version_ge "${qv:-0}" "$NEED_QUICKSHELL" || { say "  Quickshell ${qv:-isn't installed}; iRice needs $NEED_QUICKSHELL or newer."; return 1; }
    note "Hyprland $hv, Quickshell $qv"
}

# yay, the AUR helper: always set up (iRice's extras come from the AUR, and
# `irice update` uses it to keep them current)
install_yay() {
    [[ $DEPS == 0 ]] && { note "Skipped."; return 0; }
    if command -v yay >/dev/null; then
        note "yay is installed."
        return 0
    fi
    say "  Installing yay (an AUR helper)…"
    local tmp
    tmp=$(mktemp -d)
    run git clone --depth 1 https://aur.archlinux.org/yay-bin.git "$tmp/yay-bin" || return 1
    (cd "$tmp/yay-bin" && run makepkg -si --noconfirm) || return 1
    rm -rf "$tmp"
    command -v yay >/dev/null || [[ $DRY == 1 ]]
}

aur_extras() {
    [[ $EXTRAS == 0 || $DEPS == 0 ]] && { note "Skipped."; return 0; }
    local missing=() p
    for p in "${AUR[@]}"; do pkg_installed "$p" || missing+=("$p"); done
    [[ ${#missing[@]} == 0 ]] && { note "Tahoe icons, cursors and GTK theme are installed."; return 0; }
    ask "Install the Tahoe-like icons, macOS and Bibata cursors and GTK theme (from the AUR)?" y || { note "Skipped."; return 0; }
    local helper
    helper=$(command -v yay || command -v paru || true)
    [[ -n "$helper" || $DRY == 1 ]] || { say "  yay isn't installed (the step before installs it)."; return 1; }
    # The GTK theme's own script needs sassc, which its package doesn't list
    run sudo pacman -S --needed --noconfirm sassc || return 1
    run "${helper:-yay}" -S --needed --noconfirm --answerdiff None --answerclean None "${missing[@]}"
}

build() {
    # Shaders (compiled with Qt's qsb) and the Liquid Glass plugin, built
    # against the installed Hyprland. The plugin source is kept so it can be
    # rebuilt after Hyprland updates (irice rebuild, or Settings).
    say "  Compiling shaders…"
    run "$ROOT/tools/build-shaders.sh" || return 1
    say "  Building the Liquid Glass plugin for Hyprland $(hypr_version)…"
    run mkdir -p "$DATA/irice/src/tools" "$DATA/irice/src/hypr"
    run rm -rf "$DATA/irice/src/hypr/plugin"
    run cp -r "$ROOT/hypr/plugin" "$DATA/irice/src/hypr/plugin"
    run cp "$ROOT/tools/build-plugin.sh" "$DATA/irice/src/tools/"
    run make -C "$DATA/irice/src/hypr/plugin/irice-glass" clean >/dev/null 2>&1
    run "$DATA/irice/src/tools/build-plugin.sh" || return 1
    [[ $DRY == 1 ]] || hypr_version >"$DATA/irice/irice-glass.built-for"
}

# ~/.local/bin on your PATH (for the irice command), in bash, zsh and fish
add_to_path() {
    [[ $DRY == 1 ]] && { note "would add ~/.local/bin to your PATH"; return 0; }
    local rc
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
        if [[ -f "$rc" ]] && ! grep -q 'irice: local bin' "$rc"; then
            printf '\nexport PATH="$HOME/.local/bin:$PATH" # irice: local bin\n' >>"$rc"
        fi
    done
    if [[ -d "$CONFIG/fish" ]]; then
        mkdir -p "$CONFIG/fish/conf.d"
        printf 'fish_add_path -g $HOME/.local/bin\n' >"$CONFIG/fish/conf.d/irice.fish"
    fi
}

install_files() {
    # The shell
    run mkdir -p "$CONFIG/quickshell" "$BIN"
    if [[ -e "$CONFIG/quickshell/irice" || -L "$CONFIG/quickshell/irice" ]]; then
        run rm -rf "$CONFIG/quickshell/irice"
    fi
    if [[ $DEV == 1 ]]; then
        run ln -s "$ROOT/shell" "$CONFIG/quickshell/irice"
    else
        run cp -r "$ROOT/shell" "$CONFIG/quickshell/irice"
    fi
    # Hyprland config: yours is backed up once, before iRice first replaces it
    if [[ -d "$CONFIG/hypr" && ! -d "$CONFIG/hypr/irice" && ! -e "$CONFIG/hypr.pre-irice" ]]; then
        run cp -a "$CONFIG/hypr" "$CONFIG/hypr.pre-irice"
        note "Your Hyprland config is backed up to $CONFIG/hypr.pre-irice"
    fi
    run mkdir -p "$CONFIG/hypr/irice"
    run cp "$ROOT/hypr/hyprland.lua" "$CONFIG/hypr/hyprland.lua"
    run cp "$ROOT/hypr/irice/"*.lua "$CONFIG/hypr/irice/"
    run cp "$ROOT/hypr/hypridle.conf" "$CONFIG/hypr/hypridle.conf"
    # A hyprland.conf next to hyprland.lua would be ambiguous; move it aside
    if [[ -f "$CONFIG/hypr/hyprland.conf" ]]; then
        run mv "$CONFIG/hypr/hyprland.conf" "$CONFIG/hypr/hyprland.conf.pre-irice"
    fi
    # Virtual machines: software cursor (hardware cursors often don't show)
    if [[ $VIRT != none ]]; then
        [[ $DRY == 1 ]] || printf -- '-- Written by the iRice installer: this is a virtual machine (%s)\nhl.config({ cursor = { no_hardware_cursors = true } })\n' "$VIRT" >"$CONFIG/hypr/irice/vm.lua"
    else
        run rm -f "$CONFIG/hypr/irice/vm.lua"
    fi
    # The irice command (doctor, update, rebuild, repair, uninstall) and where iRice came from
    run mkdir -p "$DATA/irice"
    [[ $DRY == 1 ]] || printf '%s\n' "$ROOT" >"$DATA/irice/source"
    run install -m 755 "$ROOT/tools/irice" "$BIN/irice"
    case ":$PATH:" in
        *":$BIN:"*) ;;
        *) add_to_path ;;
    esac
    # Your usual folders (Downloads, Pictures...), which iRice uses
    command -v xdg-user-dirs-update >/dev/null && run xdg-user-dirs-update
    run mkdir -p "$HOME/Pictures/Wallpapers" "$HOME/Pictures/Screenshots"
    return 0
}

extras() {
    [[ $EXTRAS == 0 ]] && { note "Skipped."; return 0; }
    local walls
    walls="$(xdg-user-dir PICTURES 2>/dev/null || echo "$HOME/Pictures")/Wallpapers/macOS"
    if [[ -f "$walls/Tahoe Light.jpg" ]]; then
        note "macOS wallpapers are in $walls"
    elif ask "Download macOS wallpapers (Tahoe and earlier, about 10 MB)?" y; then
        run "$ROOT/tools/get-wallpapers.sh" || warn "Some wallpapers didn't download; the bundled ones still work."
    fi
    # First install: start on the Tahoe pictures, light by day and dark at night
    if [[ ! -f "$CONFIG/irice/config.json" && -f "$walls/Tahoe Light.jpg" && $DRY == 0 ]]; then
        mkdir -p "$CONFIG/irice"
        python3 - "$CONFIG/irice/config.json" "$walls" <<'PY'
import json, sys
path, walls = sys.argv[1], sys.argv[2]
json.dump({"appearance": {"mode": "auto"},
           "wallpaper": {"dynamic": "appearance", "lightPath": walls + "/Tahoe Light.jpg", "darkPath": walls + "/Tahoe Dark.jpg"}},
          open(path, "w"), indent=4)
PY
        note "Starting with the Tahoe wallpaper (light by day, dark at night)"
    fi
    return 0
}

# Turn a service on (and start it, when systemd is running)
has_unit() { [[ -n "$(systemctl list-unit-files "$1" --no-legend 2>/dev/null)" ]]; }
svc_on() {
    has_unit "$1.service" || has_unit "$1" || return 0
    if [[ -d /run/systemd/system ]]; then run sudo systemctl enable --now "$1"; else run sudo systemctl enable "$1"; fi
}

services() {
    local s
    # Networking: NetworkManager, unless something else already runs the network
    if systemctl is-active --quiet systemd-networkd || systemctl is-active --quiet dhcpcd || systemctl is-active --quiet iwd; then
        if ! systemctl is-enabled --quiet NetworkManager 2>/dev/null; then
            warn "Your network is run by $(for s in systemd-networkd dhcpcd iwd; do systemctl is-active --quiet $s && echo $s && break; done)."
            if ask "Switch it to NetworkManager (iRice's Wi-Fi menus use it; takes effect after a reboot)?" y; then
                for s in systemd-networkd dhcpcd; do systemctl is-enabled --quiet $s 2>/dev/null && run sudo systemctl disable $s; done
                run sudo systemctl enable NetworkManager
            fi
        fi
    else
        svc_on NetworkManager || return 1
    fi
    svc_on bluetooth
    svc_on power-profiles-daemon
    case "$VIRT" in
        kvm|qemu) svc_on qemu-guest-agent ;;
        oracle) svc_on vboxservice ;;
        vmware) svc_on vmtoolsd ;;
    esac
    # File-name index for Spotlight's file search
    svc_on plocate-updatedb.timer
    # Sound runs per user (socket-activated)
    run systemctl --user enable pipewire.socket pipewire-pulse.socket wireplumber 2>/dev/null
    return 0
}

GREETER_DONE=0 OTHER_DM=""
login() {
    OTHER_DM=$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service 2>/dev/null || true)
    [[ "$OTHER_DM" == greetd ]] && OTHER_DM=""
    local want=$GREETER
    if [[ $want == ask ]]; then
        if [[ -n "$OTHER_DM" ]]; then
            ask "Use the iRice login screen instead of $OTHER_DM?" n && want=yes || want=no
        else
            ask "Set up the iRice login screen (recommended)?" y && want=yes || want=no
        fi
    fi
    if [[ $want == yes ]]; then
        if ! pkg_installed greetd; then run sudo pacman -S --needed --noconfirm greetd || return 1; fi
        # The greeter user can't read your home, so it gets its own copy
        local wall
        wall=$(python3 -c 'import json,sys; c=json.load(open(sys.argv[1])); w=c.get("wallpaper",{}); print(w.get("darkPath") or w.get("path") or "")' "$CONFIG/irice/config.json" 2>/dev/null || true)
        [[ -f "$wall" ]] || wall="$ROOT/shell/assets/wallpapers/tahoe-dusk.jpg"
        run sudo mkdir -p /usr/share/irice
        run sudo rm -rf /usr/share/irice/shell
        run sudo cp -r "$ROOT/shell" /usr/share/irice/shell
        run sudo cp "$ROOT/hypr/greeter.lua" /usr/share/irice/greeter.lua
        # In a virtual machine the login screen needs the software cursor too
        if [[ $VIRT != none && $DRY == 0 ]]; then
            printf '\n-- virtual machine (%s): software cursor\nhl.config({ cursor = { no_hardware_cursors = true } })\n' "$VIRT" | sudo tee -a /usr/share/irice/greeter.lua >/dev/null
        fi
        run sudo cp "$wall" /usr/share/irice/wallpaper.jpg
        run sudo chmod -R a+rX /usr/share/irice
        if [[ -f /etc/greetd/config.toml && ! -e /etc/greetd/config.toml.pre-irice ]]; then
            run sudo cp /etc/greetd/config.toml /etc/greetd/config.toml.pre-irice
        fi
        local conf='[terminal]
vt = 1

[default_session]
command = "start-hyprland -- --config /usr/share/irice/greeter.lua"
user = "greeter"'
        command -v start-hyprland >/dev/null || conf="${conf/start-hyprland -- /Hyprland }"
        if [[ $DRY == 1 ]]; then note "would write /etc/greetd/config.toml"; else printf '%s\n' "$conf" | sudo tee /etc/greetd/config.toml >/dev/null; fi
        if [[ -n "$OTHER_DM" ]]; then
            run sudo systemctl disable "$OTHER_DM"
            [[ $DRY == 1 ]] || printf '%s\n' "$OTHER_DM" >"$DATA/irice/previous-login-manager" # for irice uninstall
        fi
        run sudo systemctl enable greetd || return 1 # (started at the next boot)
        GREETER_DONE=1
    elif [[ -z "$OTHER_DM" ]]; then
        # No login screen: start Hyprland when you log in on the first console
        local profile="$HOME/.bash_profile"
        if ! grep -q 'irice: start Hyprland' "$profile" 2>/dev/null; then
            run sh -c "printf '%s\n' '' '# irice: start Hyprland when logging in on the first console' '[ -z \"\$WAYLAND_DISPLAY\" ] && [ \"\$XDG_VTNR\" = 1 ] && command -v start-hyprland >/dev/null && exec start-hyprland' >> '$profile'"
            note "Log in on the first console (tty1) and Hyprland starts."
        fi
    fi
    return 0
}

# ── Checks ──────────────────────────────────────────────────────────────
PASS=0 FAILS=0 WARNS=0
ok() { printf '  %s✓%s %s\n' "$GRN" "$R" "$*"; PASS=$((PASS + 1)); }
bad() { printf '  %s✗%s %s\n' "$RED" "$R" "$1"; [[ -n "${2:-}" ]] && printf '      %s→ %s%s\n' "$DIM" "$2" "$R"; FAILS=$((FAILS + 1)); }
meh() { printf '  %s!%s %s\n' "$YEL" "$R" "$1"; [[ -n "${2:-}" ]] && printf '      %s→ %s%s\n' "$DIM" "$2" "$R"; WARNS=$((WARNS + 1)); }

verify() {
    local hv qv c missing=()
    hv=$(hypr_version) qv=$(qs_version)
    version_ge "${hv:-0}" "$NEED_HYPRLAND" && ok "Hyprland $hv" || bad "Hyprland ${hv:-missing} (needs $NEED_HYPRLAND+)" "sudo pacman -Syu hyprland"
    version_ge "${qv:-0}" "$NEED_QUICKSHELL" && ok "Quickshell $qv" || bad "Quickshell ${qv:-missing} (needs $NEED_QUICKSHELL+)" "sudo pacman -S quickshell"
    for c in hyprctl hypridle wpctl nmcli bluetoothctl grim slurp wl-copy cliphist playerctl brightnessctl notify-send python3 curl jq; do
        command -v "$c" >/dev/null || missing+=("$c")
    done
    [[ ${#missing[@]} == 0 ]] && ok "Core tools" || bad "Missing: ${missing[*]}" "run the installer again (it installs them)"
    missing=()
    for c in qalc rg plocate pdftoppm swappy matugen tesseract wf-recorder magick; do command -v "$c" >/dev/null || missing+=("$c"); done
    [[ ${#missing[@]} == 0 ]] && ok "Feature tools (conversions, search, Quick Look, markup, app colours)" || meh "Some features are off; missing: ${missing[*]}"
    # Shell files, shaders
    if [[ -f "$CONFIG/quickshell/irice/shell.qml" ]]; then
        local frag qsb
        frag=$(find -L "$CONFIG/quickshell/irice" -name '*.frag' -o -name '*.vert' | wc -l)
        qsb=$(find -L "$CONFIG/quickshell/irice" -name '*.qsb' | wc -l)
        (( qsb >= frag && frag > 0 )) && ok "Shell installed, $qsb shaders compiled" || bad "Shaders aren't compiled ($qsb of $frag)" "irice rebuild"
    else
        bad "The shell isn't installed in $CONFIG/quickshell/irice" "run the installer again"
    fi
    # Glass plugin built for this Hyprland
    local so="$DATA/irice/irice-glass.so"
    if [[ -f "$so" ]] && [[ -n "$(nm -D "$so" 2>/dev/null | grep -E "pluginInit|PLUGIN_INIT")" ]]; then
        if [[ "$(cat "$DATA/irice/irice-glass.built-for" 2>/dev/null)" == "$hv" ]]; then
            ok "Liquid Glass plugin built for Hyprland $hv"
        else
            meh "The glass plugin was built for another Hyprland version" "irice rebuild (until then glass looks frosted)"
        fi
    else
        meh "The Liquid Glass plugin isn't built (glass looks frosted)" "irice rebuild"
    fi
    # Hyprland config: Hyprland checks it itself
    if [[ -f "$CONFIG/hypr/hyprland.lua" ]]; then
        local out
        out=$(hypr --verify-config --config "$CONFIG/hypr/hyprland.lua" 2>&1 | grep -viE '^\s*$|welcome|hyprland ver|config ok|verifying|built from' || true)
        if echo "$out" | grep -qiE 'error|cannot|failed|traceback|unknown'; then
            bad "The Hyprland config has errors" "$(echo "$out" | grep -iE 'error|cannot|failed|unknown' | head -2 | tr '\n' ' ')"
        else
            ok "Hyprland config checks out"
        fi
    else
        bad "No Hyprland config in $CONFIG/hypr" "run the installer again"
    fi
    # Fonts, icons, services
    # (no grep -q in these pipelines: it stops reading early, and pipefail
    # would count the writer's broken pipe as a failure)
    [[ -n "$(fc-list : family 2>/dev/null | grep -i '^Inter')" ]] && ok "Inter font" || bad "The Inter font is missing" "sudo pacman -S inter-font"
    [[ -d /usr/share/icons/Papirus || -d /usr/share/icons/MacTahoe ]] && ok "Icon theme ($( [[ -d /usr/share/icons/MacTahoe ]] && echo MacTahoe || echo Papirus))" || meh "No full icon theme; some app icons may be missing" "sudo pacman -S papirus-icon-theme"
    systemctl is-enabled --quiet NetworkManager 2>/dev/null && ok "NetworkManager on" || meh "NetworkManager isn't on (Wi-Fi menus won't work)" "sudo systemctl enable --now NetworkManager"
    systemctl is-enabled --quiet bluetooth 2>/dev/null && ok "Bluetooth service on" || meh "Bluetooth service isn't on" "sudo systemctl enable --now bluetooth"
    # Login
    if systemctl is-enabled --quiet greetd 2>/dev/null; then
        if [[ -f /usr/share/irice/shell/greeter.qml && -f /usr/share/irice/greeter.lua ]] && id greeter >/dev/null 2>&1 && grep -q irice /etc/greetd/config.toml 2>/dev/null; then
            ok "iRice login screen (greetd)"
        else
            bad "greetd is on but the iRice login screen isn't set up" "run the installer again"
        fi
    elif [[ -n "$(basename "$(readlink /etc/systemd/system/display-manager.service 2>/dev/null)" .service 2>/dev/null)" ]]; then
        ok "Login screen: $(basename "$(readlink /etc/systemd/system/display-manager.service)" .service) (choose Hyprland there)"
    elif grep -q 'irice: start Hyprland' "$HOME/.bash_profile" 2>/dev/null; then
        ok "Hyprland starts when you log in on tty1"
    else
        meh "Nothing starts Hyprland at login" "start it with: start-hyprland"
    fi
    # Inside a running iRice session: the live parts
    if [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]] && command -v hyprctl >/dev/null; then
        hyprctl irice-glass ping 2>/dev/null | grep -q irice-glass && ok "Glass plugin loaded in Hyprland" || meh "The glass plugin isn't loaded in this session" "irice rebuild --load"
        if pgrep -af '(^|/)(qs|quickshell) ' | grep -q irice; then
            ok "The iRice shell is running"
        else
            bad "The iRice shell isn't running" "irice repair"
        fi
        local qlog
        qlog=$(timeout 5 qs log -c irice -t 400 2>/dev/null)
        if grep -q "Failed to load configuration" <<<"$qlog"; then
            bad "The shell reported an error" "$(grep -m1 -A1 'Failed to load' <<<"$qlog" | tail -1)"
        fi
    fi
    printf '\n  %s%d passed%s' "$GRN" "$PASS" "$R"
    (( WARNS > 0 )) && printf ', %s%d to look at%s' "$YEL" "$WARNS" "$R"
    (( FAILS > 0 )) && printf ', %s%d failed%s' "$RED" "$FAILS" "$R"
    printf '\n'
    (( FAILS == 0 ))
}

# ── Run ─────────────────────────────────────────────────────────────────
if [[ $VERIFY_ONLY == 1 ]]; then
    printf '%siRice check%s\n' "$B" "$R"
    verify
    exit $?
fi

cat <<EOF

${B}iRice${R}  macOS Tahoe for Hyprland

This sets up iRice on $(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME"):
  • installs Hyprland, Quickshell and everything iRice uses (pacman; updates the system)
  • builds the Liquid Glass plugin and shaders
  • installs the shell and its Hyprland config (yours is backed up first)
  • sets up the login screen, services and, if you like, apps, themes and wallpapers
  • checks that everything works
EOF
[[ $DRY == 1 ]] && say "${YEL}Dry run: nothing will change.${R}"
ask "Start?" y || { say "Nothing changed."; exit 0; }

step "Checking your system" preflight
step "Installing packages" install_packages
step "AUR helper (yay)" install_yay optional
step "Tahoe themes (optional)" aur_extras optional
step "Building iRice" build optional
step "Installing iRice" install_files
step "Wallpapers (optional)" extras optional
step "Starting services" services optional
step "Login" login optional
printf '\n%s[✓]%s %sChecking the result%s\n' "$BLU" "$R" "$B" "$R"
if [[ $DRY == 1 ]]; then
    note "Skipped in a dry run."
    exit 0
fi
verify
result=$?

printf '\n%s%s%s\n' "$B" "$([[ $result == 0 ]] && echo "iRice is ready." || echo "iRice is installed, with problems above.")" "$R"
cat <<EOF
  • Restart to begin$([[ $GREETER_DONE == 1 ]] && echo " at the iRice login screen" || ([[ -n "$OTHER_DM" ]] && echo ", and pick Hyprland at your login screen" || echo ": log in on tty1 and Hyprland starts")).
  • Super+/ lists the keyboard shortcuts; Super+, opens Settings.
  • The ${B}irice${R} command: irice doctor (check), irice update, irice rebuild, irice repair, irice uninstall.
  • Log: $LOG
EOF
if [[ $result == 0 ]] && ask "Restart now?" n; then
    sudo systemctl reboot
fi
exit $result
