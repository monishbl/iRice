#!/usr/bin/env bash
# Builds sandbox test tools inside the irice container (not on the host).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
OUT=../../.sandbox/bin
mkdir -p "$OUT" gen
XML=/usr/share/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml
wayland-scanner client-header "$XML" gen/wlr-virtual-pointer-unstable-v1-client-protocol.h
wayland-scanner private-code "$XML" gen/wlr-virtual-pointer-unstable-v1-protocol.c
gcc -O2 -Wall -Igen -o "$OUT/vptr" vptr.c gen/wlr-virtual-pointer-unstable-v1-protocol.c -lwayland-client -lm
echo "built $OUT/vptr"
