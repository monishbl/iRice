#!/usr/bin/env bash
# Starts the iRice shell with the right icon theme, then becomes it:
#   launch-shell.sh -c irice        (installed)
#   launch-shell.sh -p <shell dir>  (from the repo)
# Quickshell only reads the icon theme at startup (QS_ICON_THEME); without
# one it falls back to bare "hicolor" and most app icons are missing. The
# theme is, in order: Settings → Appearance → Icon theme, KDE's, GNOME/GTK's,
# else the first good one installed.
set -uo pipefail
# Updater/watchdog restarts can inherit an older environment than Hyprland's
# reloaded config. Apps launched by this shell still need the startup palette.
# KDE's theme, or qt6ct when you've set it up and KDE's isn't installed or
# your environment.d asks for qt6ct (the same rule as hypr/irice/look.lua).
qt6ct_chosen() {
    local cfg="${XDG_CONFIG_HOME:-$HOME/.config}" plugins="${IRICE_SYSTEM_ROOT:-}/usr/lib/qt6/plugins/platformthemes"
    [[ -f "$cfg/qt6ct/qt6ct.conf" && -f "$plugins/libqt6ct.so" ]] || return 1
    [[ ! -f "$plugins/KDEPlasmaPlatformTheme6.so" ]] && return 0
    grep -lsqE '^[[:space:]]*QT_QPA_PLATFORMTHEME="?qt6ct' "$cfg"/environment.d/*.conf
}
if [[ -z "${QT_QPA_PLATFORMTHEME:-}" ]]; then
    if qt6ct_chosen; then export QT_QPA_PLATFORMTHEME=qt6ct; else export QT_QPA_PLATFORMTHEME=kde; fi
fi

themes_dirs=("${XDG_DATA_HOME:-$HOME/.local/share}/icons" "$HOME/.local/share/icons" "$HOME/.icons" /usr/local/share/icons /usr/share/icons) # (the tty session moves XDG_DATA_HOME; icons installed for you stay in ~/.local/share)
installed() {
    local d
    for d in "${themes_dirs[@]}"; do
        [[ -f "$d/$1/index.theme" ]] && grep -q "^Directories=" "$d/$1/index.theme" && return 0
    done
    return 1
}

pick=""
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/irice/config.json"
if [[ -f "$cfg" ]]; then
    pick="$(sed -n 's/.*"iconTheme"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$cfg" | head -1)"
fi
# Each app's own logo by default (iRice styles it: Clear in Glass, Themed in
# Material): Adwaita draws the folders and system icons but leaves apps to
# the icons they ship. An icon theme chosen in Settings wins.
if [[ -z "$pick" ]] && installed Adwaita; then
    pick=Adwaita
fi
if [[ -z "$pick" && -f "${XDG_CONFIG_HOME:-$HOME/.config}/kdeglobals" ]]; then
    pick="$(sed -n '/^\[Icons\]/,/^\[/{s/^Theme=//p}' "${XDG_CONFIG_HOME:-$HOME/.config}/kdeglobals" | head -1)"
fi
if [[ -z "$pick" ]] && command -v gsettings >/dev/null; then
    pick="$(gsettings get org.gnome.desktop.interface icon-theme 2>/dev/null | tr -d "'")"
fi
if [[ -z "$pick" ]]; then
    pick="$(sed -n 's/^gtk-icon-theme-name[[:space:]]*=[[:space:]]*//p' "${XDG_CONFIG_HOME:-$HOME/.config}"/gtk-[34].0/settings.ini 2>/dev/null | head -1 | tr -d '"')"
fi
if [[ -z "$pick" ]] || ! installed "$pick"; then
    pick=""
    for t in Papirus-Dark Papirus breeze-plus-dark breeze-dark breeze Adwaita; do
        installed "$t" && { pick="$t"; break; }
    done
fi
[[ -n "$pick" ]] && export QS_ICON_THEME="$pick"

# Live wallpapers: decode video on the GPU (VA-API) when there is one
if [[ -z "${QT_FFMPEG_DECODING_HW_DEVICE_TYPES:-}" ]] && compgen -G "/dev/dri/renderD*" >/dev/null; then
    export QT_FFMPEG_DECODING_HW_DEVICE_TYPES=vaapi,drm
fi
# iRice's own Quickshell (tools/build-quickshell.sh), when there is one:
# kept in step with the system's Qt even if the system's Quickshell is pinned
# by another setup. Its QML modules come first on the import path.
QS=qs
LOCAL=""
for d in "${XDG_DATA_HOME:-}" "$HOME/.local/share"; do
    [[ -n "$d" && -x "$d/irice/quickshell/bin/qs" && -d "$d/irice/quickshell/lib/qt6/qml/Quickshell" ]] && { LOCAL="$d/irice/quickshell"; break; }
done
if [[ -n "$LOCAL" ]]; then
    QS="$LOCAL/bin/qs"
    export QML_IMPORT_PATH="$LOCAL/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
    export QML2_IMPORT_PATH="$QML_IMPORT_PATH"
fi
# iRice's native helpers (shell/native/build.sh), when built for this Qt: the
# IRice.Native QML module (glass kept in step from C++). Without them the
# shell uses its QML fallback. IRICE_NO_NATIVE=1 forces the fallback.
NATIVE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/native/build"
if [[ "${IRICE_NO_NATIVE:-}" != 1 && -d "$NATIVE/qml/IRice/Native" ]]; then
    want=$(cat "$NATIVE/built-for-qt" 2>/dev/null || true)
    have=$( { /usr/lib/qt6/bin/qtpaths --qt-version || qmake6 -query QT_VERSION; } 2>/dev/null || true)
    if [[ -z "$want" || -z "$have" || "$want" == "$have" ]]; then
        export QML_IMPORT_PATH="$NATIVE/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
        export QML2_IMPORT_PATH="$QML_IMPORT_PATH"
    fi
fi
# Qt's SVG renderer can't size the tiny drop-shadow blur in a couple of icon
# theme files (MacTahoe's Google Maps and Android Studio), skips just that
# blur (it's invisible at icon sizes) and logs a warning each time one loads.
# Only that category is muted; every other warning still shows.
export QT_LOGGING_RULES="${QT_LOGGING_RULES:+$QT_LOGGING_RULES;}qt.svg.draw.warning=false"
# (setsid -f: on its own, not a child of the shell this becomes. Started with
# `&`, it was: when one already runs (a shell restart) it ends at once, and
# the shell, which doesn't wait for children it didn't start, kept it as a
# zombie for its whole life)
setsid -f "$(dirname -- "${BASH_SOURCE[0]}")/start-idle.sh" "$QS" </dev/null >/dev/null 2>&1

# The look (Settings → Appearance → Look). Glass is this shell folder.
# Material (experimental) is the material/ folder beside it, run as a root of
# links over this one (material-root.py): only its own files differ. Whatever
# the arguments name (-c irice, this folder, or the Material root, as a
# restart passes them on), the shell starts from the root of the look chosen;
# without the material/ folder (a copied install), Glass.
SELF_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd -P)"
MATERIAL_DIR="$(dirname "$SELF_DIR")/material"
MATERIAL_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/irice/material-shell"
look=$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1])).get("appearance", {}).get("look", "glass"))' \
    "${XDG_CONFIG_HOME:-$HOME/.config}/irice/config.json" 2>/dev/null || echo glass)
target=""
if [[ "$look" == material && -f "$MATERIAL_DIR/shell.qml" ]] &&
    python3 -I "$SELF_DIR/assets/scripts/material-root.py" "$SELF_DIR" "$MATERIAL_DIR" "$MATERIAL_ROOT" >/dev/null; then
    target="$MATERIAL_ROOT"
else
    look=glass
fi
args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        -c | --config)
            if [[ "${2:-}" == irice && -n "$target" ]]; then args+=(-p "$target"); else args+=("$1" "${2:-}"); fi
            shift 2 || shift
            ;;
        -p | --path)
            p="${2:-}"
            if [[ "$p" == "$MATERIAL_ROOT" || "$(readlink -f "$p" 2>/dev/null)" == "$SELF_DIR" ]]; then
                args+=(-p "${target:-$SELF_DIR}")
            else
                args+=("$1" "$p")
            fi
            shift 2 || shift
            ;;
        *)
            args+=("$1")
            shift
            ;;
    esac
done
# (how the session's scripts tell an iRice shell, whichever root it runs)
export IRICE_SHELL=1 IRICE_LOOK="$look"
# The Material look is experimental: if it doesn't come up (its lock screen
# isn't answering within 20 s, or it quits first, as Quickshell does when its
# files don't load), back to Glass, so a broken Material never leaves you
# without a shell. This process becomes the shell (same PID).
if [[ $look == material ]]; then
    (
        me=$$
        sig="${HYPRLAND_INSTANCE_SIGNATURE:-}"
        for _ in $(seq 40); do
            sleep 0.5
            kill -0 "$me" 2>/dev/null || break
            "$QS" -p "$MATERIAL_ROOT" ipc show 2>/dev/null | grep -q "^target lock$" && exit 0
        done
        kill "$me" 2>/dev/null && sleep 0.5
        kill -0 "$me" 2>/dev/null && kill -9 "$me" 2>/dev/null
        python3 -I -c 'import json, sys; p = sys.argv[1]; j = json.load(open(p)); j.setdefault("appearance", {})["look"] = "glass"; open(p, "w").write(json.dumps(j, indent=4))' \
            "${XDG_CONFIG_HOME:-$HOME/.config}/irice/config.json" || exit 0
        notify-send -a iRice "Back to the Glass look" "The Material look (experimental) didn't start, so iRice went back to Glass." 2>/dev/null
        # (unless another shell was started for this session meanwhile)
        for p in $(pgrep -x irice-shell; pgrep -x qs; pgrep -x quickshell); do
            [[ $p == "$me" ]] && continue
            e=$( { tr '\0' '\n' <"/proc/$p/environ"; } 2>/dev/null)
            grep -qx "IRICE_SHELL=1" <<<"$e" && grep -qx "HYPRLAND_INSTANCE_SIGNATURE=$sig" <<<"$e" && exit 0
        done
        setsid -f "$SELF_DIR/assets/scripts/launch-shell.sh" -p "$SELF_DIR" </dev/null >/dev/null 2>&1
    ) </dev/null >/dev/null 2>&1 &
    disown
fi
# Its own process name. CachyOS's ananicy-cpp files every "qs" as a
# background service (nice 10), so the shell stuttered whenever an app was
# busy; started through a link called irice-shell, the process takes that
# name, which only iRice's own rule (assets/ananicy/irice.rules: interactive,
# like GNOME Shell and KWin) matches. Not while something from before the
# name still watches the shell: an update installing this version (its
# script is a copy of the old one) or a tty session's watchdog started
# before it (tty-shell.sh notes the ones that know). They look for "qs": the
# update screen would say the shell didn't start, the watchdog would start
# another. The next start takes the name.
old_watcher() {
    local p f
    for p in $(pgrep -f '/irice-update\.[^/]*/run\.sh'); do
        f=$( { tr '\0' '\n' <"/proc/$p/cmdline"; } 2>/dev/null | sed -n 2p)
        [[ -f $f ]] && ! grep -q 'pgrep -x irice-shell' "$f" && return 0
    done
    for p in $(pgrep -f 'tty-shell\.sh watchdog'); do
        { tr '\0' '\n' <"/proc/$p/environ"; } 2>/dev/null | grep -qx "HYPRLAND_INSTANCE_SIGNATURE=${HYPRLAND_INSTANCE_SIGNATURE:-}" || continue
        [[ "$(cat "${XDG_RUNTIME_DIR:-/tmp}/hypr/${HYPRLAND_INSTANCE_SIGNATURE:-}/irice-watchdog.names" 2>/dev/null)" == "$p" ]] || return 0
    done
    return 1
}
QSPATH="$(command -v "$QS" 2>/dev/null || true)"
LINKS="${XDG_RUNTIME_DIR:-/tmp}/irice-bin"
if [[ -n "$QSPATH" ]] && ! old_watcher && mkdir -p -m 700 "$LINKS" 2>/dev/null && ln -sfn "$QSPATH" "$LINKS/irice-shell" 2>/dev/null; then
    QS="$LINKS/irice-shell"
fi
exec "$QS" "${args[@]}"
