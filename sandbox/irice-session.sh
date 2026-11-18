#!/usr/bin/env bash
# The login screen's entry under Tessera's old name
# (/usr/share/wayland-sessions/irice.desktop) starts this file; it starts
# Tessera's, tessera-session.sh, beside it. The root setup (Software Update's
# root step, or tessera-root.py setup) replaces that entry with
# tessera.desktop, which starts tessera-session.sh itself.
# REMOVE AFTER 2027-02-01 (docs/rename-to-tessera.md), with old-names.sh.
exec "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/tessera-session.sh" "$@"
