#!/usr/bin/env bash
# The recovery screen: when iRice's shell keeps stopping soon after an
# update (keep-shell.sh counts its restarts), a terminal window offers to go
# back to before that update. It needs nothing from the shell, only a
# terminal, Python and pkexec (which asks for the password right there).
#   recover.sh [launch arguments]            open it in a terminal window
#   recover.sh --inside [launch arguments]   the screen itself
#   recover.sh --check                       exit 0 when there's a recent
#                                            update to go back from (3 days)
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSUPDATE="$HERE/sysupdate.py"

recent_update() {
    python3 -I - "$SYSUPDATE" <<'PY'
import json, subprocess, sys, time
try:
    u = json.loads(subprocess.run([sys.executable, "-I", sys.argv[1], "undo-check"], capture_output=True,
                                  text=True, timeout=120).stdout)
except Exception:
    sys.exit(1)
sys.exit(0 if u.get("packages") and time.time() - u.get("at", 0) < 3 * 86400 else 1)
PY
}

if [[ "${1:-}" == --check ]]; then
    recent_update
    exit
fi

if [[ "${1:-}" != --inside ]]; then
    # (whichever terminal is installed; each takes its command differently)
    for t in foot kitty alacritty ghostty wezterm konsole gnome-terminal kgx ptyxis xfce4-terminal xterm; do
        command -v "$t" >/dev/null || continue
        case "$t" in
            wezterm) exec wezterm start -- "$0" --inside "$@" ;;
            gnome-terminal | kgx | ptyxis) exec "$t" -- "$0" --inside "$@" ;;
            xfce4-terminal) exec xfce4-terminal -x "$0" --inside "$@" ;;
            *) exec "$t" -e "$0" --inside "$@" ;;
        esac
    done
    exit 1
fi
shift
launch=("$@")
[[ ${#launch[@]} -eq 0 ]] && launch=(-c irice)

when="$(python3 -I -c 'import json,subprocess,sys,time
try:
    u = json.loads(subprocess.run([sys.executable, "-I", sys.argv[1], "undo-check"], capture_output=True, text=True, timeout=120).stdout)
    print(time.strftime("%A %d %B, %H:%M", time.localtime(u["at"])))
except Exception:
    print("")' "$SYSUPDATE" 2>/dev/null)"

while true; do
    clear
    printf '\n  \033[1mThe desktop keeps stopping\033[0m\n\n'
    printf '  iRice'"'"'s shell has stopped three times%s.\n\n' "${when:+ since the update on $when}"
    printf '  1  Go back to before that update (asks for your password)\n'
    printf '  2  Try starting the desktop again\n'
    printf '  3  Close this and use a terminal\n\n'
    read -r -p '  Choose 1, 2 or 3: ' choice || exit 0
    case "$choice" in
        1)
            echo
            if python3 -I "$SYSUPDATE" undo --terminal; then
                echo
                read -r -p '  Restart the computer now? [Y/n] ' again || exit 0
                [[ "$again" =~ ^[Nn] ]] || systemctl reboot
            fi
            echo
            read -r -p '  Press Enter to go back to the choices. ' _ || exit 0
            ;;
        2)
            setsid -f "$HERE/launch-shell.sh" "${launch[@]}" >/dev/null 2>&1
            exit 0
            ;;
        3)
            exec "${SHELL:-bash}"
            ;;
    esac
done
