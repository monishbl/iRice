#!/usr/bin/env bash
# Whether a hypridle.conf is your own (default: ~/.config/hypr/hypridle.conf):
# exit 0 when it is, 1 when there's none or it's a copy an earlier iRice
# installer put there, unchanged. Your own wins over Settings → Battery &
# Power (start-idle.sh, services/Power.qml); an installer's copy doesn't, so
# installs from before the installer stopped copying it get the settings back.
#   own-hypridle.sh [file]
set -uo pipefail
f="${1:-${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hypridle.conf}"
[[ -r "$f" && -f "$f" ]] || exit 1
# (sha256 of each hypr/hypridle.conf iRice ever shipped)
case "$(sha256sum <"$f" | cut -d' ' -f1)" in
    cd5a63398fc723190008d71dbc0793c5e53d2910a4684635017b9812642e41a1 | \
        6bc086d7655b96b2e0803644fded9ca16570b30ae9b570e833a4359f8a6dee10 | \
        c2df50e6c796d7b3f2cccd5fd3a206d222c4f48a86e3f9be6c65a0cecf1d79d0) exit 1 ;;
esac
exit 0
