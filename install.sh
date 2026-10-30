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
if [[ $VERIFY_ONLY == 0 && $DRY == 0 ]]; then
    mkdir -p "$(dirname "$LOG")"
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

STEP=0 TOTAL=10
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
source "$ROOT/tools/packages.sh"
# What this iRice is tested with (compat.json), read as text: python and jq
# may not be installed yet
compat() { # section key
    awk -v s="\"$1\"" -v k="\"$2\":" '
        index($0, s) && /\{/ { on = 1; next }
        on && /\}/ { on = 0 }
        on && index($0, k) { sub(/^[^:]*:[[:space:]]*"/, ""); sub(/".*/, ""); print; exit }' "$ROOT/compat.json" 2>/dev/null
}
NEED_HYPRLAND=$(compat hyprland min) HYPR_MAX=$(compat hyprland max)
NEED_HYPRLAND=${NEED_HYPRLAND:-0.56}   # the Lua config
QS_BUNDLED=$(compat quickshell bundled) NEED_QUICKSHELL=$(compat quickshell min)
NEED_QUICKSHELL=${NEED_QUICKSHELL:-${QS_BUNDLED:-0.3.2}}

version_ge() { [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" == "$2" ]]; }
# Whether a Hyprland version is no newer than the one iRice is tested with
# ("0.56.x": any 0.56 release)
hypr_tested() {
    local v="$1" max="$HYPR_MAX" n
    [[ -z "$v" || -z "$max" ]] && return 0
    if [[ $max == *.x ]]; then
        max=${max%.x}
        n=$(tr -cd . <<<"$max" | wc -c)
        v=$(cut -d. -f1-$((n + 1)) <<<"$v")
    fi
    version_ge "$max" "$v"
}
# A package's version in your repositories, from a fresh private copy of the
# package lists (as checkupdates does: the system's own lists stay as they
# are); from the system's lists when fakeroot isn't there to make one
repo_version() {
    local db v=""
    db=$(mktemp -d)
    ln -s /var/lib/pacman/local "$db/local"
    if command -v fakeroot >/dev/null && fakeroot -- pacman -Sy --dbpath "$db" --logfile /dev/null >/dev/null 2>&1; then
        v=$(pacman -Si --dbpath "$db" "$1" 2>/dev/null | awk '/^Version/ { print $3; exit }')
    fi
    [[ -n "$v" ]] || v=$(pacman -Si "$1" 2>/dev/null | awk '/^Version/ { print $3; exit }')
    rm -rf "$db"
    [[ -n "$v" ]] && printf '%s\n' "$v" | sed 's/^[0-9]*://; s/-[^-]*$//'
}
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
# iRice's own Quickshell when there is one (launch-shell.sh starts that one), else the system's
qs_version() {
    local qs=qs
    [[ -x "$DATA/irice/quickshell/bin/qs" && -d "$DATA/irice/quickshell/lib/qt6/qml/Quickshell" ]] && qs="$DATA/irice/quickshell/bin/qs"
    "$qs" --version 2>/dev/null | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+' | head -1
}
# Another power manager, which power-profiles-daemon would fight with (TLP's
# package even conflicts with it); tuned-ppd stands in for it
power_manager() {
    local p
    for p in tlp auto-cpufreq tuned-ppd tuned; do
        if pkg_installed "$p" || systemctl is-active --quiet "$p" 2>/dev/null; then
            printf '%s\n' "$p"
            return 0
        fi
    done
    return 1
}
# The file your login shell reads when you log in on a console: where the
# "start Hyprland on tty1" lines go
login_profile() {
    local sh
    sh=$(basename "$(getent passwd "$(id -un)" | cut -d: -f7)")
    case "$sh" in
        zsh) printf '%s\n' "${ZDOTDIR:-$HOME}/.zprofile" ;;
        fish) printf '%s\n' "$CONFIG/fish/conf.d/irice-start-hyprland.fish" ;;
        bash)
            # bash reads the first of these that exists
            local f
            for f in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
                [[ -f "$f" ]] && { printf '%s\n' "$f"; return; }
            done
            printf '%s\n' "$HOME/.bash_profile"
            ;;
        *) printf '%s\n' "$HOME/.profile" ;;
    esac
}
# A login's PAM file with your keyring unlocked by the login password: the
# gnome-keyring lines after its last auth and session lines, each under a
# marker (uninstall.sh takes them out), on stdout. Returns 1 when there's
# nothing to add: no such file, the keyring's lines are there already, or
# KWallet's are (it unlocks its own wallet)
KEYRING_MARK='# irice: unlock the keyring with the login password'
pam_with_keyring() { # pam file
    [[ -f "$1" ]] || return 1
    grep -qE 'pam_gnome_keyring\.so|pam_kwallet[0-9]*\.so' "$1" && return 1
    grep -qE '^-?auth[[:space:]]' "$1" && grep -qE '^-?session[[:space:]]' "$1" || return 1
    awk -v mark="$KEYRING_MARK" '
        { line[NR] = $0 }
        /^-?auth[[:space:]]/ { a = NR }
        /^-?session[[:space:]]/ { s = NR }
        END {
            for (i = 1; i <= NR; i++) {
                print line[i]
                if (i == a) { print mark; print "-auth      optional     pam_gnome_keyring.so" }
                if (i == s) { print mark; print "-session   optional     pam_gnome_keyring.so auto_start" }
            }
        }' "$1"
}
# The makers of the GPUs here (intel, amd, nvidia; one per line), from the
# kernel's display devices, NVIDIA's module, else lspci.
# IRICE_SYSTEM_ROOT (tests) is where /sys is looked for.
gpu_vendors() {
    local sys="${IRICE_SYSTEM_ROOT:-}" f
    {
        for f in "$sys"/sys/class/drm/card[0-9]*/device/vendor; do
            [[ -f "$f" ]] && cat "$f"
        done
        [[ -d "$sys/sys/module/nvidia_drm" ]] && echo 0x10de
        if command -v lspci >/dev/null; then
            lspci -nn 2>/dev/null | grep -E 'VGA|3D|Display' | grep -o '\[[0-9a-f]\{4\}:' | tr -d '[:' | sed 's/^/0x/'
        fi
    } | while read -r f; do
        case "$f" in 0x8086) echo intel ;; 0x1002) echo amd ;; 0x10de) echo nvidia ;; esac
    done | sort -u
}
# hypr/irice/gpu.lua for an NVIDIA GPU (what Hyprland's wiki asks for), on
# stdout; returns 1 without one. Only when NVIDIA is the only GPU: on hybrid
# graphics the other one draws the desktop, and forcing NVIDIA's VA-API and
# GLX there would break them, so the lines are left as suggestions. The
# software cursor only for drivers before 555 (no explicit sync: hardware
# cursors flicker or vanish); newer ones work with Hyprland's default.
gpu_lua() { # vendors (gpu_vendors' output)
    local sys="${IRICE_SYSTEM_ROOT:-}" ver major on=""
    grep -qx nvidia <<<"$1" || return 1
    ver=$(cat "$sys/sys/module/nvidia/version" 2>/dev/null || true)
    major=${ver%%.*}
    printf -- '-- Written by the iRice installer: an NVIDIA GPU (driver %s). Rewritten at each install;\n' "${ver:-not loaded}"
    printf -- '-- your own changes go in ~/.config/hypr/custom.lua.\n'
    if grep -qvx nvidia <<<"$1"; then
        printf -- '-- Another GPU draws the desktop too (hybrid graphics), so nothing is forced onto NVIDIA.\n'
        printf -- '-- If NVIDIA drives your screens, remove the "-- " from these:\n'
        on="-- "
    fi
    printf -- '%shl.env("LIBVA_DRIVER_NAME", "nvidia")\n' "$on"
    printf -- '%shl.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia")\n' "$on"
    printf -- '%shl.env("NVD_BACKEND", "direct") -- (VA-API through libva-nvidia-driver)\n' "$on"
    if [[ "$major" =~ ^[0-9]+$ ]] && (( major < 555 )); then
        printf -- '-- (a driver before 555: hardware cursors flicker or vanish)\nhl.config({ cursor = { no_hardware_cursors = true } })\n'
    fi
}
# The AUR helper there is (yay or paru), or nothing
aur_helper() { command -v yay || command -v paru || true; }

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
    # (systemd-detect-virt prints "none" itself and then exits non-zero)
    VIRT=$(systemd-detect-virt --vm 2>/dev/null || true)
    VIRT=${VIRT:-none}
    note "System: $(. /etc/os-release && echo "$PRETTY_NAME"), $(uname -m)$([[ $VIRT != none ]] && echo ", virtual machine ($VIRT)")"
    if ! compgen -G "/dev/dri/renderD*" >/dev/null; then
        warn "No GPU render device found. Hyprland needs one; in a VM, turn on 3D acceleration"
        warn "(virt-manager: Video Virtio + 3D acceleration and OpenGL display; VirtualBox: VMSVGA + 3D)."
    fi
    # A Hyprland newer than this iRice is tested with: the update in the next
    # step installs it. Holding it back alone (--ignore) would be a partial
    # upgrade, which Arch doesn't support, so it's go on or stop
    if [[ $DEPS == 1 ]]; then
        local repo_hv
        repo_hv=$(repo_version hyprland)
        if [[ -n "$repo_hv" ]] && ! hypr_tested "$repo_hv"; then
            warn "Your repositories have Hyprland $repo_hv; this iRice is tested with $HYPR_MAX."
            note "Installing updates the whole system, Hyprland included (Arch doesn't support holding one"
            note "package back). Most of iRice works; the Liquid Glass plugin or some settings may not until"
            note "an iRice update supports $repo_hv (irice update, or Settings → Software Update)."
            ask "Continue with Hyprland $repo_hv anyway?" y ||
                die "Stopped: nothing was changed. Run the installer again once iRice supports Hyprland $repo_hv."
        fi
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
        if ask "Replace PulseAudio with PipeWire?" y; then
            # (only the ones installed: pacman stops at a name it doesn't have)
            local pa=()
            mapfile -t pa < <(pacman -Qq pulseaudio pulseaudio-bluetooth 2>/dev/null)
            run sudo pacman -Rdd --noconfirm "${pa[@]}" ||
                { say "  PulseAudio wasn't removed (see above), and PipeWire can't be installed next to it."; return 1; }
        fi
    fi
    local pm
    if pm=$(power_manager); then
        note "Keeping your power manager ($pm); power-profiles-daemon isn't installed."
        want=("${want[@]/power-profiles-daemon/}")
    fi
    # The GPUs' video drivers (VA-API: video decoding, recording)
    local gpus
    gpus=$(gpu_vendors)
    grep -qx intel <<<"$gpus" && want+=("${GPU_INTEL[@]}")
    grep -qx nvidia <<<"$gpus" && want+=("${GPU_NVIDIA[@]}")
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
    hv=$(hypr_version)
    if ! version_ge "${hv:-0}" "$NEED_HYPRLAND"; then
        say "  Hyprland ${hv:-is missing}; iRice needs $NEED_HYPRLAND or newer."
        if [[ "$(. /etc/os-release 2>/dev/null && echo "${ID:-}")" == manjaro ]]; then
            say "  Manjaro's stable branch gets Arch's updates a few weeks later, so its Hyprland can be too old."
            say "  Wait for it to arrive there, or move to the unstable branch, which follows Arch closely:"
            say "    sudo pacman-mirrors --api --set-branch unstable && sudo pacman -Syyu"
        elif [[ -z "$hv" ]]; then
            say "  Your repositories don't have it (pacman -Si hyprland); iRice needs Arch's, or a repository that carries it."
        fi
        return 1
    fi
    hypr_tested "$hv" || warn "Hyprland $hv is newer than this iRice is tested with ($HYPR_MAX)."
    qv=$(qs_version)
    if ! version_ge "${qv:-0}" "$NEED_QUICKSHELL"; then
        if [[ -n "$qv" ]]; then
            say "  Your Quickshell is $qv; iRice needs $NEED_QUICKSHELL or newer."
        else
            say "  Quickshell isn't in your repositories."
        fi
        say "  Building iRice its own Quickshell instead (a few minutes; your system's stays as it is)…"
        build_quickshell || return 1
        qv=$(qs_version)
    fi
    note "Hyprland $hv, Quickshell $qv"
}

# iRice's own Quickshell (tools/build-quickshell.sh: a release, in
# ~/.local/share/irice/quickshell), with what it needs to build
build_quickshell() {
    local need=()
    mapfile -t need < <(pacman -T -- "${QUICKSHELL_BUILD[@]}" 2>/dev/null)
    if (( ${#need[@]} > 0 )); then
        run sudo pacman -S --needed --noconfirm -- "${need[@]}" || return 1
    fi
    run "$ROOT/tools/build-quickshell.sh" "v${QS_BUNDLED:-$NEED_QUICKSHELL}" || return 1
    version_ge "$(qs_version)" "$NEED_QUICKSHELL" || { say "  The build finished, but Quickshell still isn't $NEED_QUICKSHELL or newer."; return 1; }
}

# yay, the AUR helper: set up unless there's one (iRice's extras come from
# the AUR, and `irice update` uses yay or paru to keep them current). On
# Manjaro, whose own software app (pamac) builds from the AUR too, only when
# you say so
install_yay() {
    [[ $DEPS == 0 ]] && { note "Skipped."; return 0; }
    local have
    have=$(aur_helper)
    if [[ -n "$have" ]]; then
        note "$(basename "$have") is installed: iRice uses it for the AUR."
        return 0
    fi
    if command -v pamac >/dev/null && ! ask "Install yay too (iRice's AUR extras and irice update use it; pamac stays as it is)?" n; then
        note "Skipped: no yay. The AUR extras are left out (pamac build <name> installs one by hand)."
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
    local helper
    helper=$(aur_helper)
    if [[ -z "$helper" && $DRY == 0 ]] && command -v pamac >/dev/null; then
        note "Skipped: no yay or paru (with pamac: pamac build ${missing[*]})"
        return 0
    fi
    ask "Install the Tahoe-like icons, macOS and Bibata cursors and GTK theme (from the AUR)?" y || { note "Skipped."; return 0; }
    [[ -n "$helper" || $DRY == 1 ]] || { say "  yay isn't installed (the step before installs it)."; return 1; }
    # The GTK theme's own script needs sassc, which its package doesn't list
    run sudo pacman -S --needed --noconfirm sassc || return 1
    run "${helper:-yay}" -S --needed --noconfirm --answerdiff None --answerclean None "${missing[@]}"
}

# Optional emoji styles (default: no)
emoji_extras() {
    [[ $EXTRAS == 0 || $DEPS == 0 ]] && { note "Skipped."; return 0; }
    local missing=() p helper
    for p in "${EMOJI_AUR[@]}"; do pkg_installed "$p" || missing+=("$p"); done
    [[ ${#missing[@]} == 0 ]] && { note "Apple and Fluent emoji are installed."; return 0; }
    helper=$(aur_helper)
    if [[ -z "$helper" && $DRY == 0 ]] && command -v pamac >/dev/null; then
        note "Skipped: no yay or paru (with pamac: pamac build ${missing[*]})"
        return 0
    fi
    ask "Install the Apple and Fluent emoji styles (from the AUR; Noto emoji is the default)?" n || { note "Skipped (Settings can install them later)."; return 0; }
    [[ -n "$helper" || $DRY == 1 ]] || { say "  yay isn't installed (the step before installs it)."; return 1; }
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
    # Native helpers (glass kept in step from C++, the C request helper),
    # built inside the shell so they're copied with it. Optional: without
    # them the shell uses its QML and Python fall-backs.
    say "  Building the native helpers…"
    run "$ROOT/shell/native/build.sh" || note "Native helpers skipped: the shell uses its slower fall-backs (rebuild later with: irice rebuild)"
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
    # Preserve locally edited installed copies before replacing managed files.
    local installed_backup="$DATA/irice/backups/installed-$(date +%Y%m%d-%H%M%S-%N)"
    if [[ -e "$CONFIG/quickshell/irice" || -d "$CONFIG/hypr" ]]; then
        run mkdir -p -m 700 "$installed_backup" || return 1
        if [[ -e "$CONFIG/quickshell/irice" || -L "$CONFIG/quickshell/irice" ]]; then
            run cp -a "$CONFIG/quickshell/irice" "$installed_backup/shell" || return 1
        fi
        if [[ -d "$CONFIG/hypr" ]]; then
            run cp -a "$CONFIG/hypr" "$installed_backup/hypr" || return 1
        fi
        note "Installed configuration backup: $installed_backup"
        # Only the last five are kept (each holds the whole shell and hypr)
        local old
        while IFS= read -r old; do
            [[ -n "$old" && "$old" == "$DATA/irice/backups/installed-"* ]] && run rm -rf -- "$old"
        done < <(ls -1d "$DATA/irice/backups/installed-"* 2>/dev/null | sort | head -n -5)
    fi
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
    # Everything in hypr/irice (Lua modules and the json they read), but leave
    # files the installer or the user added there (vm.lua) alone
    local f
    for f in "$ROOT/hypr/irice/"*; do
        [[ -f "$f" ]] && run cp "$f" "$CONFIG/hypr/irice/"
    done
    # Idle timers come from Settings → Battery & Power (start-idle.sh); a
    # hypridle.conf here is yours and wins over them. Earlier installers copied
    # one in: take that out again when it's unchanged, so the settings work
    if [[ -f "$CONFIG/hypr/hypridle.conf" ]] && ! "$ROOT/shell/assets/scripts/own-hypridle.sh" "$CONFIG/hypr/hypridle.conf"; then
        run rm -f "$CONFIG/hypr/hypridle.conf"
    fi
    # Your keyboard layout, as the system has it (localectl), over the "us" default
    if [[ $DRY == 1 ]]; then
        note "would write your keyboard layout (localectl) to $CONFIG/hypr/irice/keyboard.lua"
    else
        note "$("$ROOT/tools/keyboard-lua.sh" "$CONFIG/hypr/irice/keyboard.lua")"
    fi
    # A hyprland.conf next to hyprland.lua would be ambiguous; move it aside
    if [[ -f "$CONFIG/hypr/hyprland.conf" ]]; then
        run mv "$CONFIG/hypr/hyprland.conf" "$CONFIG/hypr/hyprland.conf.pre-irice"
    fi
    # What it set (displays, keyboard, environment, programs) as suggestions
    # you can turn on, written once; nothing of it is applied
    if [[ -f "$CONFIG/hypr/hyprland.conf.pre-irice" && ! -f "$CONFIG/hypr/irice/from-your-old-config.lua" ]]; then
        if [[ $DRY == 1 ]]; then
            note "would write your old hyprland.conf's settings as suggestions to $CONFIG/hypr/irice/from-your-old-config.lua"
        else
            OLD_CONFIG=$(python3 -I "$ROOT/tools/old-hypr-config.py" "$CONFIG/hypr/hyprland.conf.pre-irice" \
                "$CONFIG/hypr/irice/from-your-old-config.lua" 2>/dev/null || true)
        fi
    fi
    # Virtual machines: software cursor (hardware cursors often don't show)
    if [[ $VIRT != none ]]; then
        [[ $DRY == 1 ]] || printf -- '-- Written by the iRice installer: this is a virtual machine (%s)\nhl.config({ cursor = { no_hardware_cursors = true } })\n' "$VIRT" >"$CONFIG/hypr/irice/vm.lua"
    else
        run rm -f "$CONFIG/hypr/irice/vm.lua"
    fi
    # NVIDIA: the environment Hyprland's wiki asks for (gpu.lua)
    local gpus lua
    gpus=$(gpu_vendors)
    if lua=$(gpu_lua "$gpus"); then
        if [[ $DRY == 1 ]]; then note "would write NVIDIA's settings to $CONFIG/hypr/irice/gpu.lua"; else printf '%s\n' "$lua" >"$CONFIG/hypr/irice/gpu.lua"; fi
        if [[ "$(cat /sys/module/nvidia_drm/parameters/modeset 2>/dev/null)" == N ]]; then
            warn "NVIDIA's kernel mode setting is off; Hyprland needs it: add nvidia_drm.modeset=1 to your kernel parameters"
        fi
    else
        run rm -f "$CONFIG/hypr/irice/gpu.lua"
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
    # "Send to a nearby device…" in the right-click menu of the file managers
    # found (only that entry is added; Settings → Sharing can remove it)
    if command -v python3 >/dev/null; then
        run python3 -I "$CONFIG/quickshell/irice/assets/scripts/file-manager-share.py" install >/dev/null ||
            warn "Couldn't add Send to a nearby device… to your file manager; Settings → Sharing can try again."
    fi
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
                # iwd stays and NetworkManager drives it (instead of its own
                # wpa_supplicant fighting iwd for the Wi-Fi): saved networks are kept
                if systemctl is-active --quiet iwd; then
                    if [[ $DRY == 1 ]]; then
                        note "would write /etc/NetworkManager/conf.d/wifi_backend.conf (wifi.backend=iwd)"
                    else
                        sudo mkdir -p /etc/NetworkManager/conf.d &&
                            printf '# Written by the iRice installer: NetworkManager uses iwd, which ran the Wi-Fi before\n[device]\nwifi.backend=iwd\n' |
                            sudo tee /etc/NetworkManager/conf.d/wifi_backend.conf >/dev/null
                    fi
                fi
                run sudo systemctl enable NetworkManager
            fi
        fi
    else
        svc_on NetworkManager || return 1
    fi
    svc_on bluetooth
    # (not next to another power manager: they'd fight over the same settings)
    local pm
    if pm=$(power_manager); then
        note "Energy modes are left to $pm."
    else
        svc_on power-profiles-daemon
    fi
    case "$VIRT" in
        kvm|qemu) svc_on qemu-guest-agent ;;
        oracle) svc_on vboxservice ;;
        vmware) svc_on vmtoolsd ;;
    esac
    # File-name index for Spotlight's file search
    svc_on plocate-updatedb.timer
    # Sound runs per user (socket-activated)
    run systemctl --user enable pipewire.socket pipewire-pulse.socket wireplumber 2>/dev/null
    root_helper
    return 0
}

# iRice's root helper (/usr/lib/irice/irice-root) and its polkit action and
# rule: the login screen's wallpaper, the fixes in System health and Sharing,
# and Quick Share's Wi-Fi links then run without a password; updates ask once,
# at the start. Software Update keeps it in step with this folder.
root_helper() {
    command -v pkexec >/dev/null || { warn "polkit isn't installed: iRice will ask for your password for each fix"; return 0; }
    run sudo /usr/bin/python3 -I "$ROOT/shell/assets/scripts/irice-root.py" setup "$ROOT/shell" || warn "iRice's root helper wasn't installed: Software Update installs it next time"
    return 0
}

# Your keyring (passwords for apps) unlocked by your login password, with the
# login screen's PAM file (greetd's, or the console's): once, backed up first
# (.pre-irice), only with gnome-keyring's PAM module installed
keyring_at_login() { # pam file
    [[ -f /usr/lib/security/pam_gnome_keyring.so ]] || return 0
    local new
    new=$(pam_with_keyring "$1") || return 0
    if [[ $DRY == 1 ]]; then
        note "would add unlocking the keyring at login to $1"
        return 0
    fi
    [[ -e "$1.pre-irice" ]] || sudo cp -a "$1" "$1.pre-irice" || return 0
    printf '%s\n' "$new" | sudo tee "$1" >/dev/null && note "Your keyring unlocks with your login password ($1)"
}

GREETER_DONE=0 OTHER_DM="" OLD_CONFIG=""
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
        keyring_at_login /etc/pam.d/greetd
        GREETER_DONE=1
    elif [[ -z "$OTHER_DM" ]]; then
        # No login screen: start Hyprland when you log in on the first console,
        # from the file your login shell reads (bash, zsh, fish or sh)
        local profile
        profile=$(login_profile)
        if ! grep -q 'irice: start Hyprland' "$profile" 2>/dev/null; then
            if [[ $DRY == 1 ]]; then
                note "would add starting Hyprland on tty1 to $profile"
            elif [[ $profile == *.fish ]]; then
                mkdir -p "$(dirname "$profile")"
                printf '%s\n' '# irice: start Hyprland when logging in on the first console' \
                    'if status is-login; and test -z "$WAYLAND_DISPLAY"; and test "$XDG_VTNR" = 1; and command -q start-hyprland' \
                    '    exec start-hyprland' 'end' >"$profile"
            else
                printf '%s\n' '' '# irice: start Hyprland when logging in on the first console' \
                    '[ -z "$WAYLAND_DISPLAY" ] && [ "$XDG_VTNR" = 1 ] && command -v start-hyprland >/dev/null && exec start-hyprland' >>"$profile"
            fi
            note "Log in on the first console (tty1) and Hyprland starts."
        fi
        keyring_at_login /etc/pam.d/login
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
    if [[ -n "$hv" ]] && ! hypr_tested "$hv"; then
        meh "Hyprland $hv is newer than this iRice is tested with ($HYPR_MAX)" "irice update brings support once it's ready; until then the glass or some settings may not work"
    fi
    version_ge "${qv:-0}" "$NEED_QUICKSHELL" && ok "Quickshell $qv" || bad "Quickshell ${qv:-missing} (needs $NEED_QUICKSHELL+)" "$ROOT/tools/build-quickshell.sh (iRice's own; or sudo pacman -Syu quickshell)"
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
    if [[ -f "$CONFIG/hypr/irice/from-your-old-config.lua" ]]; then
        note "Settings from your old hyprland.conf are in $CONFIG/hypr/irice/from-your-old-config.lua, turned off (remove \"-- \" to use one)"
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
    elif grep -q 'irice: start Hyprland' "$(login_profile)" 2>/dev/null; then
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
step "Emoji styles (optional)" emoji_extras optional
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
if [[ -n "$OLD_CONFIG" ]]; then
    printf '\n%s\n' "$OLD_CONFIG" | sed 's/^/  /'
fi

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
