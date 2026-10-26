#!/usr/bin/env bash
# Compiles shell/**/*.frag|vert to .qsb next to the source. Runs qsb inside the
# irice distrobox when it isn't available on the host.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
QSB=/usr/lib/qt6/bin/qsb
run() { if [[ -x "$QSB" ]]; then "$@"; else distrobox enter irice -- "$@"; fi; }
find "$ROOT/shell" -name '*.frag' -o -name '*.vert' | while read -r src; do
    if [[ ! -f "$src.qsb" || "$src" -nt "$src.qsb" ]]; then
        run "$QSB" --glsl "300 es,330" --hlsl 50 --msl 12 -o "$src.qsb" "$src"
        echo "compiled ${src#"$ROOT"/}"
    fi
done
