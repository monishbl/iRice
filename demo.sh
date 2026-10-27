#!/usr/bin/env bash
# Show iRice for real, on its own console, without touching your desktop.
#
#   ./demo.sh           from a text console: checks, then starts the demo
#   ./demo.sh --check   only the checks (works from anywhere; rehearse with it)
#
# How: switch to a free console (Ctrl+Alt+F3), log in, cd to this folder and
# run ./demo.sh. It starts a real Hyprland session with iRice (the same one as
# ./dev.sh tty: your own ~/.config is never used or changed; iRice keeps its
# settings in .sandbox/home), then a track starts playing and a few
# notifications arrive. Super+Shift+Escape ends it; then go back to your
# desktop with Ctrl+Alt+F1 or F2 (whichever it's on).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_ONLY=0
[[ "${1:-}" == "--check" ]] && CHECK_ONLY=1
[[ "${1:-}" == "-h" || "${1:-}" == "--help" ]] && { sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0; }

ok() { printf '  \033[32m✓\033[0m %s\n' "$1"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$1"; WARN=$((WARN + 1)); }
bad() { printf '  \033[31m✗\033[0m %s\n' "$1"; FAIL=$((FAIL + 1)); }
FAIL=0 WARN=0

echo "iRice demo: checking…"

# Hyprland, and the glass plugin built for this version
if command -v Hyprland >/dev/null && command -v start-hyprland >/dev/null; then
    hv=$(Hyprland --version 2>/dev/null | sed -n '1s/^Hyprland \([0-9.]*\).*/\1/p')
    ok "Hyprland $hv"
else
    bad "Hyprland isn't installed (needs Hyprland and start-hyprland)"
fi
so="$ROOT/hypr/plugin/irice-glass/irice-glass.so"
if [[ ! -f "$so" ]]; then
    bad "Glass plugin isn't built: run  make -C hypr/plugin/irice-glass  (needs the Hyprland headers)"
elif [[ -f /usr/include/hyprland/src/version.h && "$so" -ot /usr/include/hyprland/src/version.h ]]; then
    warn "Glass plugin is older than your Hyprland: rebuild with  make -C hypr/plugin/irice-glass  (glass falls back to frosted otherwise)"
else
    ok "Glass plugin built"
fi

# Quickshell: iRice's own build, or the system one
if [[ -x "$HOME/.local/share/irice/quickshell/bin/qs" ]]; then
    ok "Quickshell (iRice build)"
elif command -v qs >/dev/null; then
    ok "Quickshell ($(qs --version 2>/dev/null | head -1))"
else
    bad "Quickshell isn't installed"
fi

# Fonts and helpers the demo uses
fonts=$(fc-list : family 2>/dev/null)
grep -qi '^Inter' <<<"$fonts" && ok "Inter font" || warn "Inter font missing: text falls back to another font"
grep -qi 'emoji' <<<"$fonts" && ok "Emoji font" || warn "No colour emoji font (noto-fonts-emoji)"
command -v notify-send >/dev/null && ok "notify-send" || warn "notify-send missing: no demo notifications"
python3 -c 'import gi' 2>/dev/null && ok "Now Playing demo track (python-gobject)" || warn "python-gobject missing: no demo track"
[[ -f "$ROOT/.sandbox/home/.config/irice/config.json" ]] && ok "Settings (from .sandbox/home)" || warn "No saved settings yet: iRice starts with its defaults"

# Power
if bat=$(ls -d /sys/class/power_supply/BAT* 2>/dev/null | head -1) && [[ -n "$bat" ]]; then
    cap=$(cat "$bat/capacity" 2>/dev/null || echo 100); st=$(cat "$bat/status" 2>/dev/null || echo "")
    if [[ "$st" == "Discharging" && "$cap" -lt 25 ]]; then
        warn "Battery at $cap%: plug in for the demo"
    else
        ok "Power: $st, $cap%"
    fi
fi

# Where it's run from
if [[ -n "${WAYLAND_DISPLAY:-}" || -n "${DISPLAY:-}" ]]; then
    if [[ $CHECK_ONLY == 1 ]]; then
        ok "(checks only: the demo itself starts from a text console)"
    else
        bad "Run the demo from a text console, not inside your desktop: Ctrl+Alt+F3, log in, then  cd $ROOT && ./demo.sh"
    fi
fi

echo
if [[ $FAIL -gt 0 ]]; then
    echo "Fix the ✗ items above first."
    exit 1
fi
[[ $WARN -gt 0 ]] && echo "Ready, with $WARN note(s) above." || echo "Ready."
[[ $CHECK_ONLY == 1 ]] && exit 0

cat <<'EOF'

Starting iRice. In the demo:
  Super+Space          Spotlight            Alt+Tab               switch windows
  Super+Alt+C          Control Center       Click the menu bar    Wi-Fi, battery, Now Playing
  Super+.              Emoji picker         Super+Return          terminal
  Super+Shift+Escape   end the demo
EOF
sleep 3

# The nested dev sandbox shares the settings: stop it first
"$ROOT/dev.sh" status 2>/dev/null | grep -q running && "$ROOT/dev.sh" down >/dev/null 2>&1

export IRICE_ROOT="$ROOT" IRICE_DEMO=1
export HYPRLAND_NO_SD_VARS=1 # don't touch the user systemd your usual desktop shares (sandbox/hypr/tty.lua)
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN GEMINI_API_KEY OPENAI_API_KEY OPENROUTER_API_KEY
dbus-run-session -- start-hyprland -- --config "$ROOT/sandbox/hypr/tty.lua"
code=$?

# Anything the demo started that outlived the session
for p in $(pgrep -f "sandbox/demo-extras.sh|sandbox/tools/fakeplayer.py"); do
    tr '\0' '\n' <"/proc/$p/environ" 2>/dev/null | grep -qx "IRICE_DEMO=1" && kill "$p" 2>/dev/null
done

echo
[[ $code -eq 0 ]] && echo "Demo ended." || echo "Demo ended (Hyprland exited with $code; its log: ~/.local/share/hyprland/ or \$XDG_RUNTIME_DIR/hypr)."
echo "Back to your desktop: Ctrl+Alt+F1 or Ctrl+Alt+F2. If the screen ever freezes: ./dev.sh tty-stop from another console."
