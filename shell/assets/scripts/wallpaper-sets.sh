#!/usr/bin/env sh
# Lists wallpaper sets in a folder (and one level down), one per line:
#   pair|<name>|<light picture>|<dark picture>   "<Name> Light" + "<Name> Dark"
#   time|<name>|<folder>|<first picture>         a sub-folder of 2+ pictures
# Symlinked folders are followed.
d="$1"
[ -d "$d" ] || exit 0
find -L "$d" -maxdepth 2 -type f \( -iname '* light.jpg' -o -iname '* light.jpeg' -o -iname '* light.png' -o -iname '* light.webp' \) | sort | while read -r f; do
    b="${f% [Ll]ight.*}"
    dk=$(ls "$b Dark."* "$b dark."* 2>/dev/null | head -1)
    [ -n "$dk" ] && printf 'pair|%s|%s|%s\n' "$(basename "$b")" "$f" "$dk"
done
find -L "$d" -mindepth 1 -maxdepth 2 -type d | sort | while read -r sub; do
    ls "$sub"/*[Ll]ight.* >/dev/null 2>&1 && continue
    pics=$(find -L "$sub" -maxdepth 1 -type f \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \) | sort)
    [ "$(printf '%s\n' "$pics" | grep -c .)" -ge 2 ] && printf 'time|%s|%s|%s\n' "$(basename "$sub")" "$sub" "$(printf '%s\n' "$pics" | head -1)"
done
exit 0
