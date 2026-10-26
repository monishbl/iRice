#!/usr/bin/env bash
# Starts the default app for a role, for keybinds and the shell:
#   open-app.sh terminal | files | browser | code | text | office | mixer | tasks
# The choice from Settings → Default apps (config.json "defaultApps.<role>",
# a command) wins; otherwise the first one installed from the list below.
# IRICE_USER_DIR overrides where config.json is (the tty test session).
role="${1:-terminal}"
cfgdir="${IRICE_USER_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/irice}"

chosen=""
if [[ -f "$cfgdir/config.json" ]] && command -v python3 >/dev/null; then
    chosen=$(python3 -I -c 'import json,sys
try:
    c = json.load(open(sys.argv[1]))
    v = c.get("defaultApps", {}).get(sys.argv[2], "") or ""
    if not v and sys.argv[2] == "files":
        v = c.get("desktop", {}).get("fileManager", "") or ""
    print(v)
except Exception:
    pass' "$cfgdir/config.json" "$role" 2>/dev/null)
fi
if [[ -n "$chosen" ]]; then
    exec sh -c "$chosen"
fi

first() {
    for c in "$@"; do
        local bin="${c%% *}"
        if command -v "$bin" >/dev/null 2>&1; then
            exec sh -c "$c"
        fi
    done
    notify-send -a iRice "No app for $role" "Install one, or choose it in Settings → Default apps." 2>/dev/null
    exit 1
}

case "$role" in
    terminal) first kitty foot alacritty wezterm konsole kgx xterm ;;
    files) first nautilus dolphin nemo thunar "kitty -e yazi" ;;
    browser)
        # The system's default browser
        d=$(xdg-settings get default-web-browser 2>/dev/null)
        if [[ -n "$d" ]] && command -v gtk-launch >/dev/null; then
            exec gtk-launch "${d%.desktop}"
        fi
        first zen-browser firefox google-chrome-stable chromium brave ;;
    code) first code codium zed zeditor cursor windsurf "kitty -e nvim" ;;
    text) first gnome-text-editor kate mousepad "kitty -e nvim" ;;
    office) first libreoffice onlyoffice-desktopeditors wps ;;
    mixer) first pavucontrol pavucontrol-qt ;;
    tasks) first gnome-system-monitor "plasma-systemmonitor --page-name Processes" "kitty -e btop" "kitty -e htop" ;;
    *) echo "unknown role: $role" >&2; exit 2 ;;
esac
