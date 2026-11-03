#!/usr/bin/env bash
# Compiles shell/**/*.frag|vert and material/**/*.frag|vert (the Material
# look) to .qsb next to the source. Runs qsb inside the
# irice distrobox when it isn't available on the host.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
QSB=/usr/lib/qt6/bin/qsb
run() { if [[ -x "$QSB" ]]; then "$@"; else distrobox enter irice -- "$@"; fi; }
# (a build whose source is gone, say a Material shader merged into shell/,
# would still be linked into the Material root over the shared one)
find "$ROOT/shell" "$ROOT/material" -name '*.qsb' 2>/dev/null | while read -r built; do
    if [[ ! -f "${built%.qsb}" ]]; then
        rm -f "$built"
        echo "removed ${built#"$ROOT"/} (its source is gone)"
    fi
done
find "$ROOT/shell" "$ROOT/material" \( -name '*.frag' -o -name '*.vert' \) 2>/dev/null | while read -r src; do
    if [[ ! -f "$src.qsb" || "$src" -nt "$src.qsb" ]]; then
        run "$QSB" --glsl "300 es,330" --hlsl 50 --msl 12 -o "$src.qsb" "$src"
        echo "compiled ${src#"$ROOT"/}"
    fi
done
