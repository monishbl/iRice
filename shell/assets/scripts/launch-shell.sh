#!/usr/bin/env bash
# Starts the iRice shell with the right icon theme, then becomes it:
#   launch-shell.sh -c irice        (installed)
#   launch-shell.sh -p <shell dir>  (from the repo)
# Quickshell only reads the icon theme at startup (QS_ICON_THEME); without
# one it falls back to bare "hicolor" and most app icons are missing. The
# theme is, in order: Settings → Appearance → Icon theme, KDE's, GNOME/GTK's,
# else the first good one installed.
set -uo pipefail

themes_dirs=("${XDG_DATA_HOME:-$HOME/.local/share}/icons" "$HOME/.icons" /usr/local/share/icons /usr/share/icons)
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
# Tahoe-like default: MacTahoe icons when installed (light or dark with iRice)
if [[ -z "$pick" ]] && installed MacTahoe; then
    if [[ -f "$cfg" ]] && grep -q '"mode"[[:space:]]*:[[:space:]]*"light"' "$cfg"; then pick=MacTahoe; else pick=MacTahoe-dark; fi
    installed "$pick" || pick=MacTahoe
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
exec "$QS" "$@"
