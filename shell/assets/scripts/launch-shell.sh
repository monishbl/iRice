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
exec qs "$@"
