# shell/modules/screentools/ScreenToolbar.qml → surfaces/screentools (src/toolbar.rs, ui/screentools.slint `ScreenToolbar`)

Spring conversion used here (Qt SpringAnimation steps 16 ms): ω = √(62.5·spring), ζ = 62.5·damping / (2ω);
Slint `spring(bounce)` with duration d has ω = 2π/d, ζ = 1 − bounce.
springFast 12.8/0.54 → ω 28.3, ζ 0.60 → 222 ms spring(0.4). springPress 7.5/0.3 (ElasticControl, SplitButton) → ω 21.7, ζ 0.43 → 290 ms spring(0.57).
Qt.darker(c, f) divides the HSV value by f; Slint `darker(x)` divides by 1 + x, so Qt.darker(c, 1.2) = `darker(0.2)`.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Opens on ⇧⌘5 (all tools) or Control Center → Record / `recorder toolbar` (recording only); Spotlight's "Screen Recording Options" opens with Options showing | 128-147, 158-177 | `open`, `prepare` | same |
| 2 | Window ready when it opens | 115-120 (PanelLoader) | `make_ahead` (M::Prepare, 1.5 s after start) | **was** made on the first open (first ⇧⌘5 late by the window's creation); now made ahead on the focused output, as the spotlight's |
| 3 | Fade + rise 12 px, popover (220 ms × pace), emphasized in / exit out, per look | 139-150, 192-195 | `animate progress` | same |
| 4 | Bar: row + 24 wide, 64 high, above the dock (iconSize + 34, or 14 without a dock); Glass r 22, Material a primaryContainer pill | 188-189, 356-377 | `bar` | same |
| 5 | Tools 104 wide, separators 17 (a 1 × 32/36 line), Options label + 40; spacing 4 | 379-399 | `for t in tools` | same |
| 6 | The tools list | 384-385 (Repeater) | `tool_rows: Rows` | **was** a new model on every refresh (every tool rebuilt, hover/press lost on a recording or setting change); now one Rows set in place |
| 7 | Tool face: Glass r 12, fill on hover, fillPressed pressed / Options open, picked accent 22 %; Material round, 16 pressed, 20 picked, state layers of onPrimaryContainer | 400-416 | `face` | same |
| 8 | Material face radius on MaterialSpring "fast" | 410-414 | `animate border-radius` | **was** 250 ms spring(0.55); now 222 ms spring(0.4) |
| 9 | Material press: ElasticControl swells 1 + min(6 %, 10 px / size) on springPress; Glass: no transform | 408-409 | `transform-scale` | **was** 1.04 on 300 ms spring(0.35); now 1 + min(0.06, 10 px / size) (1.06 for 104-wide tools) on 290 ms spring(0.57) |
| 10 | Icon 22 (filled when picked / recording), label caption 500; the record glyph red while recording; "Stop Recording" hides Record Window / Selection | 172-174, 417-435 | `geometry::tools`, `StIcon` | same (Material Symbols draw without the FILL axis) |
| 11 | Options: label body 500, caret up/down 11 bold | 436-454 | `t.kind == 2` | same |
| 12 | Glass Record: 36 high pill in danger; hover Qt.darker 1.08, pressed 1.2, 97 % on 100 ms; unavailable: fill + tertiary | 486-510 | `rt` | **was** darker(0.07) / darker(0.17); now darker(0.08) / darker(0.2) |
| 13 | Material Record: SplitButton centred in a slot 16 wider (8 each side) | 476-484, 513 | `split` | **was** x 16 (16 on the left, 0 on the right: 8 px right of the QML's); now x 8 |
| 14 | Split lead: 40 high, outer r 20, inner 4 (8 pressed), state layers, colour animated (hover); ElasticControl swell | SplitButton.qml 46-58 | `lead` | **was** no press swell; now 1 + min(6 %, 10 px) on 290 ms spring(0.57) |
| 15 | Split trail: 40 wide, inner r 4 / 8 pressed / 20 open on springPress; colour animated; swell; caret-down turned 180° (up) closed, 360° open, popover OutCubic | SplitButton.qml 106-129 | trail Rectangle | **was** 300 ms spring(0.3), no colour animation, no swell, the caret swapped glyphs without turning; now 290 ms spring(0.57), Theme.hover colour, swell, `StIcon.turn` 180° → 360° on popover cubic-bezier(0.33, 1, 0.68, 1) |
| 16 | Split menu: 200 wide (max(200, implicit)), r 16, surfaceContainer, shadow 0/4/16; right edge on the split button's, 8 above it; 6 px slide + opacity (popover in, hover out); rows 44, icon 20, 12 gap, 14 px text; a row records that area | SplitButton.qml 148-218 | `menu` | **was** 220 wide, right edge 12 from the bar's end, 8 above the bar (12 px higher than the button's top); now 200 wide, x = bar end − 20 − w (≥ 8), y = bar.y + 12 − 8 − h |
| 17 | A click outside the menu closes it; Esc closes the menu, then Options, then the toolbar; Return records (recording only) | 201-202; SplitButton.qml 140-146, 217 | `on_key`, TouchArea | same |
| 18 | Options above the bar (8 gap): 340 wide, list 14 in, spacing 2; Caption 8 in / 2 / 4, caption 600 tertiary; Gap 6 | 205-227, 19-31 | `opts` | same |
| 19 | Microphones: None, Default microphone, then each; checked row in Material tertiaryContainer; check 14 bold | 228-236, 32-67 | `mic_rows`, `OptionRow` | **was** a new model on each refresh, and the check a Phosphor glyph in both looks; now Rows in place, and the check the look's icon (a Material Symbol in Material) |
| 20 | System audio; Show mouse pointer (gsr); Countdown None / 3 s / 5 s; Frame rate; Format (gsr, 192) + its note; Quality (gsr, 232) | 238-293 | SwitchRow / SegmentRow | same |
| 21 | "Save to ~/…" (middle elide), reset (custom folder only) / choose / open, 32 px buttons; the toolbar closes for the chooser | 295-340 | folder row | same (no tooltips, accessible labels instead; Slint elides at the end) |
| 22 | Without gpu-screen-recorder: the install note | 341-351 | `Note` | same |
| 23 | Input only over the bar and Options while shown; keyboard Exclusive while shown | 181-186 | `after_frame` region | same |
| 24 | Icons follow the look | (Icon per look) | `icons()` again on `config(look)` | **was** set once when the window was made (a look change kept the old look's glyphs; matters now that the window is made ahead); now set again on a look change |

Known differences: the options surface is the shell's Glass in both looks in the QML; Rust draws Material's as surfaceContainer with a shadow. Squircle corners are drawn as round corners.

## Test plan (tester)
Arena 1920×1080, output IRICE-TEST, dock on with 48 px icons (bar bottom margin 82 → bar y 934..998, centre y 966). Both shells (`--only qml`, `--only rust`), both looks.
All tools (not recording): bar ≈ 1041 wide, x ≈ 440. Tool centres x ≈ 504 (Capture Screen), 612, 741 (Copy Text), 849, 957, 1086 (Record Screen), 1194, 1302, Options ≈ 1424.
1. **First open is immediate** (row 2): start the shell, wait 3 s, `rec tb-first 1 "inbox 'ipc panels open screenTools IRICE-TEST'"`. The first frame with the bar: Rust no later than QML (± 1 frame). `wtype -k Escape` closes.
2. **Open/close curve** (row 3): `rec tb-open 1 "inbox 'ipc panels open screenTools IRICE-TEST'"` → rises 12 px and fades in over 220 ms; `rec tb-close 0.5 "wtype -k Escape"` → 220 ms exit; compare frame strips QML vs Rust.
3. **Hover / press** (rows 7-9): open; `P glide 741 966 300`; `shot tb-hover` (Glass fill / Material 8 % layer). Material: `rec tb-press 0.7 "P down; sleep 0.25; P glide 741 700 1; P up"` (released away so nothing runs) → the face swells to ≈ 110 px wide, corners 28 → 16, springs back past 100 % once and settles ≈ 0.3 s; QML and Rust curves within ± 1 frame.
4. **Options** (rows 18-20): `P click 1424 966` → Options open above the bar (bottom edge y ≈ 926), caret flips. Click "Default microphone" → the check moves (Material: tertiaryContainer, Material Symbol check). Toggle System audio while the pointer rests on a microphone row: that row keeps its hover (rows kept in place). Esc closes Options, Esc again the toolbar.
5. **Recording only, Glass** (rows 12, 17): `inbox 'ipc recorder toolbar'` → Entire Screen / Window / Selected Region | Options, Record (bar ≈ 553 wide, x ≈ 683). Click Window (≈ (843, 966)) → picked (accent 22 %). Hover Record (≈ (1176, 966)), `shot rec-hover`: its red matches QML (± 2 per channel); `P down`, `shot rec-press` (÷1.2 darker, 97 %). Esc.
6. **Recording only, Material split button** (rows 13-16): `--look material`, `inbox 'ipc recorder toolbar'` (bar ≈ 627 wide, x ≈ 646). Lead ≈ x 1101..1207, trail centre ≈ (1229, 966). `shot split` → the lead's left edge at the same x as QML (± 1). `rec split-menu 0.6 "P click 1229 966"` → caret turns half a turn over ≈ 220 ms, the menu fades and slides 6 px: 200 wide, right edge x ≈ 1253, bottom y ≈ 938; the trail's inner corners round to 20 with a small overshoot. Esc closes the menu, Esc the toolbar. Starting a recording is **not testable here** (no gpu-screen-recorder in the arena): just check the Rust log asks `screen.do ["record","window"]` after "Record a window".
7. **Look change** (row 24): open and close once in Glass, switch the look to Material in Settings → Appearance, open again → Material Symbols everywhere (the Options check too).
