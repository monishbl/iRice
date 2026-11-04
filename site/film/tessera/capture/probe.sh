#!/usr/bin/env bash
# probe.sh <g|m>: stills of each surface, to read positions from (not used in the film)
. "$(dirname "$0")/lib.sh"
L=$1
inbox 'ipc panels open controlCenter IRICE-TEST; sleep 1.5'; still probe-$L-cc
inbox 'ipc panels close; sleep 0.5; ipc panels open sidebar IRICE-TEST; sleep 1.5'; still probe-$L-sidebar
inbox "ipc panels close; sleep 0.5; ipc panels quicklook $CAP/seed/menu.png; sleep 2"; still probe-$L-ql
inbox 'ipc panels close; sleep 0.5; ipc sharingdemo ask; sleep 1.5'; still probe-$L-share
inbox 'ipc island demo parts; sleep 1.2'; still probe-$L-island
inbox 'sleep 4; ipc panels close'
