#!/usr/bin/env bash
# Switches the look (Glass or Material) by restarting the shell behind a
# full-screen switching screen (assets/lookswitch):
#   switch-look.sh glass|material
# The shell that runs it has already saved the choice (appearance.look); this
# waits for the file to say so, shows the screen, restarts the shell through
# repair-shell.sh (launch-shell.sh then starts the chosen look's root), waits
# for the new shell to answer, and lets the screen fade out. Leaving Material
# puts Hyprland's window styling (borders, shadow, corners: set live by the
# Material look) back to the config's. Your apps and windows aren't touched.
set -uo pipefail
TO="${1:-}"
[[ $TO == glass || $TO == material ]] || { echo "usage: switch-look.sh glass|material" >&2; exit 2; }
SHELL_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd -P)"
MATERIAL_DIR="$(dirname "$SHELL_DIR")/material"
RUN="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
# Tessera's folder under $1, or iRice's while it hasn't been moved yet (the
# shell moves it as it starts: migrate-names.py)
app_dir() { if [[ ! -e "$1/tessera" && -d "$1/irice" ]]; then echo "$1/irice"; else echo "$1/tessera"; fi; }
CONFIG="$(app_dir "${XDG_CONFIG_HOME:-$HOME/.config}")/config.json"
CACHE="$(app_dir "${XDG_CACHE_HOME:-$HOME/.cache}")"
LOG="$RUN/tessera-look.log"
exec >>"$LOG" 2>&1
echo "== $(date '+%F %T') switch to $TO"

if [[ $TO == material && ! -f "$MATERIAL_DIR/shell.qml" ]]; then
    notify-send -a Tessera "The Material look isn't installed" "Its files (material/, beside the shell) are missing. Update Tessera, then try again." 2>/dev/null || true
    exit 1
fi
# One switch at a time (8>&- on everything it starts: a shell or the screen
# holding the lock would block the next switch)
exec 8>"$RUN/tessera-look.lock"
flock -n 8 || { echo "already switching"; exit 0; }

# The choice is saved (the shell writes its settings a moment after a change)
saved() { python3 -I -c 'import json, sys; sys.exit(json.load(open(sys.argv[1])).get("appearance", {}).get("look", "glass") != sys.argv[2])' "$CONFIG" "$TO" 2>/dev/null; }
for _ in $(seq 30); do
    saved && break
    sleep 0.1
done
saved || { echo "the setting wasn't saved"; exit 1; }

# The screen, from a copy (it must outlive the shell, and a Glass install has
# the Material fonts only in material/)
QS=qs
# (…/irice: Tessera's folder from before it was renamed, not moved yet)
for d in "${XDG_DATA_HOME:-}/tessera" "$HOME/.local/share/tessera" "${XDG_DATA_HOME:-}/irice" "$HOME/.local/share/irice"; do
    if [[ -n "$d" && -x "$d/quickshell/bin/qs" && -d "$d/quickshell/lib/qt6/qml/Quickshell" ]]; then
        QS="$d/quickshell/bin/qs"
        export QML_IMPORT_PATH="$d/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
        break
    fi
done
T="$(mktemp -d "$RUN/tessera-look.XXXXXX")"
trap 'rm -rf "$T"' EXIT
mkdir "$T/ui"
cp "$SHELL_DIR/assets/lookswitch/shell.qml" "$T/ui/"
cp "$SHELL_DIR/assets/fonts/Phosphor-Fill.ttf" "$T/ui/"
cp "$SHELL_DIR/common/materialshape.frag.qsb" "$T/ui/" 2>/dev/null
cp "$MATERIAL_DIR/assets/fonts/GoogleSansFlex.ttf" "$T/ui/" 2>/dev/null
echo run >"$T/status"
python3 -I - "$T/look.json" "$TO" "$CACHE" "$CONFIG" <<'PY'
import glob, json, os, sys
out, to, cache, config = sys.argv[1:]
try:
    mode = json.load(open(config)).get("appearance", {}).get("mode", "dark")
except (OSError, ValueError):
    mode = "dark"
backdrops = {os.path.basename(p)[len("backdrop-"):-len(".png")]: p for p in glob.glob(os.path.join(cache, "backdrop-*.png"))}
json.dump({"to": to, "dark": mode != "light", "backdrops": backdrops}, open(out, "w"))
PY
env -u TESSERA_TTY -u TESSERA_SHELL setsid -f "$QS" -p "$T/ui" >"$T/ui.log" 2>&1 </dev/null 8>&-
sleep 0.5 # (covered)

# Leaving Material: the window styling it set live goes back to the config's
if [[ $TO == glass ]]; then
    hyprctl reload config-only >/dev/null 2>&1
    hyprctl tessera-glass controls-style glass >/dev/null 2>&1
    hyprctl irice-glass controls-style glass >/dev/null 2>&1 # (a plugin built before the rename)
fi

# The shell, restarted into the chosen look. In the dev sandbox through its
# own reload, which only touches the sandbox's shells (repair-shell.sh also
# clears shells of other sessions it thinks have ended, and from inside the
# sandbox's container a real session can look ended)
if [[ ${TESSERA_SANDBOX:-${IRICE_SANDBOX:-0}} == 1 && ${TESSERA_TTY:-${IRICE_TTY:-0}} != 1 && -x "${TESSERA_ROOT:-${IRICE_ROOT:-}}/sandbox/reload-shell.sh" ]]; then
    "$TESSERA_ROOT/sandbox/reload-shell.sh" 8>&-
else
    TESSERA_REPAIR_QUIET=1 "$SHELL_DIR/assets/scripts/repair-shell.sh" 8>&-
fi
# Up once it answers (then a moment to draw its first frames)
for _ in $(seq 100); do
    QS="$QS" "$SHELL_DIR/assets/scripts/shell-ipc.sh" show >/dev/null 2>&1 && break
    sleep 0.2
done
sleep 1.2
echo done >"$T/status"
# The screen fades out and quits; then the copy goes
for _ in $(seq 40); do
    pgrep -f -- "-p $T/ui" >/dev/null || break
    sleep 0.1
done
echo "switched to $TO"
