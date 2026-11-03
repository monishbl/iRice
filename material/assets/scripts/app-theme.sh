#!/usr/bin/env bash
# Colours apps to match iRice (Settings → Appearance → Match apps), with
# matugen: from the accent colour or the wallpaper, light or dark.
#   app-theme.sh <dark|light|off> <accent #rrggbb> <source: accent|wallpaper> <wallpaper path> <targets: gtk,qt,terminals,browsers> [desktop dark|light] [Material roles JSON]
# In the Material look the shell passes its own colour roles (Theme.m3) and
# the templates are filled with those (app-theme-render.py), not matugen's:
# matugen's scheme is the 2021 spec, the shell's the 2025 one, and only the
# shell's own colours match it exactly. matugen isn't needed then.
# Writes its own colour files and adds one include line (once) to each app's
# config so they're used; "off" empties those files again. Desktop light/dark
# preferences and Dolphin's palette are synced independently of accent matching.
# Apps that can reload do (kitty, Firefox via Pywalfox, Chromium).
set -uo pipefail
mode="$1" accent="$2" source="$3" wall="$4" targets=",$5,"
CFG="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
T="$(cd "$(dirname "${BASH_SOURCE[0]}")/../theming" && pwd)"
DESKTOP="$(dirname "${BASH_SOURCE[0]}")/desktop-theme.py"
want() { [[ "$targets" == *",$1,"* ]]; }
desktop_mode="${6:-$mode}"
roles="${7:-}" # the Material look's colour roles (empty in Glass)
RENDER="$(dirname "${BASH_SOURCE[0]}")/app-theme-render.py"
[[ "$desktop_mode" == light ]] || desktop_mode=dark
# Light/dark is a desktop preference, independent of optional accent matching.
if [[ "$mode" != off ]] && want qt && { [[ -n "$roles" ]] || command -v matugen >/dev/null; }; then
    python3 -I "$DESKTOP" "$desktop_mode" --keep-kde
else
    python3 -I "$DESKTOP" "$desktop_mode"
fi

# The generated files (and where their template is)
declare -A out=(
    [gtk3]="$CFG/gtk-3.0/irice-colors.css|gtk.css|gtk"
    [gtk4]="$CFG/gtk-4.0/irice-colors.css|gtk.css|gtk"
    [qt6ct]="$CFG/qt6ct/colors/iRice.conf|qt6ct.conf|qt"
    [kde]="$DATA/color-schemes/iRice.colors|kde.colors|qt"
    [foot]="$CFG/foot/irice-colors.ini|foot.ini|terminals"
    [kitty]="$CFG/kitty/irice-colors.conf|kitty.conf|terminals"
    [alacritty]="$CFG/alacritty/irice-colors.toml|alacritty.toml|terminals"
    [pywal]="$CACHE/wal/colors.json|pywal.json|browsers"
)

if [[ "$mode" == off ]]; then
    for k in "${!out[@]}"; do
        f="${out[$k]%%|*}"
        [[ -f "$f" && "$k" != pywal ]] && : >"$f"
    done
    command -v pkill >/dev/null && pkill -USR1 -x kitty 2>/dev/null
    exit 0
fi

# ANSI colours for terminals (the design system's semantic hues)
if [[ "$mode" == dark ]]; then
    red=F87171 green=4ADE80 yellow=FBBF24 blue=60A5FA magenta=E087CE cyan=25C2C2
else
    red=B91C1C green=15803D yellow=B45309 blue=1D4ED8 magenta=BB5FAA cyan=1D9999
fi
a="${accent#\#}"
accent_rgb="$((16#${a:0:2})),$((16#${a:2:2})),$((16#${a:4:2}))"
extra=$(printf '{"irice":{"accent":"#%s","accent_stripped":"%s","accent_rgb":"%s","wallpaper":"%s","red":"%s","green":"%s","yellow":"%s","blue":"%s","magenta":"%s","cyan":"%s"}}' \
    "$a" "$a" "$accent_rgb" "${wall//\"/}" "$red" "$green" "$yellow" "$blue" "$magenta" "$cyan")

if [[ -n "$roles" ]]; then
    # Material: the shell's own roles, filled in here
    pairs=()
    for k in "${!out[@]}"; do
        IFS='|' read -r f tpl grp <<<"${out[$k]}"
        want "$grp" && pairs+=("$T/$tpl=$f")
    done
    python3 -I "$RENDER" "$roles" "$extra" "${pairs[@]}" || { python3 -I "$DESKTOP" "$desktop_mode"; exit 1; }
else
    # Glass: matugen, from the accent or the wallpaper
    conf=$(mktemp --suffix=.toml)
    trap 'rm -f "$conf"' EXIT
    echo "[config]" >"$conf"
    for k in "${!out[@]}"; do
        IFS='|' read -r f tpl grp <<<"${out[$k]}"
        want "$grp" || continue
        mkdir -p "$(dirname "$f")"
        printf '\n[templates.%s]\ninput_path = "%s/%s"\noutput_path = "%s"\n' "$k" "$T" "$tpl" "$f" >>"$conf"
    done
    if [[ "$source" == wallpaper && -f "$wall" && "$wall" != *.mp4 && "$wall" != *.webm && "$wall" != *.mkv ]]; then
        matugen image "$wall" -m "$mode" -c "$conf" --import-json-string "$extra" -q --prefer saturation 2>/dev/null ||
            matugen color hex "#$a" -m "$mode" -c "$conf" --import-json-string "$extra" -q || { python3 -I "$DESKTOP" "$desktop_mode"; exit 1; }
    else
        matugen color hex "#$a" -m "$mode" -c "$conf" --import-json-string "$extra" -q || { python3 -I "$DESKTOP" "$desktop_mode"; exit 1; }
    fi
fi

# Include the colours (once) in each app's own config
include() { # file, line, [create if missing]
    local f="$1" line="$2"
    if [[ -f "$f" ]]; then
        grep -qF "irice-colors" "$f" || printf '%s\n%s' "$line" "$(cat "$f")" >"$f"
    elif [[ "${3:-}" == create ]]; then
        mkdir -p "$(dirname "$f")" && printf '%s\n' "$line" >"$f"
    fi
}
if want gtk; then
    include "$CFG/gtk-3.0/gtk.css" "@import 'irice-colors.css';" create
    include "$CFG/gtk-4.0/gtk.css" "@import 'irice-colors.css';" create
    # GTK apps follow light/dark too, on Adwaita (coloured by the files
    # above); a MacTahoe theme set by an older iRice is put back to it, any
    # other theme you chose is left alone
    if command -v gsettings >/dev/null; then
        gsettings set org.gnome.desktop.interface color-scheme "prefer-$mode" 2>/dev/null
        cur=$(gsettings get org.gnome.desktop.interface gtk-theme 2>/dev/null | tr -d "'")
        t=""
        [[ $cur == MacTahoe* || $cur == Adwaita* || $cur == adw-gtk3* ]] && t=$([[ $mode == dark ]] && echo Adwaita-dark || echo Adwaita)
        # Material: libadwaita's look for GTK 3 apps (adw-gtk3), when installed
        if [[ -n "$roles" ]]; then
            a3=$([[ $mode == dark ]] && echo adw-gtk3-dark || echo adw-gtk3)
            for d in /usr/share/themes "$DATA"/themes "$HOME"/.themes; do
                [[ -d "$d/$a3" ]] && t="$d/$a3" && break
            done
        fi
        [[ -n "$t" && ( $cur == MacTahoe* || $cur == Adwaita* || $cur == adw-gtk3* ) ]] && gsettings set org.gnome.desktop.interface gtk-theme "$(basename "$t")" 2>/dev/null
    fi
fi
if want qt; then
    python3 -I "$DESKTOP" "$desktop_mode" --scheme "$DATA/color-schemes/iRice.colors"
fi
if want terminals; then
    command -v foot >/dev/null && include "$CFG/foot/foot.ini" "include=$CFG/foot/irice-colors.ini" create
    command -v kitty >/dev/null && include "$CFG/kitty/kitty.conf" "include irice-colors.conf" create
    if command -v alacritty >/dev/null; then
        f="$CFG/alacritty/alacritty.toml"
        if [[ ! -f "$f" ]]; then
            printf '[general]\nimport = ["%s"]\n' "$CFG/alacritty/irice-colors.toml" >"$f"
        elif ! grep -qF "irice-colors" "$f" && ! grep -q '^\[general\]' "$f"; then
            printf '[general]\nimport = ["%s"]\n\n%s' "$CFG/alacritty/irice-colors.toml" "$(cat "$f")" >"$f"
        fi
    fi
    command -v pkill >/dev/null && pkill -USR1 -x kitty 2>/dev/null # kitty reloads its config
fi
if want browsers; then
    command -v pywalfox >/dev/null && pywalfox update >/dev/null 2>&1
    for b in chromium google-chrome-stable brave vivaldi-stable; do
        command -v "$b" >/dev/null && pgrep -x "${b%-stable}" >/dev/null && "$b" --no-startup-window --set-theme-color="$accent_rgb" >/dev/null 2>&1 &
    done
fi
exit 0
