# shell/modules/spotlight/ResultRow.qml → ui/spotlight.slint `GRow` (Glass rows)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | 40 px row, radius control; selected accent, hover fill, else clear (no fade) | 12-16 | GRow background | same |
| 2 | Leading at x 10, 26 px: app icon (styled for Applications / Top Hit), or emoji 20 px, or the screenshot, or a glyph 18 px (onAccent selected / secondary) | 18-63 | GRow leading | same |
| 3 | Loading row: glyph turns steadily, 900 ms a turn, back to 0 when done | 54-62 | `transform-rotation` from animation-tick | same |
| 4 | Title (title size, 500, label/onAccent), subtitle on its baseline (body, tertiary / onAccent 75 %), elided | 64-86 | HorizontalLayout | same |
| 5 | Click runs it | 87-92 | TouchArea | same |
| 6 | File rows: eye button (Quick Preview) at the right while hovered (row or button) or selected | 93-105 | eye `visible` | same |
| 7 | Icons from the user's theme (Colloid) | 18-25 | `Icons::path` + `appicon::render`, cached; arriving icons go into the shown row only | same |

## Test plan (tester)
Glass; open Search, `wtype fi`.
1. Shot: app rows have their Colloid icons at x 10 (26 px). Note any row with an empty icon slot (the "icons not loading" report) and its app.
2. `P glide 960 350 300` over an unselected row → fill at once; glide off → clear.
3. `/.bash`: hover a file row → eye at the right; glide onto the eye (row stays hovered); click → Quick Look.
4. `:smile` → emoji rows, emoji 20 px centred in the leading slot.
5. Click a row → runs (app starts), Search closes.


## Round 2 (after panels-batch2 results)
| # | Finding | Cause | Fix |
|---|---------|-------|-----|
| R1 | Glass app icons small dark monochrome glyphs; QML Colloid colour icons (Material fine: its rows are unstyled) | core `appicon::Style::from_settings` read `iconStyle: "default"` as the look's own style (Clear in Glass = a monochrome glyph). Config.qml renamed the old "default" to "look" once (defaultsVersion 3); a "default" now is the user's pick, each app's own logo (AppIcon.qml mode 0) | "default" → mode 0; only "look" takes the look's style (`core/src/appicon.rs`). Same cause as batch 1's monochrome dock / switcher icons |

Re-test: Glass, `wtype fi` → Thunar's blue folder, Foot's icon in colour (as QML).
