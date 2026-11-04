#!/usr/bin/env bash
# iRice is now Tessera: the login screen's entry installed before the rename
# (/usr/share/wayland-sessions/irice.desktop) starts this file. It starts
# Tessera's, tessera-session.sh, beside it. ./install.sh --only=login (or
# tools/setup-session.sh) replaces that entry with tessera.desktop, which
# starts tessera-session.sh itself; then this file is no longer used.
exec "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/tessera-session.sh" "$@"
