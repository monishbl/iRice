#!/usr/bin/env bash
# Talks to the running Tessera shell over IPC, whichever look it runs (Glass
# from this shell folder, Material from its root of links, launch-shell.sh):
#   shell-ipc.sh <ipc arguments>      e.g. shell-ipc.sh call lock lock
# QS=<quickshell binary> picks the binary (start-idle.sh passes the shell's).
set -uo pipefail
SHELL_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd -P)"
# Tessera's folder under $1, or iRice's while it hasn't been moved yet (the
# shell moves it as it starts: migrate-names.py)
app_dir() { if [[ ! -e "$1/tessera" && -d "$1/irice" ]]; then echo "$1/irice"; else echo "$1/tessera"; fi; }
MATERIAL_ROOT="$(app_dir "${XDG_CACHE_HOME:-$HOME/.cache}")/material-shell"

# The same Quickshell the shell runs on (Tessera's own build when there is
# one; see launch-shell.sh): asking it over IPC needs the same version
if [[ -z "${QS:-}" ]]; then
    QS=qs
    # (…/irice: Tessera's folder from before it was renamed, not moved yet)
    for d in "${XDG_DATA_HOME:-}/tessera" "$HOME/.local/share/tessera" "${XDG_DATA_HOME:-}/irice" "$HOME/.local/share/irice"; do
        if [[ -n "$d" && -x "$d/quickshell/bin/qs" && -d "$d/quickshell/lib/qt6/qml/Quickshell" ]]; then
            QS="$d/quickshell/bin/qs"
            export QML_IMPORT_PATH="$d/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
            break
        fi
    done
fi

# The root with a shell running: one that lists IPC targets (only one look
# runs at a time; asking a root with none, qs says "No running instances"
# and still exits 0, so the exit code can't tell)
for root in "$SHELL_DIR" "$MATERIAL_ROOT"; do
    [[ -e "$root/shell.qml" ]] || continue
    if "$QS" -p "$root" ipc show 2>/dev/null | grep -q "^target "; then
        exec "$QS" -p "$root" ipc "$@"
    fi
done
echo "shell-ipc.sh: no Tessera shell is running" >&2
exit 1
