# shell/modules/screentools/editor/ShotThumbnail.qml → surfaces/screentools (`ShotThumbnail` in ui/shoteditor.slint, `Core::thumb_*` and `Shots::make_thumb`/`drag_out` in src/shots.rs)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Bottom right, 24 px from the edges, on the screenshot's screen, overlay layer, no keyboard | 15-29 | anchored BOTTOM|RIGHT, fixed 340×260 window, card at its bottom right minus 24 | same |
| 2 | Card: picture fitted in 200×200 (never thinner than 72) + 6 px padding; Undo toast: its row + 32 (Material) / 24 (Glass), 48 / 44 high | 31-38 | `thumb_size`; toast 262×48 / 244×44 | known difference: the toast's width is fixed (the row's text is fixed, so it matches within a few px) |
| 3 | Slides in from the right (progress 300 ms OutCubic), leaves to the right 200 ms InCubic (after a drop: fades in place) | 44-86, 145-149 | `animate progress`, `fade` | same |
| 4 | A short swipe springs back: offset → 0 over 200 ms OutCubic | 87-94, 268-275 | `slide` (animated copy of `offset`) | **was** a jump back to 0; now 200 ms OutCubic (follows the hand at once while swiping) |
| 5 | 5 s for a screenshot, 8 s for the Undo toast; hovering holds it (the HoverHandler covers the whole card, toast included); not while dragging or while a "said" shows | 96-102, 169 | `life_check`, `hovered` | **was** the hover TouchArea disabled on the toast, so hovering the Undo toast didn't hold its 8 s; now it does (presses on the toast still do nothing but Undo) |
| 6 | A newer screenshot (or a save) slides it in again | 103-109 | `thumb_enter` | same |
| 7 | Picture loaded asynchronously at ≤ 400 px, cover, rounded 8 (Glass) / 10 (Material), fill behind; a quiet image glyph while loading; "Couldn't load preview" on error | 177-213 | decode thread `ThumbDecoded`, `status` | same |
| 8 | Press darkens 12 % (Glass black) / 10 % onSurface (Material), 100 ms | 214-220 | press Rectangle | same |
| 9 | Hint at the bottom while hovered and loaded, or while something was said: "Click to mark up" (Material plain tooltip r 4 h 24; Glass pill r 11 h 22, 78 % dark) | 223-240 | hint Rectangle | same |
| 10 | Click opens the editor; > 6 px sideways (dx > |dy|) swipes (dx, or dx/4 the wrong way), released past 56 px dismisses; otherwise a drag of the file out (text/uri-list) | 242-287 | `thumb_press/drag/release`, `ThumbDragOut` | same |
| 11 | After a drag: dropped → fade out; not dropped → stays (life restarts) | 390-408 | `drag_out` | known difference: Wayland doesn't tell tessera-ui whether the drop was taken; the thumbnail fades out once a drag starts |
| 12 | Copy text button top right (28, r 14), on hover when loaded and not reading; reads the text, copies it, says "Text copied" / "No text found" / "Install Tesseract to copy text" / "Couldn't read the text" for 2.5 s | 111-138, 289-313 | `thumb_copy_text`, `live_done` | same |
| 13 | Close button at the top-left corner (−6, −6), 22 Glass / 24 Material, on hover; hit area 6 px bigger | 315-338 | close Rectangle | same |
| 14 | Undo toast: trash, "Screenshot deleted", Undo (Material inversePrimary text action, r 16 h 32; Glass fill, Theme.radius.control, h 28) | 341-384 | toast row | same |
| 15 | Undo press: Glass scale 0.97 (100 ms); Material ElasticControl swell (min(6 %, 10 px)) on springPress | 362-365 | `transform-scale` | **was** Material 1.04 over a linear 100 ms; now 1.06 on 290 ms spring(0.57) (springPress 7.5/0.3) |
| 16 | Glass card corner 14 (toast 22); Material 16 (snackbar 4) | 152-167 | border-radius, glass shape | same |

## Test plan (tester)
Arena 1920×1080, both shells, both looks. 1600×900 `/tmp/t.png` (Annotation plan). Card 212×125 at x 1684..1896,
y 931..1056, centre (1790, 993); close button centre (1689, 936) Glass / (1690, 937) Material; Copy text (1870, 957).

1. **Enter / leave timing**: `P move 960 400`; `rec th-in 6 "ipc shotEditor thumbnail /tmp/t.png"` → slides in ≈ 300 ms, leaves after 5 s in ≈ 200 ms. Compare card x per frame QML vs Rust.
2. **Hover holds**: thumbnail in, `P glide 1790 993 200`, wait 7 s → still there, hint "Click to mark up", close and Copy text buttons shown; `P glide 960 400 200` → leaves 5 s later.
3. **Swipe back** (row 4): `P move 1760 993`, `P down`, `P glide 1800 993 150` (40 px, < 56), `rec th-back 0.5 "P up"` → the card glides back over ≈ 200 ms (Rust before: jumped). Swipe 120 px → leaves to the right.
4. **Toast hover** (row 5): open the editor (click the card), click the trash → toast. `P glide 1780 1034 200` (on the toast), wait 10 s → still there; move away → gone 8 s later. QML and Rust alike.
5. **Undo press** (row 15, Material): `P move <Undo centre ≈ 1853 1032>`, `P down`, `rec undo-press 0.5 "sleep 0.3; P up"` → swells ~6 % with a small overshoot on release; Glass: shrinks to 97 %.
6. **Copy text**: hover, `P click 1870 957` → hint "Reading text…" then "Text copied" (2.5 s); `wl-paste` holds the picture's text.
7. **Drag out**: `P move 1790 993`, `P down`, `P glide 1790 800 200` (vertical) → a file drag starts (cursor shows it); `P up` over the desktop → Rust fades out (row 11; QML stays when nothing took it).
8. **Close button**: hover, `P click 1689 936` → leaves to the right.
