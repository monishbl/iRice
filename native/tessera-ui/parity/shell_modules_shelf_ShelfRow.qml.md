# shell/modules/shelf/ShelfRow.qml → ui/shelf.slint `ShelfRow` (rows from src/lib.rs `push_card`)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Background: Glass r 10, fill on hover, fillPressed pressed; Material segmented (outer 16, inner 4, 1 px in from a neighbour), surfaceContainerHigh + 8 %/10 % layer; no colour animation | 293-303 | first Rectangle | same |
| 2 | Thumb 32 (Material 36) at x 6 (8): Glass r 6 fill; Material picture r 8, glyph on a secondaryContainer circle; picture async, 64 px, crop; else glyph 18 | 305-330 | `thumb`, thumbnail thread (64 px) | same (an undecodable picture shows the file glyph) |
| 3 | Name: Material 14, Glass body; files elided in the middle, text at the end; detail caption secondary, elided in the middle | 332-349 | texts, `elide_middle` in Rust | same (middle elision by character count) |
| 4 | Dragged out (Qt's start distance) as a native drag of Shelf.mimeFor(entries) (uri-list + text); it stays on the shelf | 351-372 | `grab` (10 px) → `drag-out` → `State::start_drag`, `logic::mime_for` | same |
| 5 | Open-hand cursor | 371 | `grab` / `grabbing` | same (closed hand while pressed: Slint's) |
| 6 | × (IconButton 13 px) on hover of removable rows, 4 px from the right; removes | 374-382 | `close` ShelfButton | same; its press now springs as IconButton's (see ShelfWindow #8) |
| 7 | Row elements kept while the list changes | (ListView delegates) | `Rows` | **was** rebuilt on every push; now in place (ShelfWindow #12) |

## Test plan (tester)
Arena 1920×1080, sandbox, shelf with ≥ 2 items (ShelfWindow step 3; not testable without a drag source). Rows as in ShelfWindow's plan.
1. Shots of row 1 and the last row, both looks: Material corners 16 outside / 4 inside, 2 px gap; Glass r 10 hover fill.
2. `P glide 1790 162 200` → hover fill and × appear together; `P down` → pressed fill; `P glide 1790 172 50` (< 10 px) → no drag; `P glide 1790 200 50` → drag starts; `P up` on the desktop → row still there.
3. A long file name: the middle is cut ("holiday-ph…2026.jpg"), the extension kept; a text item: cut at the end.
