#!/usr/bin/env bash
# Talks to the running iRice shell over IPC, whichever look it runs (Glass
# from this shell folder, Material from its root of links, launch-shell.sh):
#   shell-ipc.sh <ipc arguments>      e.g. shell-ipc.sh call lock lock
# QS=<quickshell binary> picks the binary (start-idle.sh passes the shell's).
set -uo pipefail
SHELL_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd -P)"
MATERIAL_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/irice/material-shell"

# The same Quickshell the shell runs on (iRice's own build when there is
# one; see launch-shell.sh): asking it over IPC needs the same version
if [[ -z "${QS:-}" ]]; then
    QS=qs
    for d in "${XDG_DATA_HOME:-}" "$HOME/.local/share"; do
        if [[ -n "$d" && -x "$d/irice/quickshell/bin/qs" && -d "$d/irice/quickshell/lib/qt6/qml/Quickshell" ]]; then
            QS="$d/irice/quickshell/bin/qs"
            export QML_IMPORT_PATH="$d/irice/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
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
echo "shell-ipc.sh: no iRice shell is running" >&2
exit 1
