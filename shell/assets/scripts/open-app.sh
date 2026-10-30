#!/usr/bin/env bash
# Starts the default app for a role, for keybinds and the shell:
#   open-app.sh terminal | files | browser | code | text | office | mixer | tasks
#   open-app.sh terminal [--hold] -- command [args...]   (a command in the terminal;
#       --hold keeps the window open after it ends, to read its output)
# The choice from Settings → Default apps (config.json "defaultApps.<role>",
# a command) wins; otherwise the first one installed from the list below.
# IRICE_USER_DIR overrides where config.json is (the tty test session).
role="${1:-terminal}"
shift
hold=0
if [[ "$role" == terminal ]]; then
    [[ "${1:-}" == --hold ]] && { hold=1; shift; }
    [[ "${1:-}" == -- ]] && shift
fi
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
if [[ -n "$chosen" && ! ( "$role" == terminal && $# -gt 0 ) ]]; then
    exec sh -c "$chosen"
fi

# Runs "$@" in the terminal named by $1 (a command line: "kitty", "foot -o
# pad=8x8"), each terminal its own way: how it takes a command, and how it
# keeps the window open afterwards (--hold). One without a hold flag waits
# for Enter instead.
in_terminal() {
    local -a t cmd
    read -ra t <<<"$1"
    shift
    cmd=("$@")
    local bin="${t[0]##*/}" own_hold=""
    case "$bin" in
        kitty | foot | footclient | alacritty | konsole | xfce4-terminal) own_hold="--hold" ;;
        xterm | urxvt | rxvt) own_hold="-hold" ;;
        ghostty) own_hold="--wait-after-command" ;;
    esac
    if ((hold)); then
        if [[ -n "$own_hold" ]]; then
            t+=("$own_hold")
        else
            cmd=(sh -c '"$@"; printf "\n[Done. Press Enter to close]"; read -r _' sh "${cmd[@]}")
        fi
    fi
    case "$bin" in
        kitty | foot | footclient) exec "${t[@]}" "${cmd[@]}" ;;
        wezterm)
            [[ " ${t[*]} " == *" start "* ]] || t+=(start)
            exec "${t[@]}" -- "${cmd[@]}" ;;
        gnome-terminal | kgx | ptyxis | mate-terminal) exec "${t[@]}" -- "${cmd[@]}" ;;
        xfce4-terminal | terminator) exec "${t[@]}" -x "${cmd[@]}" ;;
        *) exec "${t[@]}" -e "${cmd[@]}" ;; # alacritty, konsole, ghostty, xterm, st, urxvt, qterminal, …
    esac
}

if [[ "$role" == terminal ]]; then
    term="$chosen"
    if [[ -z "$term" && -n "${TERMINAL:-}" ]] && command -v "${TERMINAL%% *}" >/dev/null 2>&1; then
        term="$TERMINAL"
    fi
    if [[ -z "$term" ]]; then
        for c in kitty foot alacritty wezterm ghostty konsole gnome-terminal kgx ptyxis xfce4-terminal xterm; do
            command -v "$c" >/dev/null 2>&1 && { term="$c"; break; }
        done
    fi
    if [[ -z "$term" ]]; then
        if command -v xdg-terminal-exec >/dev/null 2>&1; then
            exec xdg-terminal-exec "$@"
        fi
        notify-send -a iRice "No terminal" "Install one (kitty, foot, alacritty…), or choose it in Settings → Default apps." 2>/dev/null
        exit 1
    fi
    (($# == 0)) && exec sh -c "$term"
    in_terminal "$term" "$@"
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
