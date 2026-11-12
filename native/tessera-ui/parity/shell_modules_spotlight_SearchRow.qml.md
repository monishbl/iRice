# shell/modules/spotlight/SearchRow.qml → ui/spotlight.slint `MRow` (Material rows)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | 56 px segmented item: outer corners 20 (first/last of group), inner 4; selected → fully round on springFast (ω 28.3, ζ 0.60) | 17-30, 37-45 | `round`, `top-r`, `bottom-r` | **was** 300 ms spring(0.45); now 222 ms spring(0.4) |
| 2 | surfaceBright / selected secondaryContainer (150 ms); state layer 8 % hover, 10 % pressed (150 ms), hover covering its buttons | 44-57, 137-138 | background, `hot` | same |
| 3 | Leading 40 px at x 8: circle (surfaceContainerHighest) → cookie9 selected (primary, or primaryContainer behind a picture), 150 ms colour | 60-71 | MShape | same (polar outline near the shader shape) |
| 4 | Icon 26 px unstyled, or picture, or emoji 22 px, or glyph 20 px (filled + onPrimary selected); loading: spinner 32 px | 72-104 | leading | same (Busy arc spinner) |
| 5 | Title 16 px (500 selected / 400), subtitle 14 px onSurfaceVariant ("Get · " dropped on store rows) | 106-128 | texts, `sub_line` | same |
| 6 | Trailing: "Get" (32 px, filled when selected) on store rows; eye on file rows while hovered/selected; a file keeps the 40 px room | 140-169 | trailing | same |
| 7 | Click runs it | 130-136 | TouchArea | same |

## Test plan (tester)
Material; open Search, `wtype fi` (≥ 3 app rows in one group).
1. Shot: group's first row top corners 20, middle 4, last bottom 20; 2 px gaps.
2. `rec sp-row-sel 0.6 "wtype -k Down"` → the new selected row's corners round in ≈ 0.22 s with a slight overshoot; the old one returns. Compare to QML ± 1 frame.
3. Glide over an unselected row → 8 % layer fades in 150 ms; `P down` → 10 %.
4. `/.bash` file rows: eye on hover/selection; the title doesn't shift when it appears.
