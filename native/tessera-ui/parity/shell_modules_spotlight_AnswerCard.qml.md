# shell/modules/spotlight/AnswerCard.qml → ui/spotlight.slint `GCard` (Glass answer card)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | 84 px, radius menu+2; selected accentTint + 1 px accent 50 % border; hover fillHover; else fill; colour 150 ms | 22-27 | GCard | same |
| 2 | Tile 52 px r 14 at x 16, kind colour (math orange, date red, time blue, unit/currency green, Dictionary purple, Do accent), 26 px filled white glyph | 13-19, 28-38 | `tone()` | same |
| 3 | Title 24 px 600 (Do 18), tabular figures; under it subtitle · "Enter to do it"/"Enter opens it"/"Enter copies it" (13 px secondary) | 39-58 | `sub_line(.., card)` | same (no tnum feature in Slint) |
| 4 | Click runs it | 59-65 | TouchArea | same |

## Test plan (tester)
Glass; open Search.
1. `wtype 7*6` → orange tile, "42", "Enter copies it"; `-k Return` → `wl-paste` = 42.
2. `10 km in miles` → green tile once the conversion arrives.
3. Hover the card (≈ (960, 322)) with another row selected (`-k Down` first) → fillHover fades in over 150 ms; `-k Up` → accent tint + border.
