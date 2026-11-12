# shell/modules/shelf/ShelfWindow.qml → surfaces/shelf (src/lib.rs `push_card`, `show_card`, `drag_card`; src/logic.rs `card_height`, `card_top`; ui/shelf.slint `ShelfCard`, `ShelfButton`)

springPress 7.5/0.3 → ω 21.7, ζ 0.43 → 290 ms spring(0.57). Glass ElasticControl squish: spring 6, damping 0.32 + 0.05 → ω 19.4, ζ 0.60 → 325 ms spring(0.4).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | While Shelf.opened (+140 ms for the fade); overlay, no keyboard, no exclusive zone; on Shelf.screen | 12-38 | `show_card`, `hide_card`, `card_gone` | same |
| 2 | At the side's edge, 8 px in (shadow room hangs off); top 40, or centred on the drag's y kept on screen | 39-44 | `push_card` anchors/margins, `card_top` | same (the window spans the screen's height; card moved inside it, input region on the card only) |
| 3 | Glass glass r 20 (Material 24), bevel 16, its opacity the card's | 47-53 | `after_frame` glass | same (Material: own tonal card with a drop shadow) |
| 4 | Card 240 wide, body + 24; opacity 200 ms OutCubic in / 130 ms InCubic out; slides 6 px toward its edge 200 ms OutCubic; there at once when the loader makes it | 55-63 | `fade`, `slide`, `instant` | same |
| 5 | Hover or a drag over it → Shelf.hovered | 65-66 | `hover` TouchArea, `drag_over`, `say_hovered` | same |
| 6 | Title "Shelf" (Material title 500, Glass body 600), count small secondary; buttons Share (with paths), Clear (with items), Close | 76-114 | heading, `ShelfButton` | same (no tooltips) |
| 7 | IconButton: Glass fill on hover / fillPressed; Material round, 8 %/10 % layer, radius → 8 pressed (150 ms OutCubic); colour Theme.motion.hover | IconButton.qml 19-23 | `ShelfButton` | **was** colour 150 ms hard-coded; now `Theme.hover` |
| 8 | IconButton press (ElasticControl): Material swell 1.06 on springPress; Glass squish 0.95 on a spring with a small rubber-band pull | ElasticControl.qml 32-38 | `ShelfButton.transform-scale` | **was** no press scale at all; now Material 1.06 / 290 ms spring(0.57), Glass 0.95 / 325 ms spring(0.4) (no rubber band; the squish is the default motion's, Snappy 0.97 / Expressive 0.9 aren't followed) |
| 9 | Empty: 132 (Material 160) r 14/16; Glass 1 px separator border (accent + 12 % fill under a drag); Material dashed outline 4/3 (primary 2 px under a drag), cookie9 → clover8 shape; "Drop files here", help | 116-195 | empty block, `logic::dashes` | same (dashes as segments; shape switch, no morph) |
| 10 | "All n items" row when more than one | 197-208 | `all_row` | same |
| 11 | List: rows 44 (Material 52), at most 6 shown, scrolls past that; each row's glyph, name (text: end-elided; files: middle), detail (~ for home) | 210-237 | `rows`, `logic::glyph_name`, `detail`, `elide_middle` | scroll by the wheel only (a press and move drags the row out) |
| 12 | The list keeps its delegates while it can (a thumbnail arriving, a drop) | 220 (model) | `CardWin.rows: Rows` (`set`) | **was** a new model on every push (each thumbnail decoded rebuilt every row, hover and × lost); now set in place |
| 13 | Drop anywhere on the card (not its own rows coming back) → Shelf.drop | 242-256 | `drag_card`, `would_add` | same |
| 14 | Accent outline 2 px + 8 % fill while a drag is over it with items there | 258-265 | overlay | same |

## Test plan (tester)
Arena 1920×1080, sandbox. Shelf on the right: card x 1672..1912, top y 40. Title buttons centre y 66: Close (1886, 66), Clear (1856, 66), Share (1826, 66). Glass rows: "All" y 88..132, list row i centre y 162 + 44·i; Material: "All" 88..140, row i centre 174 + 52·i. A row's × at x ≈ 1882.
1. **Open/close**: `rec shf-open 0.4 "ipc shelf open"` → there at once (empty state). `rec shf-close 0.4 "ipc shelf close"` → fades 130 ms while sliding 6 px right. QML vs Rust.
2. **Button press** (fix 8): `P move 1886 66`, `rec shf-btn 0.4 "P down"` → Material: grows to 1.06, corners → 8; Glass: shrinks to 0.95 with a slight spring; `P glide 1700 600 100`, `P up` (off it: nothing). Hover alone: fill fades in 150 ms (fix 7).
3. **Items**: drag 3 files onto the card (a file manager in the arena, `P down` on a file, `P glide 1790 150 600`, `P up`); if the arena has no drag source, mark this and steps 4–6 not testable here. Expect "All 3 items" + 3 rows; pictures show their thumbnails a moment later.
4. **Rows kept** (fix 12): hover row 2 (`P glide 1790 206 200`, Glass) so its × shows; drop one more picture on the card → row 2 keeps its hover and × while the new row's thumbnail arrives (before: every row was made again and the × blinked).
5. **Drag out**: `P move 1790 162`, `P down`, `P glide 900 500 600`, `P up` over a foot window → it receives the path; the row stays on the shelf.
6. Remove: hover a row, `P click 1882 <row y>` → it goes. Clear: `P click 1856 66` → empty state.
7. Drag over with items: an outside drag over the card → 2 px accent outline + 8 % fill.
