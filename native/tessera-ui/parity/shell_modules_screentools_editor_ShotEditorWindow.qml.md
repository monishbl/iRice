# shell/modules/screentools/editor/ShotEditorWindow.qml → surfaces/screentools (`ShotEditor` in ui/shoteditor.slint, `Core` + `Shots::make_editor` in src/shots.rs, flattening in src/draw.rs)

Material's split button is drawn inline here (shell/common/SplitButton.qml); its springs: springPress 7.5/0.3 →
ω 21.65, ζ 0.43 → 290 ms spring(0.57).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Centred overlay window, keyboard exclusive except while the share sheet is over it | 16-26 | `Anchor::empty`, keyboard follows `share_open` | same |
| 2 | Geometry: picture fitted in 82 % of the screen (bars and padding out), at most 1:1, canvas ≥ 720×240 (440 high while loading); the shown part at fit ≤ 2; window keeps the whole picture's size while cropping; shadow room 24, pad 12, bar 48 (Material) / 36 | 28-64 | `geo`, `recompute` | same |
| 3 | Stroke unit min(w, h)/360 (≥ 1): pen ×1/2/3.5, text ×6/9/13, blocks ×3/5/8 | 54-58 | `pen_width`, `text_size`, `block_size` | same |
| 4 | Enter: opacity + scale 0.97→1 over 300 ms OutCubic; leave 200 ms InCubic, then what it left for (close / saved / delete) | 150-173 | `animate progress`, `closed` → `editor_closed` | same |
| 5 | Tools (Crop C, Pen P, Highlighter H, Arrow A, Rectangle R, Ellipse O, Text T, Redact B, Select text S); switching commits typed text and applies a pending crop; Crop remembers where to return | 92-124, 350-369 | `set_tool`, `TOOLS` | same (Rust also takes Shift+letter: QML's plain-key Shortcut doesn't) |
| 6 | Top bar: Close (Esc), separator, the tools (Material: a primaryContainer pill, elevation shadow 3/10, tools 2 apart, selected filled primary), Undo / Redo right | 408-492 | top bar | same |
| 7 | Canvas: radius module − pad, fill colour, the shown part centred at fit, clipped to it | 494-579 | `canvas`, `view` | same |
| 8 | Crop frame and handles drawn over the picture but not clipped by it; the input reaches 8 px past its edges | 581-640 | `view` + inner clip Rectangle | **was** the whole view clipped: handles cut in half at the picture's edge, and the 8 px reach outside the picture didn't take presses when the picture was letterboxed; now only the picture and marks clip (the QML's cropView), the canvas clips the rest |
| 9 | Drawing in native pixels: pen/highlighter points ≥ 1.5 display px apart; shapes appear once the pointer moves; Shift: squares, circles, 45° arrows; tiny drags (< 4 px, arrows < 8) dropped; a pen click leaves a dot | 634-790 | `down` / `moved` / `up`, `constrain` | same |
| 10 | Crop: dim 50 % outside; frame Material 2 px primary / Glass 1 px white; thirds while pressed; handles 12 (Material round primary with 2 px onPrimary; Glass r 3 white, 30 % black edge); hit 12 display px; cursors per handle; a click restores; Enter applies, Esc cancels; readout "W × H" (112 wide), Reset (disabled when whole), Apply crop | 581-632, 654-664, 711-739, 762-768, 1003-1031 | crop elements, `crop_hit`, `apply_crop`, `cancel_crop` | same |
| 11 | Text: click places a field (size·0.6 above the click), accent 1 px border r 4, 4 px out; Enter commits, Esc cancels; a colour or size pick changes the label being typed | 555-577, 694-701, 1080-1121 | `text-field`, `commit_text`, `on_swatch`, `on_set_width` | same |
| 12 | Colours: 8 swatches (Material 24 wide, Glass 26), 36 high; 24 ring 2 px (label when current, separator on hover), 16 dot | 1041-1089 | swatch Rectangles | same |
| 13 | Swatch press: Glass 0.9 (100 ms); Material ElasticControl swell (6 %) on springPress | 1069-1073 | `transform-scale` | **was** Material 1.12 over a linear 100 ms; now 1.06 on 290 ms spring(0.57) |
| 14 | Widths: three EditorButtons (A 11/14/18 for text, blocks for redact, dots else; tooltips per tool); "Block size" label for redact; row dimmed 50 % until loaded | 1033-1142 | width buttons, `width_tips` | same (`width_tips` now a Rows, not a new model per refresh) |
| 15 | Esc order: banner (prompt → keep editing) → draft → Live Text selection → crop → close (asks when dirty) | 388-406, 175-184 | `key` | same |
| 16 | Shortcuts: Ctrl+Z, Ctrl+Shift+Z / Redo, Ctrl+C (selected text first, else the image), Ctrl+A (Select text), Ctrl+S, Ctrl+Shift+S; none while typing (Ctrl+C/S too), undo/redo/tools not while busy | 342-357 | `key` | same |
| 17 | Loading: Material its 48 px indicator, Glass a 24 px circle-notch, "Opening screenshot…" | 803-833 | `status == 0` | same |
| 18 | Couldn't open: image icon (Material on a 72 px cookie9 secondaryContainer), title, help, Close editor | 835-877 | `status == 2` | same |
| 19 | Banner (r menu, 12 in): error (errorContainer / #FDECEA) with Copy to clipboard (unless the copy failed), Try again, Dismiss; prompt with Keep editing / Discard changes / Save | 879-967 | banner | same |
| 20 | Toast: 2 s, Material snackbar 48 r 4 / Glass 32 r 16, check icon, 200 ms in, 140 ms out | 969-992, 77-85 | toast | same |
| 21 | Save (over the original; unchanged → just leaves), Save as a copy ("<name> (edited).png", "(edited 2)", …), Copy (wl-copy; flattened to $XDG_RUNTIME_DIR/tessera-markup.png when marked), Share (flattened to …/tessera-share/<name>), Delete (immediate, Undo toast); errors with how to fix and Try again | 198-302 | `save`, `save_copy`, `copy`, `share`, `remove`, `flattened` | same (flattening on a thread, draw.rs) |
| 22 | Actions: Delete (danger), Share, Copy (secondary, busy spinners), Glass: Save/Done primary + Save as a copy | 1152-1218 | actions | same |
| 23 | Material split button: lead (Save floppy / Done check, 14 px Medium) and a 40 px caret 2 px apart; inner corners 4, 8 pressed, the caret's round while open, on springPress | SplitButton 44-152 | split Rectangle | **was** 300 ms spring(0.3); now 290 ms spring(0.57) |
| 24 | Split button colours ease over Theme.motion.hover; each half swells ~6 % while pressed (ElasticControl) | SplitButton 54-57, 121-124 | `animate background`, `transform-scale` | **was** colours jumping and no swell; now as the QML |
| 25 | Caret: upward, 180° closed, 360° open, Theme.motion.popover OutCubic | SplitButton 125-132 | `transform-rotation` | **was** 180°→0° (turning the other way), linear; now 180°→360° OutCubic |
| 26 | Disabled split button (not loaded, or another action busy): 38 % and neither half takes clicks | SplitButton 42 | `split.can` | **was** the caret still opening the menu while disabled; now both halves off |
| 27 | Menu: surfaceContainer, r 16, shadow 4/16, ≥ 200 wide, 8 px above the button, right-aligned to it, fades in popover / out hover with 6 px drop; items 44 high, icon 20 onSurfaceVariant, 14 px text; Save as a copy, Copy image, Send to a nearby device; outside click closes | SplitButton 154-236 | menu Rectangle | **was** surfaceContainerHigh, 230 wide, 4 px too high (the button sits centred in the 48 bar); now as the QML. Known difference: the QML empties its items the moment it starts closing (a bare pill fades); Rust keeps them while fading |
| 28 | Live Text bar left of the actions, room = bar − actions − 24 | 1144-1150 | `room` = canvas − 380 | known difference: estimated (see the LiveTextBar file) |

## Test plan (tester)
Arena 1920×1080, both shells, both looks. 1600×900 `/tmp/t.png` (Annotation plan); `inbox 'ipc shotEditor edit /tmp/t.png'`.
Geometry: Glass window 1433×934 at (244, 73); canvas x 280..1640, y 157..923; top bar centre y 127 (tools x 351 + 40·i, Crop first; undo 1583, redo 1623); bottom bar centre y 953 (swatches x 293 + 28·i).
Material window 1390×934 at (265, 73); canvas x 301..1619, y 169..911; top bar y 133 (tools x 382 + 42·i; undo 1555, redo 1599); bottom bar y 947 (swatches x 313 + 26·i; split caret centre 1599, lead just left of it).

1. **Open / close timing**: `rec ed-open 1 "ipc shotEditor edit /tmp/t.png"` → fade + scale 0.97→1 in 300 ms; `rec ed-close 0.6 "wtype -k Escape"` → 200 ms out. Compare QML vs Rust frame by frame.
2. **Crop handles at the edge** (row 8): a very wide picture letterboxes in the 240 px minimum canvas: `magick -size 1900x200 xc:white /tmp/wide.png`, open it, `wtype -k c`. Canvas x 185..1735, y 420..660 (both looks); the picture 1550×163 at y 458..621. The four corner handles are whole (not cut in half) at the picture's corners. Grab the top-left handle from 5 px outside the picture: `P move 181 454`, `P down`, `P glide 400 520 200`, `P up` → the crop shrinks from that corner (before: the press did nothing). Enter applies; readout shows the size.
3. **Draw / constrain**: R with Shift → a square; A with Shift → 45°; a 2 px drag leaves nothing; pen click leaves a dot.
4. **Swatch press** (row 13): `P move 349 953` (Glass colour 3) / `P move 365 947` (Material), `P down`, `dev shot sw-press`, `P up`, `rec sw 0.5 "…"`. Glass 0.9 shrink; Material a ~6 % swell with a little overshoot when let go.
5. **Text**: T, `P click 600 500`, type `Hi`, click colour 3 → the label turns that colour; width button 3 → larger; Enter commits; Esc instead cancels.
6. **Esc order**: select text (S, Ctrl+A), Esc clears the selection; C then Esc cancels the crop; draw then Esc → "Save changes to this screenshot?" banner; Esc → Keep editing.
7. **Save / copy / error**: Ctrl+C → spinner then toast "Copied to the clipboard" (2 s); make the folder read-only (`chmod -w /tmp/ro` with the picture inside) and Ctrl+S → error banner "Couldn't save “…”" with Copy to clipboard / Try again / ×.
8. **Split button (Material)** (rows 23-27): hover the lead (≈ 1540, 947): colour eases in 150 ms; `P down` → swells and its inner corners go to 8 on a spring; click the caret (1599, 947) → `rec split 0.6 "P click 1599 947"`: caret turns clockwise 180→360 over 220 ms OutCubic, its inner corners round up with a small overshoot (290 ms), menu (surfaceContainer, 200 wide) fades in with its bottom 8 px above the button's top (y ≈ 919). Click outside → fades out (Rust keeps the items while fading: known).
9. **Disabled split** (row 26): `inbox 'ipc shotEditor edit /tmp/missing.png'` → "Couldn't open this screenshot"; the split button is 38 % and clicking its caret does nothing.
10. **Loading**: open a big picture (`magick -size 7000x4000 plasma: /tmp/big.png`) → indicator + "Opening screenshot…" until it loads.
