#!/usr/bin/env bash
# Restarts the sandbox Quickshell. Only touches sandbox processes (see lib.sh).
# IRICE_LAB=1 also loads the glass tuning bench.
set -euo pipefail
ROOT="${IRICE_ROOT:-/home/$(id -un)/Projects/Personal/iRice}"
[[ -f "$ROOT/shell/shell.qml" ]] || ROOT="$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")"
ROOT="${ROOT#/run/host}"
source "$ROOT/sandbox/lib.sh"
export XDG_RUNTIME_DIR="$IRICE_RT"
export IRICE_LAB="${IRICE_LAB:-0}" IRICE_SANDBOX=1
[[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" && -f "$IRICE_RT/session-bus" ]] && export DBUS_SESSION_BUS_ADDRESS="$(cat "$IRICE_RT/session-bus")"

stop_sandbox_shells
setsid -f "$ROOT/shell/assets/scripts/launch-shell.sh" -p "$ROOT/shell" >"$XDG_RUNTIME_DIR/quickshell.log" 2>&1
