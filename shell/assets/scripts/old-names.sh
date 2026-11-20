# Tessera's old name in the bash scripts: the one place that still knows it.
# Sourced, never run. Tessera was called iRice until 4 November 2026; an
# installed machine that updates across that rename needs these, once.
# REMOVE AFTER 2027-02-01 (with migrate-names.py, tessera-root.py's own
# section, core's old_names.rs and common/OldNames.qml: see
# docs/rename-to-tessera.md), and the lines that source it.
OLD_NAMES_REMOVE_AFTER=2027-02-01
OLD_NAME=irice
OLD_NAMES_DIR="$(dirname -- "${BASH_SOURCE[0]}")"

# Environment names: IRICE_<x> still counts as TESSERA_<x> (the new name
# wins when both are set), for a session or a habit from before
old_names_env() {
    local v n
    for v in $(compgen -e); do
        [[ $v == IRICE_* ]] || continue
        n="TESSERA_${v#IRICE_}"
        [[ -n "${!n+x}" ]] || export "$n=${!v}"
    done
}

# Is anything still to move (migrate-names.py)? Cheap, so the shell's start
# runs Python only when there is: an old-name folder or link in your XDG
# folders, the checkout at ~/iRice, or old notes.
old_names_pending() {
    local b root="${1:-}"
    for b in "${XDG_CONFIG_HOME:-$HOME/.config}" "${XDG_DATA_HOME:-$HOME/.local/share}" "${XDG_STATE_HOME:-$HOME/.local/state}" \
        "${XDG_CACHE_HOME:-$HOME/.cache}" "$HOME/.config" "$HOME/.local/share" "$HOME/.local/state" "$HOME/.cache"; do
        [[ -e "$b/irice" || -L "$b/irice" || -e "$b/tessera/MOVED-FROM-IRICE.txt" ]] && return 0
    done
    [[ -e "$HOME/iRice" || -L "$HOME/iRice" || -e "${XDG_CONFIG_HOME:-$HOME/.config}/hypr/irice" || -L "${XDG_CONFIG_HOME:-$HOME/.config}/hypr/irice" ]] && return 0
    [[ -z "$root" ]] || for b in "$root/.sandbox/home/.config" "$root/.sandbox/home/.local/share" "$root/.sandbox/home/.local/state"; do
        [[ -e "$b/irice" || -L "$b/irice" ]] && return 0
    done
    return 1
}

# Moves what's still under the old name (never fails the caller)
old_names_migrate() {
    old_names_pending "${1:-}" || return 0
    python3 -I "$OLD_NAMES_DIR/migrate-names.py" ${1:+--checkout "$1"} </dev/null >/dev/null 2>&1 || true
}

# The system pieces installed under the old name, until the root setup
# (tessera-root.py setup) or ./install.sh --only=login replaces them
OLD_ROOT_HELPER=/usr/lib/irice/irice-root
OLD_LOGIN_SHARE=/usr/share/irice
OLD_SESSION_ENTRY=/usr/share/wayland-sessions/irice.desktop
OLD_PLYMOUTH_DROPIN=/etc/systemd/system/plymouth-quit.service.d/irice.conf

# Everything an install from before the rename may have left under the old
# name, for the uninstaller: root's (under $1, the system root) …
old_names_system_paths() {
    local s="${1:-}"
    printf '%s\n' "$s/usr/lib/irice" "$s/etc/polkit-1/rules.d/49-irice.rules" "$s/etc/polkit-1/rules.d/49-irice-quickshare.rules" \
        "$s/usr/share/polkit-1/actions/org.irice.root.policy" "$s/etc/ananicy.d/irice.rules" "$s/etc/irice" "$s/var/lib/irice" \
        "$s/usr/share/irice" "$s$OLD_SESSION_ENTRY" "$s$OLD_PLYMOUTH_DROPIN" \
        "$s/etc/systemd/system/bluetooth.service.d/irice-no-rap.conf" "$s/etc/systemd/system/fprintd.service.d/irice-timeout.conf" \
        "$s/etc/modprobe.d/irice-phonecam.conf" "$s/etc/modules-load.d/irice-phonecam.conf"
}
# … and yours (config and data folders as $1 $2)
old_names_user_paths() {
    printf '%s\n' "$1/quickshell/irice" "$1/hypr/irice" "$1/fish/conf.d/irice.fish" "$1/fish/conf.d/irice-start-hyprland.fish" \
        "$HOME/.local/bin/irice" "$2/irice" "$2/tessera/irice-glass.so" "$2/tessera/irice-glass.so.new" "$2/tessera/irice-glass.built-for" \
        "$2/tessera/bin/irice-events" "$2/tessera/bin/irice-events.new"
}
# Backups the installer kept under the old name (.pre-irice), and the marks
# it put on lines it added (# irice: …)
OLD_BACKUP_SUFFIX=.pre-irice
OLD_LINE_MARK=irice
# (and the root helper's mark on a fingerprint line in a PAM file)
OLD_FP_MARK="# iRice fingerprint"

# The installer: root's backups (greetd, GRUB, PAM) take the new suffix, and
# the login screen's entry from before (irice.desktop, starting
# sandbox/irice-session.sh) becomes Tessera's, starting the same folder.
# $1: the system root (tests), $2: 1 = dry run (only says so)
old_names_install() {
    local s="${1:-}" dry="${2:-0}" p n
    for p in "$s"/etc/greetd/config.toml.pre-irice "$s"/etc/default/grub.pre-irice "$s"/etc/pam.d/*.pre-irice; do
        [[ -e $p ]] || continue
        n="${p%.pre-irice}.pre-tessera"
        [[ $dry == 1 ]] && { echo "would rename $p to $n"; continue; }
        [[ -e $n ]] || sudo mv "$p" "$n" || true
    done
    local old="$s$OLD_SESSION_ENTRY" sessions exec dir
    [[ -f $old ]] || return 0
    sessions=$(dirname "$old")
    exec=$(sed -n 's/^Exec=//p' "$old" | head -1)
    dir=$(cd "$(dirname "$exec")" 2>/dev/null && pwd -P) || return 0
    if [[ "$exec" == */sandbox/irice-session.sh && -x "$dir/tessera-session.sh" ]]; then
        [[ $dry == 1 ]] && { echo "would replace $old with $sessions/tessera.desktop ($dir/tessera-session.sh)"; return 0; }
        printf '%s\n' '[Desktop Entry]' 'Name=Tessera' "Comment=Tessera, the macOS-style Hyprland desktop, from $(dirname "$dir")" \
            "Exec=$dir/tessera-session.sh" 'Type=Application' 'DesktopNames=Hyprland' |
            sudo tee "$sessions/tessera.desktop" >/dev/null && sudo rm -f "$old" && echo "The login screen's entry is \"Tessera\" now"
    fi
    return 0
}

# The installer: the shell copy installed under the old name goes ($1 the
# config folder, $2 1 = dry run)
old_names_quickshell() {
    local q="$1/quickshell/irice"
    [[ -e $q || -L $q ]] || return 0
    [[ ${2:-0} == 1 ]] && { echo "would remove $q"; return 0; }
    rm -rf "$q"
}

# The development sandbox's container, created as "irice" before the rename
# (podman rename irice tessera moves it; until then dev.sh uses it)
OLD_DEV_BOX=irice
# The container to use: "tessera", else the old one while only it exists
old_names_dev_box() {
    local have
    if command -v podman >/dev/null; then
        podman container exists tessera 2>/dev/null && { echo tessera; return; }
        podman container exists "$OLD_DEV_BOX" 2>/dev/null && { echo "$OLD_DEV_BOX"; return; }
    fi
    have=$(distrobox list --no-color 2>/dev/null | awk -F'|' 'NR > 1 { gsub(/ /, "", $2); print $2 }')
    if ! grep -qx tessera <<<"$have" && grep -qx "$OLD_DEV_BOX" <<<"$have"; then echo "$OLD_DEV_BOX"; else echo tessera; fi
}
