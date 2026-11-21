#!/usr/bin/env bash
# Removes Tessera: every change it made to the system goes, and the Hyprland
# config backed up by install.sh comes back.
#   ./uninstall.sh [--yes] [--dry-run]
# Packages are left installed; your Tessera settings in ~/.config/tessera are kept.
# Tessera's big downloads and builds (its checkout, tessera-ui, its own
# Quickshell, the build caches) go after one more question (--yes: they go too).
# What an install from before Tessera's rename left under its old name goes
# too (shell/assets/scripts/old-names.sh, the one place here that knows it).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$HERE/shell/assets/scripts/old-names.sh"

CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
# (TESSERA_SYSTEM_ROOT: where /etc and /usr are, for the tests)
S="${TESSERA_SYSTEM_ROOT:-}"
YES=0 DRY=0
for a in "$@"; do
    case "$a" in
        --yes|-y) YES=1 ;;
        --dry-run) DRY=1 ;;
        -h|--help) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $a (see --help)"; exit 1 ;;
    esac
done
run() { if [[ $DRY == 1 ]]; then printf '  would run: %s\n' "$*"; else "$@"; fi; }
# An optional step: one that fails is said, and the uninstall goes on (a
# unit that was never installed, a login manager removed since: set -e must
# never stop it halfway)
opt() { run "$@" || echo "  (didn't work, carrying on: $*)"; return 0; }
# The same, quiet (things that are often not there)
quiet() {
    if [[ $DRY == 1 ]]; then printf '  would run: %s\n' "$*"; else "$@" >/dev/null 2>&1 || true; fi
    return 0
}

# The config from before Tessera (older installers made timestamped backups;
# the oldest of those is the one from before Tessera; or the one an install
# from before the rename kept)
BACKUP=""
if [[ -d "$CONFIG/hypr.pre-tessera" ]]; then
    BACKUP="$CONFIG/hypr.pre-tessera"
elif [[ -d "$CONFIG/hypr$OLD_BACKUP_SUFFIX" ]]; then
    BACKUP="$CONFIG/hypr$OLD_BACKUP_SUFFIX"
else
    BACKUP="$(ls -d "$CONFIG"/hypr.backup-* 2>/dev/null | sort | head -1 || true)"
fi
# Tessera's Hyprland config in place? (else: already uninstalled, nothing to swap)
HYPR_OURS=0
[[ -d "$CONFIG/hypr/tessera" || -d "$CONFIG/hypr/$OLD_NAME" ]] && HYPR_OURS=1
[[ $HYPR_OURS == 1 ]] || BACKUP=""

# The big parts: Tessera's checkout from setup.sh, tessera-ui and the other
# binaries, its own Quickshell and its build, the installer's backups of the
# shell, and the build caches (never a checkout of yours elsewhere)
BIG=()
for p in "$DATA/tessera/repo" "$DATA/tessera/bin" "$DATA/tessera/quickshell" "$DATA/tessera/build" "$DATA/tessera/backups" \
    "$CACHE/tessera/cargo" "$CACHE/tessera/stage-src"; do
    [[ -e "$p" || -L "$p" ]] && BIG+=("$p")
done
big_size() { du -shc "${BIG[@]}" 2>/dev/null | tail -1 | cut -f1; return 0; }

echo "This will remove $CONFIG/quickshell/tessera, the glass plugin, the tessera command, the Tessera login screen"
echo "and every system change Tessera made (its root helper, polkit rules, drop-ins, PAM lines, units)."
if [[ -n "$BACKUP" ]]; then
    echo "$CONFIG/hypr comes back from $BACKUP (the current one moves to hypr.tessera-removed)."
elif [[ $HYPR_OURS == 1 ]]; then
    echo "There's no Hyprland config from before Tessera: $CONFIG/hypr moves to hypr.tessera-removed, and Hyprland writes its default one when it next starts."
fi
if [[ $YES == 0 && $DRY == 0 ]]; then
    read -r -p "Continue? [y/N] " reply
    [[ "$reply" == [yY]* ]] || { echo "Nothing changed."; exit 0; }
fi
REMOVE_BIG=0
if (( ${#BIG[@]} )); then
    if [[ $YES == 1 ]]; then
        REMOVE_BIG=1
    elif [[ $DRY == 1 ]]; then
        echo "  would ask whether to delete Tessera's downloads and builds too: ${BIG[*]}"
    else
        echo "Tessera's downloads and builds ($(big_size)):"
        printf '  %s\n' "${BIG[@]}"
        read -r -p "Delete these too? [y/N] " reply
        [[ "$reply" == [yY]* ]] && REMOVE_BIG=1
    fi
fi

# ── Yours ───────────────────────────────────────────────────────────────
# The battery guard, a user service (battery-guard-install.sh): stopped and
# its unit removed, before the script it runs goes
GUARD="$HOME/.config/systemd/user/tessera-battery-guard.service"
if [[ -f "$GUARD" || -L "$GUARD" ]]; then
    quiet systemctl --user disable --now tessera-battery-guard.service
    opt rm -f "$GUARD"
    quiet systemctl --user daemon-reload
    quiet systemctl --user reset-failed tessera-battery-guard.service
fi
# Tessera's desktop look: your app fonts, Qt style, prompt and terminal as they were
# (desktop-look.py off; its state goes too, so an install sets it up again)
LOOK_PY="$HERE/shell/assets/scripts/desktop-look.py"
if [[ $DRY == 1 ]]; then
    echo "  would put your apps' fonts and terminal back as they were (desktop-look.py off)"
elif [[ -f "$LOOK_PY" ]] && command -v python3 >/dev/null; then
    python3 -I "$LOOK_PY" off >/dev/null 2>&1 && rm -f "${XDG_STATE_HOME:-$HOME/.local/state}/tessera/desktop-look.json" || true
fi
# GTK's colours and theme Tessera set (Match apps, light/dark): back to GTK's own
if [[ $DRY == 1 ]]; then
    echo "  would take Tessera's GTK colours and theme out (desktop-theme.py undo-gtk)"
elif [[ -f "$HERE/shell/assets/scripts/desktop-theme.py" ]] && command -v python3 >/dev/null; then
    python3 -I "$HERE/shell/assets/scripts/desktop-theme.py" undo-gtk >/dev/null 2>&1 || true
fi
# The file picker's portal preference Tessera wrote (yours is never touched)
if [[ $DRY == 1 ]]; then
    echo "  would remove Tessera's file picker preference (file-picker.py remove)"
elif [[ -f "$HERE/shell/assets/scripts/file-picker.py" ]] && command -v python3 >/dev/null; then
    python3 -I "$HERE/shell/assets/scripts/file-picker.py" remove >/dev/null 2>&1 || true
fi
# "Send to a nearby device…" in the file managers' menus (it runs a script
# of the shell copy removed below): out first
for f in "$CONFIG/quickshell/tessera/assets/scripts/file-manager-share.py" "$HERE/shell/assets/scripts/file-manager-share.py"; do
    if [[ -f "$f" ]] && command -v python3 >/dev/null; then
        quiet python3 -I "$f" remove
        break
    fi
done
run rm -rf "$CONFIG/quickshell/tessera" "$CONFIG/quickshell/$OLD_NAME"
if [[ -n "$BACKUP" ]]; then
    run rm -rf "$CONFIG/hypr.tessera-removed"
    run mv "$CONFIG/hypr" "$CONFIG/hypr.tessera-removed"
    run cp -a "$BACKUP" "$CONFIG/hypr"
elif [[ $HYPR_OURS == 1 ]]; then
    # (Tessera's config would run the scripts removed above, every 5 s:
    # aside, whole, with any change of yours in it)
    run rm -rf "$CONFIG/hypr.tessera-removed"
    run mv "$CONFIG/hypr" "$CONFIG/hypr.tessera-removed"
fi

# ── The system's ────────────────────────────────────────────────────────
# Tessera's root helper undoes its own changes while it's there (not in the
# tests); an older helper without one of these says so, and the steps
# below remove the files either way
helper=""
for h in "$S/usr/lib/tessera/tessera-root" "$S$OLD_ROOT_HELPER"; do
    [[ -x $h ]] && { helper=$h; break; }
done
LOGIN_DONE=0
PREV="$(cat "$DATA/tessera/previous-login-manager" 2>/dev/null || cat "$DATA/$OLD_NAME/previous-login-manager" 2>/dev/null || true)"
RECORD="$S/etc/tessera/login-switch.json"
LOGIN_HERE=0
[[ -d "$S/usr/share/tessera" || -d "$S$OLD_LOGIN_SHARE" ]] && LOGIN_HERE=1
if [[ -n $helper && -z $S ]]; then
    # The login screen: the one before back (greetd with another greeter
    # included: its config back, greetd kept), as Settings' switch does
    if [[ $LOGIN_HERE == 1 ]] && { [[ -f $RECORD ]] || [[ -n "$PREV" ]]; }; then
        if [[ -f $RECORD ]]; then args=(login-switch off); else args=(login-switch off "$PREV"); fi
        if [[ $DRY == 1 ]]; then
            run sudo "$helper" "${args[@]}"
        elif sudo "$helper" "${args[@]}" >/dev/null 2>&1; then
            LOGIN_DONE=1
            echo "The login screen from before Tessera is back."
        fi
    fi
    # Faces and fingers for administrator prompts and sudo, the phone
    # camera, the firewall's ports for sharing
    quiet sudo "$helper" face admin off
    for w in polkit sudo login; do quiet sudo "$helper" fingerprint off "$w"; done
    quiet sudo "$helper" phonecam off
    quiet sudo "$helper" quickshare-trust off
    quiet sudo "$helper" sharing-ports close
fi
# The Tessera login screen: switch back to the login manager it replaced
# (or none: then log in on a console and start your desktop yourself)
if [[ $LOGIN_HERE == 1 ]]; then
    # (the root helper's record, from a switch in Settings: "greetd" when
    # greetd ran another greeter before)
    [[ -n "$PREV" ]] || PREV="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("previous",""))' "$RECORD" 2>/dev/null || true)"
    # (and what brings it back by itself when Tessera's fails; an install
    # from before 19 Nov has none of these)
    [[ -f "$S/etc/systemd/system/tessera-login-check.timer" ]] && quiet sudo systemctl disable tessera-login-check.timer
    for p in "$S/etc/systemd/system/greetd.service.d/tessera-fallback.conf" "$S/etc/systemd/system/tessera-login-fallback.service" \
        "$S/etc/systemd/system/tessera-login-check.service" "$S/etc/systemd/system/tessera-login-check.timer" "$RECORD"; do
        [[ -e $p ]] && opt sudo rm -f "$p"
    done
    if [[ $LOGIN_DONE == 0 ]] && systemctl is-enabled --quiet greetd 2>/dev/null && grep -q -e tessera -e "$OLD_NAME" "$S/etc/greetd/config.toml" 2>/dev/null; then
        if [[ "$PREV" == greetd ]]; then
            # (greetd with another greeter before: it stays on, with its config back, below)
            echo "greetd stays on, with its config from before Tessera."
        else
            opt sudo systemctl disable greetd
            if [[ -n "$PREV" ]]; then
                if run sudo systemctl enable "$PREV"; then
                    echo "Your login screen ($PREV) is back."
                else
                    echo "  $PREV couldn't be turned back on (is it still installed?)."
                    # (greetd again, with its own config from before, rather than no login screen)
                    if [[ -f "$S/etc/greetd/config.toml.pre-tessera" || -f "$S/etc/greetd/config.toml$OLD_BACKUP_SUFFIX" ]]; then
                        opt sudo systemctl enable greetd
                        echo "  greetd stays on with its config from before Tessera; turn on your login screen with: sudo systemctl enable <name>"
                    else
                        echo "  Log in on a console, then: sudo systemctl enable <your login screen>"
                    fi
                fi
            fi
        fi
    fi
    opt sudo rm -rf "$S/usr/share/tessera" "$S$OLD_LOGIN_SHARE"
    for b in "$S/etc/greetd/config.toml.pre-tessera" "$S/etc/greetd/config.toml$OLD_BACKUP_SUFFIX"; do
        if [[ -f $b ]]; then
            opt sudo mv "$b" "$S/etc/greetd/config.toml"
            break
        fi
    done
fi
# The login screen's entry for a Tessera run from its folder (tools/setup-session.sh)
opt sudo rm -f "$S/usr/share/wayland-sessions/tessera.desktop" "$S$OLD_SESSION_ENTRY"
# The quiet start (install.sh's quiet_boot): Plymouth quits as before, and
# GRUB's kernel line is as it was
RELOAD=0
for d in "$S/etc/systemd/system/plymouth-quit.service.d/tessera.conf" "$S$OLD_PLYMOUTH_DROPIN"; do
    if [[ -f $d ]]; then
        opt sudo rm -f "$d"
        RELOAD=1
    fi
done
for b in "$S/etc/default/grub.pre-tessera" "$S/etc/default/grub$OLD_BACKUP_SUFFIX"; do
    if [[ -f $b ]]; then
        opt sudo mv "$b" "$S/etc/default/grub"
        if command -v grub-mkconfig >/dev/null && [[ -z $S ]]; then
            opt sudo grub-mkconfig -o /boot/grub/grub.cfg
        fi
        break
    fi
done
# Unlocking the keyring at login (install.sh's lines under its marker, in the
# login screen's or the console's PAM file), and the copies made before
for f in "$S/etc/pam.d/greetd" "$S/etc/pam.d/login"; do
    for mark in tessera "$OLD_LINE_MARK"; do
        if grep -q "# $mark: unlock the keyring" "$f" 2>/dev/null; then
            opt sudo sed -i "/# $mark: unlock the keyring/,+1d" "$f"
            opt sudo rm -f "$f.pre-tessera" "$f$OLD_BACKUP_SUFFIX"
        fi
    done
done
# A finger for sudo, administrator prompts and the login screen (the root
# helper's lines, each marked; the helper took them out above when it could)
for f in polkit-1 sudo greetd ly sddm lightdm lxdm; do
    for mark in "# Tessera fingerprint" "$OLD_FP_MARK"; do
        if grep -qF "$mark" "$S/etc/pam.d/$f" 2>/dev/null; then
            opt sudo sed -i "/$mark/d" "$S/etc/pam.d/$f"
        fi
    done
done
# The boot animation: the system's own again when it's one of Tessera's (the
# helper rebuilds the boot image, and puts it back as it was if that fails)
if [[ -n $helper ]] && grep -qE "^[[:space:]]*Theme[[:space:]]*=[[:space:]]*(tessera|$OLD_NAME)-" "$S/etc/plymouth/plymouthd.conf" 2>/dev/null; then
    run sudo "$helper" bootsplash system ||
        echo "  The boot animation stays Tessera's (it still works). To change it: sudo plymouth-set-default-theme -R <theme>"
fi
# Systemd drop-ins: faces for administrator prompts first (it shows polkit
# a file in /var/lib/tessera, removed below), Bluetooth's plugins, fprintd's
# timeout; the phone camera's module settings; Bluetooth's settings as they were
FACE_DROPIN="$S/etc/systemd/system/polkit-agent-helper@.service.d/50-tessera-face.conf"
for d in "$FACE_DROPIN" "$S/etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf" \
    "$S/etc/systemd/system/fprintd.service.d/tessera-timeout.conf"; do
    if [[ -e $d || -L $d ]]; then
        opt sudo rm -f "$d"
        quiet sudo rmdir "$(dirname "$d")"
        RELOAD=1
    fi
done
for p in /etc/modprobe.d/tessera-phonecam.conf /etc/modules-load.d/tessera-phonecam.conf; do
    [[ -e "$S$p" ]] && opt sudo rm -f "$S$p"
done
BT_RESTART=0
[[ $RELOAD == 1 ]] && BT_RESTART=1
if [[ -f "$S/etc/bluetooth/main.conf.pre-tessera" ]]; then
    opt sudo mv "$S/etc/bluetooth/main.conf.pre-tessera" "$S/etc/bluetooth/main.conf"
    BT_RESTART=1
fi
if [[ $RELOAD == 1 ]]; then
    opt sudo systemctl daemon-reload
fi
if [[ $BT_RESTART == 1 && -z $S ]]; then
    quiet sudo systemctl try-restart bluetooth.service
fi
# Tessera's root helper and its polkit actions and rules, its ananicy rule
# and what it keeps in /etc and /var/lib (under the old name too)
for p in /usr/lib/tessera /etc/polkit-1/rules.d/49-tessera.rules /etc/polkit-1/rules.d/49-tessera-quickshare.rules \
    /usr/share/polkit-1/actions/org.tessera.root.policy /etc/ananicy.d/tessera.rules /etc/tessera /var/lib/tessera; do
    if [[ -e "$S$p" || -L "$S$p" ]]; then
        opt sudo rm -rf "$S$p"
        [[ $p == /etc/ananicy.d/tessera.rules && -z $S ]] && quiet sudo systemctl try-restart ananicy-cpp.service
    fi
done
while IFS= read -r p; do
    [[ -e "$p" || -L "$p" ]] && opt sudo rm -rf "$p"
done < <(old_names_system_paths "$S")

# ── Yours again ─────────────────────────────────────────────────────────
# The glass plugin and its source (the running session keeps its loaded copy
# until you log out), and the installer's bookkeeping. Settings in ~/.config/tessera stay.
for d in "$DATA/tessera" "$DATA/$OLD_NAME"; do
    run rm -f "$d/tessera-glass.so" "$d/tessera-glass.so.new" "$d/tessera-glass.built-for"
    run rm -rf "$d/src"
    run rm -f "$d/bin/tessera-events" "$d/bin/tessera-events.new"
    run rm -f "$d/source" "$d/previous-login-manager"
    # (the Hyprland files as last shipped, the base updates merge your edits against)
    run rm -rf "$d/hypr-base" "$d/hypr-base.old"
done
run rm -f "${XDG_STATE_HOME:-$HOME/.local/state}/tessera/config-sync.json"
# The tessera command, and starting Hyprland from tty1
run rm -f "$HOME/.local/bin/tessera" \
    "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/tessera.fish" "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/tessera-start-hyprland.fish"
# (and what the old name left: its command, glass plugin, events helper…)
while IFS= read -r p; do
    [[ $HYPR_OURS == 1 && $p == "$CONFIG/hypr/$OLD_NAME" ]] && continue # (moved aside with hypr/ above)
    [[ $p == "$DATA/$OLD_NAME" ]] && continue
    [[ -e "$p" || -L "$p" ]] && run rm -rf "$p"
done < <(old_names_user_paths "$CONFIG" "$DATA")
for f in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile" "${ZDOTDIR:-$HOME}/.zprofile"; do
    for mark in tessera "$OLD_LINE_MARK"; do
        if grep -q "$mark: start Hyprland" "$f" 2>/dev/null; then
            run sed -i "/# $mark: start Hyprland/,+1d" "$f"
        fi
    done
done
# ~/.local/bin on your PATH, the line install.sh added (# tessera: local bin)
for f in "$HOME/.bashrc" "$HOME/.zshrc"; do
    for mark in tessera "$OLD_LINE_MARK"; do
        if grep -q "# $mark: local bin\$" "$f" 2>/dev/null; then
            run sed -i "/# $mark: local bin\$/d" "$f"
        fi
    done
done

# The big parts last (this script may be running from the checkout among them)
if [[ $REMOVE_BIG == 1 ]]; then
    for p in "${BIG[@]}"; do
        run rm -rf "$p"
    done
    echo "Tessera's downloads and builds are deleted."
elif (( ${#BIG[@]} )); then
    echo "Kept (delete them yourself when you like): ${BIG[*]}"
fi
echo "Done. Log out and back in to start your previous setup."
exit 0
