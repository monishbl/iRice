# shell/modules/spotlight/SpotlightGlass.qml → surfaces/spotlight (Glass drawing in ui/spotlight.slint; glass shapes in `after_frame`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Full-screen overlay, exclusive keyboard while shown, input only while shown | 45-53 | `open`/`close` | same |
| 2 | Progress: popover (220 ms), emphasized bezier in, exit bezier out | 34-43 | `animate openness` (now `Theme.popover`) | same |
| 3 | Column at y = round(h·0.2) − 10·(1−progress), opacity progress, 680 wide centred | 69-80 | `g-x`, `g-top`, `col` | same |
| 4 | Field glass 680×56 r 28; results glass 8 below, r 24, ≤ 440 + 16 high; glass follows the slide | 81-88, 181-190 | `after_frame` shapes | same |
| 5 | Magnifier 22 px bold secondary at x 20; field 22 px; placeholder "Search" tertiary | 90-139 | field | same |
| 6 | Clear (x-circle filled, 36 px, while text) and Ask (sparkle) at the end, tertiary → secondary on hover/focus; Ask filled + accent ink while answering | 141-179 | SIconButton ×2 | same (no tooltips) |
| 7 | Tab: next result if > 1, else Clear (or Ask); Shift+Tab: Ask, Clear, field; Space/Enter presses a focused button | 119-130, 159-176 | `glass_tab`, focus ring | same |
| 8 | Click outside closes; clicks on the panels don't | 63-67, 191-192 | TouchArea → `dismissed` | same |
| 9 | First row an AnswerCard for Calculator/Answer/Conversion/Dictionary/Do (6 px after); section heading (caption 600 tertiary, left 10, top 2/10, bottom 4) before each section | 207-242 | `layout::glass` | same |
| 10 | Selected row kept in view | 204-206 | `reveal` | same |
| 11 | Inline AI answer: sparkle pulsing 0.35↔1 (600 ms each) while thinking, "Thinking…"/model name, answer ≤ 360 scrollable, "Continue in chat" (accent) / "Copy answer" + "↵ to continue in chat" | 246-352 | answer panel | same (Markdown via StyledText: no headings/tables) |

## Test plan (tester)
Glass look. Geometry 1920×1080: field x 620..1300, y 216..272 (centre (960, 244)); Clear ≈ (1252, 244), Ask ≈ (1280, 244) (Ask alone ≈ (1272, 244)); results from y 280, rows 40 px, first row centre ≈ (960, 310) under a heading.
1. `rec sp-g-open 1 "ipc panels open spotlight TESSERA-TEST"`: 220 ms, slides 10 px down while fading; `-k Escape` closes in 220 ms (exit curve). QML/Rust frame counts ±1.
2. `wtype foot` → results glass below; heading in caption type; selected row accent-filled.
3. `P glide 1280 244 300` → Ask turns secondary; glide to Clear → same; click Clear → field empty, typing goes to the field.
4. Keys: with ≥ 2 results `-k Tab` moves the selection; with 1 result `-k Tab` → Clear ringed, again → Ask, again → field. `-M shift -k Tab -m shift` from the field → Ask, → Clear, → field. `-k space` on Clear clears.
5. `P click 960 900` → closes; reopen, `P click 960 244` → stays open.
6. `wtype 12*3` → answer card (orange 52 px tile, "36", "Enter copies it").
