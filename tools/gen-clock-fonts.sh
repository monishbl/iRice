#!/usr/bin/env bash
# The Material lock's clock digits for the Rust shell (tessera-ui): Slint can't
# set a variable font's axes, so Google Sans Flex's width and roundness (the
# lock's clockWidth and clockRound, LockClock.qml's wdth and ROND) come as
# fixed instances of material/assets/fonts/GoogleSansFlex.ttf, one family per
# width and roundness ("Tessera Clock 25", "Tessera Clock 25 Round", …), at
# the optical size of a big clock (opsz 144), the weight axis kept. Cut down to
# Latin-1, with the tabular figures as the digits (the QML's "tnum").
#   material/assets/fonts/clock/TesseraClock-<wdth>[-Round].ttf
# Needs uv (fonttools, from the network once).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/material/assets/fonts/GoogleSansFlex.ttf"
OUT="$ROOT/material/assets/fonts/clock"
mkdir -p "$OUT"
uv run -q --no-project --with fonttools python3 -I - "$SRC" "$OUT" <<'PY'
import io, sys
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

src, out = sys.argv[1:]
for wdth in (25, 50, 75, 100, 125, 151):
    for rond in (0, 100):
        f = TTFont(src)
        f = instancer.instantiateVariableFont(f, {"wdth": wdth, "ROND": rond, "opsz": 144, "slnt": 0})
        b = io.BytesIO()
        f.save(b)
        b.seek(0)
        f = TTFont(b)
        # the tabular figures as the plain digits
        cmap = f.getBestCmap()
        gsub = f["GSUB"].table
        tnum = {}
        for fr in gsub.FeatureList.FeatureRecord:
            if fr.FeatureTag == "tnum":
                for li in fr.Feature.LookupListIndex:
                    for st in gsub.LookupList.Lookup[li].SubTable:
                        st = getattr(st, "ExtSubTable", st)
                        tnum.update(getattr(st, "mapping", {}) or {})
        for cp in range(0x30, 0x3A):
            g = cmap.get(cp)
            if g in tnum:
                for t in f["cmap"].tables:
                    if t.isUnicode() and cp in t.cmap:
                        t.cmap[cp] = tnum[g]
        opts = subset.Options()
        opts.layout_features = ["kern", "liga", "calt", "ccmp", "locl", "mark", "mkmk"]
        opts.name_IDs = ["*"]
        opts.notdef_outline = True
        opts.hinting = False
        s = subset.Subsetter(opts)
        s.populate(unicodes=list(range(0x20, 0x7F)) + list(range(0xA0, 0x100)))
        s.subset(f)
        family = "Tessera Clock %d%s" % (wdth, " Round" if rond else "")
        ps = family.replace(" ", "")
        for r in f["name"].names:
            if r.nameID in (1, 16):
                r.string = family
            elif r.nameID == 4:
                r.string = family + " Regular"
            elif r.nameID in (3, 6):
                r.string = ps + "-Regular"
        f["name"].names = [r for r in f["name"].names if r.nameID not in (21, 22, 25)]
        name = "TesseraClock-%d%s.ttf" % (wdth, "-Round" if rond else "")
        f.save(out + "/" + name)
        print(name)
PY
ls -la "$OUT"
