#!/usr/bin/env bash
# Opens iRice's share sheet with these files: what the file managers'
# "Share…" entries run (file-manager-share.sh installs them).
#   share-files.sh <path or file:// URI>...
# Each is passed as a file:// URI, percent-encoded, one per line, so any
# name (spaces, quotes, a newline even) arrives intact (shell.qml's
# shareFiles decodes them). It goes to the shell this script belongs to.
set -uo pipefail
[[ $# -gt 0 ]] || exit 0
SHELL_DIR="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/../.." && pwd)"

# The same Quickshell the shell runs on (iRice's own build when there is one;
# see launch-shell.sh): asking it over IPC needs the same version
QS=qs
for d in "${XDG_DATA_HOME:-}" "$HOME/.local/share"; do
    if [[ -n "$d" && -x "$d/irice/quickshell/bin/qs" && -d "$d/irice/quickshell/lib/qt6/qml/Quickshell" ]]; then
        QS="$d/irice/quickshell/bin/qs"
        export QML_IMPORT_PATH="$d/irice/quickshell/lib/qt6/qml${QML_IMPORT_PATH:+:$QML_IMPORT_PATH}"
        break
    fi
done

uris=$(python3 -I -c '
import sys, urllib.parse, os
out = []
for a in sys.argv[1:]:
    if a.startswith("file://"):
        out.append(a)  # already a URI (Dolphin %U, Nautilus)
    else:
        out.append("file://" + urllib.parse.quote(os.path.abspath(a)))
print("\n".join(out))' "$@") || exit 1

exec "$QS" -p "$SHELL_DIR" ipc call panels shareFiles "$uris"
