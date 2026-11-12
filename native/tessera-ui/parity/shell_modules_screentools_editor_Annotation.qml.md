# shell/modules/screentools/editor/Annotation.qml → surfaces/screentools (src/draw.rs geometry and flattening, `ann_ui`/`push_anns` in src/shots.rs, `AnnView` in ui/shoteditor.slint)

Marks live in the image's own pixels (session `Ann`); Rust maps them to display pixels for Slint (`ann_ui`) and draws
them again onto the screenshot's pixels to save (`draw::flatten`, tiny-skia + resvg for text) where the QML used
`grabToImage`.

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | One item per mark, in a Repeater over the session's items, plus one for the draft | 35-38 (and ShotEditorWindow 537-553) | `push_anns` → `anns: Rows<StAnn>`; `has-draft` + `draft` | **was** a new `VecModel` per change (every mark's element made again on each add / undo / tool switch / crop); now one `Rows` handed to the window once, only rows that differ change |
| 2 | Pen: polyline, round cap and join, ink colour, width unit×{1,2,3.5}; a click leaves a dot (+0.01 px point) | 40-55 | `AnnView` kind 0, `draw::polyline` | same |
| 3 | Highlighter: width ×4, flat cap, drawn opaque in its own layer then faded to 40 % (no darkening where it crosses itself) | 57-83 | kind 1: `Rectangle { opacity: 0.4 }` around the Path (Slint renders an opacity element with children as a layer) | same |
| 4 | Arrow: shaft (round cap) stops 30 % into the head; head length min(60 % of arrow, width×3.2+10), half-width 55 %, filled, stroke max(1, width/2), round join | 85-124 | `draw::arrow_parts`; kind 2 (two Paths) | same |
| 5 | Rectangle: border inside the box, radius min(width, short side/2) | 126-139 | kind 3 Rectangle, `draw::rect_radius` | same |
| 6 | Ellipse: stroke centred on radii (w−width)/2, (h−width)/2 (≥ 0.5) | 141-161 | kind 4, `draw::ellipse_radii` + `ellipse_path` | same |
| 7 | Text: Theme font, DemiBold, pixelSize = size, top-left at (x, y), plain text | 163-173 | kind 5 Text weight 600 (`default-font-family: Theme.font`) | same |
| 8 | Redact: the screenshot under the box sampled `block`× smaller (mipmapped texture), shown unsmoothed; markup on top not sampled | 175-192 | kind 6: `draw::pixelate` block averages, `image-rendering: pixelated` | **was** every redaction's blocks recomputed on the UI thread on every `push_anns` (and a new picture each time, so Rows saw a change); now made once per mark (`pix_cache`, keyed by box+block, cleared when the screenshot reloads); a draft's are made per move as before. Known difference: true block averages vs Qt's mip-level sample (colours within a few levels) |
| 9 | Flattened output at native resolution, crop applied | ShotEditorWindow 200-205 | `draw::flatten` on a thread | same (vector marks redrawn; antialiasing may differ by ≤ 1 px on edges) |

## Test plan (tester)
Arena IRICE-TEST 1920×1080, both shells (`--only qml` / `--only rust`), both looks. Make a test picture with text:
`inbox 'magick -size 1600x900 xc:white -fill black -pointsize 48 -annotate +100+200 "Invoice total 42.00" -annotate +100+320 "https://example.com/pay" /tmp/t.png'`
(no magick: `grim -o IRICE-TEST /tmp/t.png` after opening a terminal with some text). Open the editor:
`inbox 'ipc shotEditor edit /tmp/t.png'`, wait 1 s.
Geometry (1600×900 picture): Glass canvas x 280..1640, y 157..923 (fit 0.851); Material canvas x 301..1619, y 169..911 (fit 0.824).
Tool keys: P pen, H highlighter, A arrow, R rectangle, O ellipse, T text, B redact, C crop.

1. **Pen + dot**: `wtype -k p`; `P move 500 400`, `P down`, `P glide 800 500 300`, `P up`; then `P click 900 400` (a dot). `dev shot pen` both shells: same stroke width/colour (Vermilion #D05F43), round ends, the dot round.
2. **Highlighter crossing itself**: `wtype -k h`; `P down` at (500, 600), `P glide 900 600 200`, `P glide 700 550 200`, `P up`. The crossing must not be darker (both shells); flat ends.
3. **Arrow**: `wtype -k a`; drag 600,700 → 1000,700 (`P move`, `P down`, `P glide 1000 700 200`, `P up`). Compare head size/shape and that the shaft doesn't poke past the tip. Shift-drag (`wtype -M shift` held) snaps to 45°.
4. **Rectangle / ellipse**: `wtype -k r`, drag 1100,300 → 1400,500; `wtype -k o`, drag 1100,550 → 1400,750. Border inside the box (zoom the shots: outer edge at the drag corners).
5. **Text**: `wtype -k t`; `P click 400 820`; `wtype 'Hello'`; `wtype -k Return`. Label in the ink, weight 600, same baseline in both.
6. **Redact (Rows / cache)**: `wtype -k b`; drag over "Invoice total" (Glass ≈ 360,320 → 700,370). Blocks appear while dragging and stay after release. Then draw a pen stroke elsewhere: the redaction must not flicker (record `rec redact-keep 1 "wtype -k p"` + a stroke; `tools/region-changes.py` over the redaction box: no change). QML vs Rust block colours may differ slightly (known).
7. **Undo keeps the others in place**: with 6 marks, `rec undo 1 "wtype -M ctrl -k z -m ctrl"`; only the last mark disappears; the others don't blink (region-changes over mark 1 shows no change). Redo `-M ctrl -M shift -k z`.
8. **Saved output**: copy the picture first per shell (`inbox 'cp /tmp/t.png /tmp/t-qml.png'`, `/tmp/t-rust.png`), draw steps 1-6 on each, Ctrl+S. Compare the two files (`magick compare -metric AE -fuzz 10% /tmp/t-qml.png /tmp/t-rust.png /tmp/diff.png`): differences only on antialiased edges, text rendering and redaction block colours.
