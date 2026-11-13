#!/usr/bin/env bash
# The Material look's filled icons for the Rust shell (tessera-ui): Slint can't
# set a variable font's axes, so Icon.qml's `fill` (Material Symbols' FILL 1)
# comes as a fixed instance of material/assets/fonts/MaterialSymbolsRounded.ttf
# (the "Tessera Symbols" cut, tools/gen-material.sh) with FILL at 1, its other
# axes kept, named "Tessera Symbols Fill".
#   material/assets/fonts/MaterialSymbolsRounded-Fill.ttf
# Needs uv (fonttools, from the network once).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FONTS="$ROOT/material/assets/fonts"
uv run -q --no-project --with fonttools python3 -I - "$FONTS/MaterialSymbolsRounded.ttf" "$FONTS/MaterialSymbolsRounded-Fill.ttf" <<'PY'
import sys
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
src, out = sys.argv[1:]
f = instancer.instantiateVariableFont(TTFont(src), {"FILL": 1})
for r in f["name"].names:
    if r.nameID in (1, 3, 4, 6, 16):
        r.string = r.toUnicode().replace("Tessera Symbols", "Tessera Symbols Fill").replace("TesseraSymbols", "TesseraSymbolsFill")
f["name"].names = [r for r in f["name"].names if r.nameID not in (21, 22, 25)]
f.save(out)
print(out)
PY
ls -la "$FONTS/MaterialSymbolsRounded-Fill.ttf"
