#!/usr/bin/env bash
# Your keyboard layout as the system has it (localectl: what the console and
# login screens use), written as Hyprland Lua. hyprland.lua loads it after
# its "us" default; custom.lua still wins over it. Run by install.sh (into
# ~/.config/hypr/irice) and tools/setup-tty.sh (for the test session).
#   tools/keyboard-lua.sh <file>   writes it, or removes it when no layout is set;
#                                  prints what it did
set -uo pipefail
out="${1:?usage: keyboard-lua.sh <file>}"
status=$(localectl status 2>/dev/null) || status=""
field() { # X11 Layout, Variant, Model or Options; empty when unset
    local v
    v=$(sed -n "s/^[[:space:]]*X11 $1:[[:space:]]*//p" <<<"$status" | head -1)
    [[ "$v" == "n/a" || "$v" == "(unset)" ]] && v=""
    printf '%s' "$v"
}
layout=$(field Layout) variant=$(field Variant) model=$(field Model) options=$(field Options)
# Only what layouts are made of (letters, digits and , : _ - + ( ) .), so
# nothing odd ends up inside the Lua strings
for v in "$layout" "$variant" "$model" "$options"; do
    [[ "$v" =~ ^[A-Za-z0-9_,:+().-]*$ ]] || { layout=""; break; }
done
if [[ -z "$layout" ]]; then
    rm -f "$out"
    echo "Keyboard layout: us (the system has none set: localectl set-x11-keymap <layout>, then run this again)"
    exit 0
fi
fields="kb_layout = \"$layout\""
[[ -n "$variant" ]] && fields+=", kb_variant = \"$variant\""
[[ -n "$model" ]] && fields+=", kb_model = \"$model\""
[[ -n "$options" ]] && fields+=", kb_options = \"$options\""
mkdir -p "$(dirname "$out")"
cat >"$out.tmp" <<LUA
-- Written by iRice from your system's keyboard layout (localectl status); it's
-- written again each time the installer runs. To change it: localectl
-- set-x11-keymap <layout> [model] [variant] [options], or set kb_layout in
-- ~/.config/hypr/custom.lua, which wins over this.
hl.config({ input = { $fields } })
LUA
mv "$out.tmp" "$out"
echo "Keyboard layout: $layout${variant:+ ($variant)}"
