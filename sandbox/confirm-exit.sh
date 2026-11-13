#!/bin/sh
# Ends the session only on a second press within 3 s: a stray Super+Shift+Escape
# or Ctrl+Alt+Backspace used to log out at once. Hyprland's own notice, not
# the shell's, so it still works when the shell is frozen.
stamp="${XDG_RUNTIME_DIR:-/tmp}/tessera-exit-armed"
now=$(date +%s)
if [ -f "$stamp" ] && [ $((now - $(cat "$stamp" 2>/dev/null || echo 0))) -le 3 ]; then
    rm -f "$stamp"
    exec hyprctl dispatch "hl.dsp.exit()"
fi
echo "$now" >"$stamp"
hyprctl notify 1 3000 "rgb(ff9f0a)" "Press again to log out"
