# shell/modules/screentools/editor/LiveTextLayer.qml → surfaces/screentools (`Live` in src/livetext.rs, `ltui::set_boxes` / `glow_at` in src/ltui.rs, `LiveTextLayer` in ui/shoteditor.slint, wiring in src/shots.rs)

The layer is shared with Quick Look (quicklook.rs, not mine: it still uses `ltui::boxes`, a new model per refresh).

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Reads when turned on or the picture's key (path + revision) changes: clear, status "reading", LiveText.read + WifiShare.find | 46-65 | `Live::start` → `livetext.read` (the shell's RustBridge also runs WifiShare.find and sends `{wifi}`) | same |
| 2 | Tesseract just installed while "missing": read again | 67-71 | `Shots::bridge` | **was** only the bar redrawn (stayed "needs Tesseract" until reopened); now reads again when `ocrAvailable` turns true while "missing" (Quick Look's copy in quicklook.rs not changed) |
| 3 | Answer for this key: page size, words, status; select what Search found it by; glow if ready | 72-84, 96-107 | `Live::done`, `select_found` | same |
| 4 | Glow: 0→1 in Theme.motion.hover (150 ms) OutCubic, hold 400, →0 over Theme.motion.large (450) InOutQuad | 119-126 | `ltui::glow_at` driven by a 16 ms timer | **was** the timer never stopped (a frame every 16 ms setting 0 forever after a read); now it stops at 1000 ms |
| 5 | A box per word, mapped from picture px to the shown part (crop) at −2/−1, +4/+2 px; selected 36 % accent, glow 22 %×glow; a selected word reaches the next on its line (one band), rounded only at the band's ends (Material 4, Glass 3) | 127-151 | `livetext::boxes`, `LtBox` | same |
| 6 | Boxes kept while the selection changes (Repeater over `words`; only bindings move) | 127 | `lt_boxes: Rows<LtBox>` | **was** a new model on every press/drag step (every box element made again per pointer move); now one model, changed in place |
| 7 | Press: on a word selects it; Shift extends to the nearest word; elsewhere clears | 166-179 | `Live::press` | same |
| 8 | Drag: after 4 px, from the nearest word to the press to the word nearest the pointer | 180-192 | `Live::drag` | same |
| 9 | Double-click: the word's line | 193-201 | `Live::double` | same |
| 10 | I-beam over a word, arrow elsewhere; only while ready | 155-161, 182-184 | `over-word`, `lt-hover` | same |
| 11 | Copy (selection, else all) / copyAll with "Text copied" / "All text copied"; Ctrl+C; Ctrl+A selects all (in the editor's Select text tool); Esc clears the selection first | 108-116, ShotEditorWindow 346-347, 396-397 | `key`, `live_copy` | same |
| 12 | Accessible: static text "Text in the picture" | 204-206 | `accessible-role: text` | same |

## Test plan (tester)
Arena 1920×1080, both shells, both looks, tesseract in the arena. Test picture of the Annotation plan; editor open
(`ipc shotEditor edit /tmp/t.png`), `wtype -k s`. Glass: "Invoice" at picture (100..270, 160..210) → screen ≈ (365..510, 293..336); Material ≈ (383..523, 301..342).

1. **Glow once**: `rec lt-glow 2 "wtype -k s"` (from another tool): every word box glows in (≈ 150 ms), holds, fades (≈ 450 ms); compare the curve QML vs Rust (`tools/anim-timing.py` on a box). After 1.1 s: no more frames from the editor (Rust before: a repaint every 16 ms forever — check with `dev frames` / the surface's commit count staying flat).
2. **Click a word**: `P click 430 315` (Glass) → "Invoice" selected (accent band), bar "1 word selected".
3. **Drag** (row 6): `P move 380 315`, `P down`, `P glide 900 440 400`, `P up`. Selection grows word by word; `rec lt-drag 0.6 "…"` and compare: no box flickers (before: each step rebuilt all boxes).
4. **Shift+click**: `P click 430 315`, then hold `wtype -M shift`, `P click 900 440`, release → selection from Invoice to the link line.
5. **Double-click** a word on the first line → the whole line.
6. **Hover cursor**: `P glide 430 315 100` → I-beam; `P glide 1500 800 100` → arrow (`dev cursor` if available, else mark not testable).
7. **Esc**: with a selection, `wtype -k Escape` clears it (editor stays open); a second Escape closes the editor.
8. **Crop + live text**: crop to the right half (C, drag, Enter), then S: boxes only for the words in the crop, positioned on them.
