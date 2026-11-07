#!/usr/bin/env bash
# Talks to the running Tessera shell over IPC, whichever look it runs (Glass
# from this shell folder, Material from its root of links, launch-shell.sh):
#   shell-ipc.sh <ipc arguments>      e.g. shell-ipc.sh call lock lock
# QS=<quickshell binary> picks the binary (start-idle.sh passes the shell's).
set -uo pipefail
SHELL_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd -P)"
# Standalone (rust.standalone, on unless set to false): no Quickshell runs;
# tessera-ui is the whole shell and answers over its own socket
# ($XDG_RUNTIME_DIR/tessera/ipc.sock), `tessera-ui ipc call <target> <fn> …`.
# Only while that socket is there, so with the setting on but the QML shell
# running (tessera-ui missing, or not started yet) calls still reach the QML.
if [[ "${1:-}" == call ]]; then
    sock="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/tessera/ipc.sock"
    if [[ -S "$sock" ]]; then
        cfg="${XDG_CONFIG_HOME:-$HOME/.config}/tessera/config.json"
        [[ -f "$cfg" ]] || cfg="${XDG_CONFIG_HOME:-$HOME/.config}/irice/config.json"
        if [[ "${TESSERA_STANDALONE:-}" == 1 ]] || ! python3 -I -c 'import json, sys; sys.exit(0 if json.load(open(sys.argv[1])).get("rust", {}).get("standalone") is False else 1)' "$cfg" 2>/dev/null; then
            for ui in "${TESSERA_UI:-}" "${XDG_DATA_HOME:-$HOME/.local/share}/tessera/bin/tessera-ui" "$HOME/.local/share/tessera/bin/tessera-ui" "$(command -v tessera-ui 2>/dev/null)"; do
                [[ -n "$ui" && -x "$ui" ]] && exec "$ui" ipc "$@"
            done
        fi
    fi
fi
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
