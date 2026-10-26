#!/usr/bin/env sh
# Lists wallpaper sets in a folder (and one level down), one per line,
# tab-separated:
#   pair <name> <light picture> <dark picture>   "<Name> Light" + "<Name> Dark"
#   time <name> <folder> <first picture>         a sub-folder of 2+ pictures
# "Light"/"Dark" are whole words in any case ("Daylight" doesn't count).
# Symlinked folders are followed.
d="$1"
[ -d "$d" ] || exit 0
tab="$(printf '\t')"
find -L "$d" -maxdepth 2 -type f \( -iname '* light.jpg' -o -iname '* light.jpeg' -o -iname '* light.png' -o -iname '* light.webp' \) | sort | while IFS= read -r f; do
    case "$f" in *"$tab"*) continue ;; esac
    b="$(printf '%s\n' "$f" | sed 's/ [Ll][Ii][Gg][Hh][Tt]\.[^.]*$//')"
    dk="$(find -L "$(dirname "$f")" -maxdepth 1 -type f -iname "$(basename "$b") dark.*" | sort | head -1)"
    [ -n "$dk" ] && printf 'pair\t%s\t%s\t%s\n' "$(basename "$b")" "$f" "$dk"
done
find -L "$d" -mindepth 1 -maxdepth 2 -type d | sort | while IFS= read -r sub; do
    case "$sub" in *"$tab"*) continue ;; esac
    # A folder of light/dark pairs isn't a time-of-day set
    [ -n "$(find -L "$sub" -maxdepth 1 -type f -iname '* light.*' | head -1)" ] && continue
    pics="$(find -L "$sub" -maxdepth 1 -type f \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \) | sort)"
    [ "$(printf '%s\n' "$pics" | grep -c .)" -ge 2 ] && printf 'time\t%s\t%s\t%s\n' "$(basename "$sub")" "$sub" "$(printf '%s\n' "$pics" | head -1)"
done
exit 0
