# shell/modules/spotlight/SearchHero.qml → ui/spotlight.slint `MHero`

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | 96 px card r 28 → fully round when selected, springFast | 197-210 | `round` | **was** 300 ms spring(0.45); now 222 ms spring(0.4) |
| 2 | primaryContainer (Top Hit, Do) / tertiaryContainer (answers); state layer 8/10 % (150 ms) | 186-219 | container, layer | same |
| 3 | Leading 64 px: sunny selected, cookie12 behind a picture (surfaceBright), else cookie9 strong colour; icon 40 / glyph 28 filled | 222-246 | MShape | same (polar outlines) |
| 4 | Section label 12 px 500 ×0.8 (not Do; "Top hit"); title 24/18/22; body · hint 14 px ×0.8 | 248-279 | texts | same (no tnum) |
| 5 | Key button 56 px, right 20: round, squares to 16 pressed on springFast, press swell; arrow-up-right / copy / arrow-square-out; store: "Get" filled 40 px | 289-338 | `go` | **was** 250 ms spring(0.4); now 222 ms spring(0.4) |
| 6 | Click (card or button) runs it | 281-287, 330-336 | TouchAreas | same |

## Test plan (tester)
Material; open Search.
1. `wtype 9*9` → tertiary hero "81", "Enter copies it", copy icon; selected: fully round.
2. `-k Down` → corners back to 28 in ≈ 0.22 s; `-k Up` → round. Record, compare with QML.
3. Glide onto the go button (≈ (1272, 328)) → 8 % layer; `P down` → corners 16, slight swell; `P up` → copied, closed.
4. `wtype foot` → Top Hit hero, primaryContainer, foot icon on cookie12, arrow-up-right; Return launches foot.
