# shell/modules/clipboard/ClipboardPanel.qml → surfaces/clipboard (ui/clipboard.slint, src/lib.rs)

Also covers what it takes from shell/glass/Dialog.qml, shell/common/Segmented.qml and shell/common/ScrollIndicator.qml.
Spring conversion (Qt SpringAnimation steps 16 ms): ω = √(62.5·spring), ζ = 62.5·damping / (2ω); Slint `spring(bounce)` with duration d: d = 2π/ω, bounce = 1 − ζ.
Dialog opens on Theme.motion.spring + 0.4 / damping + 0.08: Material 6.48 / 0.58 → ω 20.1, ζ 0.90 → 312 ms spring(0.1); Glass (motion "subtle") 4.2 / 0.40 → ω 16.2, ζ 0.77 → 388 ms spring(0.23).
springFast 12.8 / 0.54 → 222 ms spring(0.4); springPress 7.5 / 0.3 → ω 21.7, ζ 0.43 → 290 ms spring(0.57).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Opens on Dialog's spring (Material ζ 0.90 ≈ 0.3 s, Glass ζ 0.77 ≈ 0.4 s, ~3 % overshoot); closes in panel × 0.6 = 192 ms on the look's exit curve (Material 0.3,0,0.8,0.15; Glass 0.4,0,1,1) | Dialog 20-42 | `animate openness` | **was** 420 ms spring(0.15) in both looks, Glass's exit curve in Material; now 312 ms spring(0.1) / 388 ms spring(0.23), exit curve per look |
| 2 | Scrim black 25 % × progress; a click on it closes | Dialog 58-66 | scrim Rectangle + TouchArea | same |
| 3 | Body 592×512 (560×480 + 16 padding) centred, 40 px above centre; opacity progress, scale 0.96→1 from its centre; radius 28 | Dialog 68-110, 30-32 | `body` | same (no ElasticGrab stretch: known difference, Slint has no drag-stretch here) |
| 4 | Search: 36 px, Material pill surfaceContainerHigh / Glass fill + 1 px accent ring when focused; magnifier x 14/10; text body+1; "Search clipboard" | 129-178 | `SearchField` (core) | same |
| 5 | Keys: Esc closes, Tab next kind, Up/Down move, Left/Right only in Images, Enter pastes / Shift copies / Alt plain, Ctrl+P pin, Delete remove | 154-169, Dialog 78-79 | `handles`, `key` | same |
| 6 | All/Text/Images: 210 wide, Glass 30 px track with a sliding pill (popover, emphasized 0.2,0.9,0.1,1) | 180-188, Segmented 89-100 | `seg` | **was** 220 ms cubic-bezier(0.2,0,0,1); now Theme.popover on (0.2,0.9,0.1,1.0) |
| 7 | Material segments: 36 px, 2 apart, outer ends round, inner corners 8 (4 pressed), chosen one fully round; corners on springFast; 8 %/10 % state layers | Segmented 116-145 | `seg` Material Rectangle | **was** corners jumped; now `inner` animates 222 ms spring(0.4) |
| 8 | Material press: the pressed segment widens min(14, 18 %) and its neighbours give way, on springPress, bouncing back on release | Segmented 54-77 | `seg.squeeze`, `w0..w2` | **fixed** (was missing) |
| 9 | Section headings: Material primary 12/500 x 16, 16 above / 8 below; Glass tertiary caption 600 x 10, 12 / 4; none above the first | 191-199 | `layout`, heading Text | same |
| 10 | List rows 44 px, pictures 168, 2 apart | 215-233 | `layout` | same |
| 11 | Material: segmented list, each section a group (16 outer corners, 4 between); the chosen row rounds fully in secondaryContainer; corners on springFast, colour on hover (150 ms) | 239-256 | Material row Rectangle | **was** corners jumped; now `top`/`bottom` animate 222 ms spring(0.4); colour on Theme.hover |
| 12 | Glass: accent 16 % (dark 32 %) behind the chosen row, fill on hover | 234 | Glass row Rectangle | same |
| 13 | Hovering a row/tile chooses it; the ListView follows its currentIndex (grid: reveal) so a half-hidden row scrolls into view | 209, 258-264, 385, 397 | `M::Hovered` → `select` | **was** chosen without scrolling; now revealed as by keys |
| 14 | Colour swatch (16, r 4) / link or text icon (16, x 10) / text x 40 elided / picture 152 tall, own aspect (96 … w−120), r 12/10 | 266-309 | row contents | same |
| 15 | Row actions: pin (filled + accent when pinned) shown when chosen or pinned; trash when chosen | 311-327 | `actions` | same (no tooltips) |
| 16 | Images: two columns (cell ⌊(w−8)/2⌋ × 0.72), 8 apart; chosen tile 3/2 px accent ring; pin/remove chip (Material primaryContainer pill, Glass dark 72 %) | 333-431 | `layout(grid)`, tile | same |
| 17 | Pictures load off the UI thread (pinned ones always, others when the preview is ready) | 440-474 | Previews service, `keep_images` | same (the first screen waits ≤ 250 ms for its pictures so they come in together) |
| 18 | Scroll indicator: 12 px strip at the list's right edge (6 px past it); thumb 6 px (8 on hover/drag), ≥ 28 tall, black/white 32 % (50 % dragged); shown while scrolling (900 ms), hovered or dragged, fades in 150 ms / out 400 ms; click on track jumps, thumb drags | 433-436, ScrollIndicator.qml | `sb` + `scrolling` (Rust 900 ms timer) | **fixed** (was missing) |
| 19 | Empty / no results: Glass 32 px icon; Material 72 px cookie9 / clover4 shape; title (Material large 400, Glass title 600), body; "No matches for “…”" etc. | 476-522 | `count == 0` layout | same (Material shape is an outline from shapes.rs) |
| 20 | Material floating toolbar 64 px over the list's foot, 12 above the footer: Paste, Plain text (not for pictures), Copy, Pin, Delete (error) | 527-542 | `toolbar` | same (no tooltips) |
| 21 | Footer: key hints 14 apart; Clear history (danger text) → "Clear N items" filled on first click, cancels when the pointer leaves | 544-602 | `footer` | same |
| 22 | The list is refreshed in place (a ListView over the rows) | 202-213 | `Win.items: Rows<CItem>` | **was** a new model per refresh (every row rebuilt, hover lost, pictures re-set); now one `Rows`, set in place |
| 23 | Window made on open, dropped 600 ms after closing | PanelLoader | `RETIRE_SECS` 10 s | known difference (Rust keeps it longer; both are made on open, so no ahead-of-time window) |
| 24 | Touchpad momentum (Momentum.qml) | 204, 335 | Slint Flickable | known difference (Slint's own flick) |

## Test plan (tester)
Arena 1920×1080. Dialog body x 664..1256, y 244..756; content x 680..1240, y 260..740. Search field x 680..1018, y 260..296. Segments (centre y 278): All ≈ (1067, 278), Text ≈ (1135, 278), Images ≈ (1205, 278). List x 680..1240, y 308..704. First row: Glass y 326..370 (centre 348), Material y 332..376 (centre 354); next rows +46. Scroll indicator x 1234..1246. Material toolbar y 636..700, centred x 960. Footer y 712..740; Clear history ≈ (1195, 726).
Seed the history with ≥ 12 text entries and 3 pictures first (`printf 'item %s' N | wl-copy` per entry; a PNG via `wl-copy < pic.png`); mark not testable here if cliphist is missing in the arena.
1. **Open, Material**: `rec cb-m-open 1 "ipc panels open clipboard TESSERA-TEST"`; track the body's top edge/scale: settles in ≈ 0.3 s, overshoot ≤ 1 % (before: ≈ 0.42 s). `wtype -k Escape` while recording `rec cb-m-close 0.4 …` → gone in 192 ms, slow start (exit curve 0.3,0,0.8,0.15).
2. **Open, Glass**: same, switch look first; settles in ≈ 0.4 s with ~3 % overshoot; close curve (0.4,0,1,1). Compare QML vs Rust frame strips (± 1 frame).
3. **Segments, Glass**: `P click 1135 278` → pill slides to Text in 220 ms, fast start (emphasized); compare frames.
4. **Segments, Material**: `P glide 1135 278 200`, `P down`, `rec cb-m-seg 0.6 ""`, `P up`: Text widens ≈ 10 px while held (neighbours shrink), corners of the pressed segment go 8→4, then on release it springs back past its width once and settles (≈ 0.3 s); chosen segment's corners round on a ≈ 0.22 s spring.
5. **Rows, Material**: `wtype -k Down` ×2 → the chosen row's corners round (16) on a ≈ 0.22 s spring, the previous one squares back; colour fades in 150 ms. `rec cb-m-rows 0.5 "wtype -k Down"`.
6. **Hover reveals**: scroll so the last visible row is cut by the list's bottom (y ≈ 700), `P glide 900 698 200` → that row is chosen and the list scrolls just enough to show it (Material: above the toolbar's 76 px).
7. **Scroll indicator**: with more rows than fit, scroll the wheel over (900, 500) → thin bar at x ≈ 1240 appears, fades out ≈ 0.9 s after scrolling stops (400 ms fade). `P glide 1240 400 200` → bar widens to 8 px; `P down` on the thumb, `P glide 1240 600 300`, `P up` → list follows; `P click 1240 690` → jumps toward the end.
8. **Rows kept**: hover row 2 (`P glide 900 394 100`), then copy a new item from outside (`printf new | wl-copy`) while open → the list updates without a flash and row hover stays.
9. **Images grid**: `wtype -k Tab` ×2 → two columns of 276×199; arrows move across/down; chosen tile has a 3 px (Material) / 2 px (Glass) accent ring and the pin/trash chip at its top-right.
10. **Clear history**: `P click 1195 726` → becomes "Clear N items" filled red; `P glide 900 600 100` → back to "Clear history". (Don't click twice: it wipes the arena history.)
